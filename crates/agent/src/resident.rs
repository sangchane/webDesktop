//! 상주 계층 — 설정·기기 키·등록(E-04)·허브 서명 WS(E-05)·재접속 (04 「코드 구조」 resident).
use crate::platform;
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use ed25519_dalek::{Signer, SigningKey};
use futures_util::{SinkExt, StreamExt};
use proto::{EnrollReq, EnrollResp, ErrorType, Os, Problem, WsMsg, PROTO_MAJOR};
use serde::{Deserialize, Serialize};
use std::{io::BufRead, path::PathBuf, time::Duration};
use tokio::time::{interval_at, sleep, sleep_until, timeout, Instant};
use tokio_tungstenite::tungstenite::Message;
use tracing::{info, warn};

/// 03 상수 표 — 재접속 대기 상한(1초부터 두 배씩).
pub const AGENT_RECONNECT_MAX_SEC: u64 = 60;
/// 허브 판(빌드 때 `WEBDESKTOP_HUB` 로 바꿀 수 있음). 실행 때 같은 이름 환경 변수가 이긴다(시험용).
const HUB_BUILT_IN: &str = match option_env!("WEBDESKTOP_HUB") {
    Some(u) => u,
    None => "https://webdesktop-hub.sangchane87.workers.dev",
};
/// 허브가 같은 기기의 새 연결을 받아 앞 연결을 닫을 때 쓰는 닫힘 코드(허브 entry CLOSE_REPLACED).
const CLOSE_REPLACED: u16 = 4000;

pub fn hub_url() -> String {
    std::env::var("WEBDESKTOP_HUB").unwrap_or_else(|_| HUB_BUILT_IN.to_owned())
}

/// 에이전트 자료 폴더 — 설정·키·로그. 기본 `%ProgramData%\WebDesktop`, 시험은 `WEBDESKTOP_HOME`.
pub struct Home(pub PathBuf);

impl Home {
    pub fn from_env() -> Self {
        let dir = std::env::var_os("WEBDESKTOP_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                PathBuf::from(
                    std::env::var_os("ProgramData").unwrap_or_else(|| r"C:\ProgramData".into()),
                )
                .join("WebDesktop")
            });
        Self(dir)
    }
    fn config(&self) -> PathBuf {
        self.0.join("config.json")
    }
    fn key(&self) -> PathBuf {
        self.0.join("device.key")
    }
    pub fn logs(&self) -> PathBuf {
        self.0.join("logs")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Config {
    pub hub_url: String,
    pub device_id: String,
}

impl Config {
    /// 바닥선 8 — 틀린 설정이면 기동하지 않는다.
    fn check(&self) -> Result<(), String> {
        if !hub_ok(&self.hub_url) {
            return Err(format!("허브 주소가 틀림: {}", self.hub_url));
        }
        if !proto::id_ok(&self.device_id) {
            return Err("device_id 꼴이 틀림".into());
        }
        Ok(())
    }
}

/// https 만. 내 PC 의 시험 허브만 http 를 허용한다.
fn hub_ok(u: &str) -> bool {
    let rest = u
        .strip_prefix("https://")
        .or_else(|| u.strip_prefix("http://127.0.0.1:"))
        .or_else(|| u.strip_prefix("http://localhost:"));
    rest.is_some_and(|r| !r.is_empty() && !r.ends_with('/') && !r.contains(char::is_whitespace))
}

fn ws_url(hub: &str) -> String {
    format!("ws{}/agent/ws", hub.strip_prefix("http").unwrap_or(hub))
}

/// 저장한 설정·키. 등록 전이면 None, 파일이 깨졌으면 Err.
pub fn load(home: &Home) -> Result<Option<(Config, SigningKey)>, String> {
    let raw = match std::fs::read_to_string(home.config()) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(io_msg("설정 읽기", &e)),
    };
    let cfg: Config = serde_json::from_str(&raw).map_err(|e| format!("설정 꼴: {e}"))?;
    cfg.check()?;
    let sealed = std::fs::read(home.key()).map_err(|e| io_msg("키 읽기", &e))?;
    let seed: [u8; 32] = platform::unprotect(&sealed)
        .map_err(|e| format!("키 풀기: {e}"))?
        .try_into()
        .map_err(|_| "키 길이가 틀림".to_owned())?;
    Ok(Some((cfg, SigningKey::from_bytes(&seed))))
}

/// 등록이 풀렸을 때 — 설정만 지운다. 키는 다음 등록이 새로 만든다.
pub fn forget(home: &Home) -> Result<(), String> {
    match std::fs::remove_file(home.config()) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(io_msg("설정 지우기", &e)),
        _ => Ok(()),
    }
}

/// 관리자 권한으로 처음 만든 파일은 일반 실행에서 읽지도 지우지도 못한다 — 그 경우를 따로 알린다.
fn io_msg(what: &str, e: &std::io::Error) -> String {
    if e.kind() == std::io::ErrorKind::PermissionDenied {
        format!("{what}: 권한 없음 — 관리자 권한으로 실행해 만든 파일일 수 있습니다. 관리자로 실행하세요")
    } else {
        format!("{what}: {e}")
    }
}

/// 사람이 친 코드를 허브 꼴로 — 소문자·띄어쓰기·붙임표를 받아 준다.
fn normalize_code(s: &str) -> Option<String> {
    let c: String = s
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '-')
        .collect::<String>()
        .to_ascii_uppercase();
    proto::reg_code_ok(&c).then_some(c)
}

fn device_name() -> String {
    let n = std::env::var("COMPUTERNAME").unwrap_or_else(|_| "Windows PC".into());
    n.chars()
        .filter(|c| !c.is_control())
        .take(proto::NAME_MAX)
        .collect()
}

/// 코드를 물어 등록한다(E-04). 틀리면 다시 묻고, 입력이 끝나면 Err.
pub fn enroll(
    home: &Home,
    hub: &str,
    input: &mut impl BufRead,
) -> Result<(Config, SigningKey), String> {
    if !hub_ok(hub) {
        return Err(format!("허브 주소가 틀림: {hub}"));
    }
    let agent = ureq::Agent::config_builder()
        .http_status_as_error(false)
        .timeout_global(Some(Duration::from_secs(30)))
        .tls_config(
            ureq::tls::TlsConfig::builder()
                .provider(ureq::tls::TlsProvider::NativeTls)
                .root_certs(ureq::tls::RootCerts::PlatformVerifier)
                .build(),
        )
        .build()
        .new_agent();
    loop {
        println!("웹앱 「기기 추가」에 보이는 등록 코드 8자리를 입력하세요:");
        let mut line = String::new();
        if input.read_line(&mut line).map_err(|e| e.to_string())? == 0 {
            return Err("입력이 끝났습니다".into());
        }
        let Some(code) = normalize_code(&line) else {
            println!("코드는 숫자·영문 8자리입니다. 다시 입력하세요.");
            continue;
        };
        let mut seed = [0u8; 32];
        getrandom::fill(&mut seed).map_err(|e| format!("난수: {e}"))?;
        let key = SigningKey::from_bytes(&seed);
        let sealed = platform::protect(&seed).map_err(|e| format!("키 잠그기: {e}"))?;
        platform::write_private(&home.key(), &sealed).map_err(|e| format!("키 쓰기: {e}"))?;
        let req = EnrollReq {
            code,
            pubkey: B64.encode(key.verifying_key().as_bytes()),
            name: device_name(),
            os: Os::Windows,
            agent_version: env!("CARGO_PKG_VERSION").into(),
        };
        let body = serde_json::to_string(&req).map_err(|e| e.to_string())?;
        let mut resp = match agent
            .post(format!("{hub}/agent/enroll"))
            .header("content-type", "application/json")
            .send(body)
        {
            Ok(r) => r,
            Err(e) => {
                warn!(event = "enroll.failed", why = %e);
                println!("허브에 닿지 못했습니다({e}). 인터넷을 확인하고 코드를 다시 입력하세요.");
                continue;
            }
        };
        let status = resp.status().as_u16();
        let text = resp.body_mut().read_to_string().unwrap_or_default();
        if status == 201 {
            let r: EnrollResp =
                serde_json::from_str(&text).map_err(|e| format!("허브 응답: {e}"))?;
            let cfg = Config {
                hub_url: hub.to_owned(),
                device_id: r.device_id,
            };
            cfg.check()?;
            let tmp = home.config().with_extension("tmp");
            std::fs::write(
                &tmp,
                serde_json::to_vec_pretty(&cfg).map_err(|e| e.to_string())?,
            )
            .and_then(|_| std::fs::rename(&tmp, home.config()))
            .map_err(|e| {
                format!(
                    "{} — 허브에는 등록됐지만 이 PC 에 저장하지 못했습니다. 다시 실행해 새 코드로 등록하세요",
                    io_msg("설정 쓰기", &e)
                )
            })?;
            info!(event = "enroll.ok", device_id = %cfg.device_id);
            println!("등록했습니다. 이 PC 이름: {}", req.name);
            return Ok((cfg, key));
        }
        let kind = serde_json::from_str::<Problem>(&text).map(|p| p.kind).ok();
        warn!(event = "enroll.failed", status, kind = ?kind);
        println!(
            "{}",
            match kind {
                Some(ErrorType::CodeInvalid) =>
                    "코드가 맞지 않습니다. 웹앱의 코드를 다시 확인하세요.",
                Some(ErrorType::CodeExpired) =>
                    "코드가 만료됐습니다. 웹앱에서 새 코드를 받아 입력하세요.",
                Some(ErrorType::RateLimited) => "시도가 너무 많습니다. 1분 뒤 다시 입력하세요.",
                _ => "등록하지 못했습니다. 잠시 뒤 다시 입력하세요.",
            }
        );
    }
}

/// 재접속을 멈추는 사유.
#[derive(Debug, PartialEq)]
pub enum Stop {
    /// 허브가 이 기기를 모르거나 폐기함 — 다시 등록해야 한다.
    Reenroll,
    /// 판이 다름 — 업데이트가 필요하다.
    Upgrade,
    /// 같은 기기의 다른 연결이 이 연결을 대신함(에이전트 두 개).
    Replaced,
}

enum Outcome {
    Stop(Stop),
    /// 다시 붙는다. healthy = 인증 뒤 유지 신호까지 오갔음(대기 시간을 처음부터).
    Retry {
        healthy: bool,
        why: String,
    },
}

fn retry(why: impl std::fmt::Display) -> Outcome {
    Outcome::Retry {
        healthy: false,
        why: why.to_string(),
    }
}

/// 허브 오류 중 재접속으로는 풀리지 않는 것.
fn fatal(kind: ErrorType) -> Option<Stop> {
    match kind {
        ErrorType::BadSignature | ErrorType::DeviceRevoked => Some(Stop::Reenroll),
        ErrorType::UpgradeRequired => Some(Stop::Upgrade),
        _ => None,
    }
}

fn next_wait(sec: u64) -> u64 {
    (sec * 2).min(AGENT_RECONNECT_MAX_SEC)
}

/// 허브에 붙어 있고, 끊기면 다시 붙는다. 멈출 사유가 생길 때만 돌아온다.
pub async fn run(cfg: &Config, key: &SigningKey) -> Stop {
    let mut wait = 1;
    loop {
        match connect(cfg, key).await {
            Outcome::Stop(s) => return s,
            Outcome::Retry { healthy, why } => {
                if healthy {
                    wait = 1;
                }
                warn!(event = "agent.disconnected", why, retry_sec = wait);
                println!("허브 연결이 끊겼습니다({why}). {wait}초 뒤 다시 붙습니다.");
                sleep(Duration::from_secs(wait)).await;
                wait = next_wait(wait);
            }
        }
    }
}

async fn connect(cfg: &Config, key: &SigningKey) -> Outcome {
    let beat = Duration::from_secs(proto::AGENT_HEARTBEAT_SEC);
    let silent = Duration::from_secs(proto::OFFLINE_DETECT_SEC);
    let mut ws = match timeout(beat, tokio_tungstenite::connect_async(ws_url(&cfg.hub_url))).await {
        Ok(Ok((ws, _))) => ws,
        Ok(Err(e)) => return retry(e),
        Err(_) => return retry("접속 시간 초과"),
    };
    let nonce = match timeout(beat, ws.next()).await {
        Ok(Some(Ok(Message::Text(t)))) => match proto::parse::<WsMsg>(&t) {
            Ok(WsMsg::Hello { proto, .. }) if proto != PROTO_MAJOR => {
                return Outcome::Stop(Stop::Upgrade)
            }
            Ok(WsMsg::Hello { nonce, .. }) => nonce,
            _ => return retry("hello 아님"),
        },
        _ => return retry("hello 없음"),
    };
    let sig = key.sign(&proto::auth_message(&nonce, &cfg.device_id));
    let auth = WsMsg::Auth {
        device_id: cfg.device_id.clone(),
        sig: B64.encode(sig.to_bytes()),
        proto: PROTO_MAJOR,
    };
    let auth = serde_json::to_string(&auth).expect("auth 직렬화");
    if let Err(e) = ws.send(Message::text(auth)).await {
        return retry(e);
    }
    info!(event = "agent.auth_sent", device_id = %cfg.device_id);
    println!("허브에 접속했습니다.");
    // 허브는 인증 성공을 따로 알리지 않는다. 실패는 곧바로 error 로 온다.
    // 자동 응답 pong 은 인증 전 연결에도 오므로, 첫 유지 신호(beat 뒤)의 pong 을 「인증 뒤 살아 있음」으로 본다.
    let mut ping = interval_at(Instant::now() + beat, beat);
    let mut healthy = false;
    // 조용함은 마지막으로 받은 시각부터 잰다 — select 마다 새 시한을 만들면 ping 차례마다 되돌아간다.
    let mut last_rx = Instant::now();
    loop {
        tokio::select! {
            _ = ping.tick() => {
                if let Err(e) = ws.send(Message::text("ping")).await {
                    return Outcome::Retry { healthy, why: e.to_string() };
                }
            }
            _ = sleep_until(last_rx + silent) => {
                return Outcome::Retry { healthy, why: "허브 응답 없음".into() };
            }
            m = ws.next() => { last_rx = Instant::now(); match m {
                None => return Outcome::Retry { healthy, why: "연결 닫힘".into() },
                Some(Err(e)) => return Outcome::Retry { healthy, why: e.to_string() },
                Some(Ok(Message::Close(f))) => {
                    if f.as_ref().map(|f| u16::from(f.code)) == Some(CLOSE_REPLACED) {
                        return Outcome::Stop(Stop::Replaced);
                    }
                    return Outcome::Retry { healthy, why: format!("허브가 닫음 {f:?}") };
                }
                Some(Ok(Message::Text(t))) if t.as_str() == "pong" => {
                    if !healthy {
                        healthy = true;
                        info!(event = "agent.connected", device_id = %cfg.device_id);
                    }
                }
                Some(Ok(Message::Text(t))) => match proto::parse::<WsMsg>(&t) {
                    Ok(WsMsg::Error { kind, .. }) => {
                        warn!(event = "hub.error", kind = ?kind);
                        if let Some(s) = fatal(kind) {
                            return Outcome::Stop(s);
                        }
                    }
                    // 세션 메시지는 T9(도우미). 내용(TURN 자격·SDP)은 로그에 남기지 않고 종류만.
                    Ok(m) => info!(event = "hub.msg", t = %serde_json::to_value(&m).ok().and_then(|v| v["t"].as_str().map(str::to_owned)).unwrap_or_default()),
                    Err(kind) => warn!(event = "hub.bad_msg", kind = ?kind),
                },
                Some(Ok(_)) => {}
            }}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signature, Verifier};

    #[test]
    fn code_is_normalized() {
        assert_eq!(
            normalize_code(" abcd-2345\r\n").as_deref(),
            Some("ABCD2345")
        );
        assert_eq!(normalize_code("ABCD 2345"), Some("ABCD2345".into()));
        assert_eq!(normalize_code("ABCD234"), None);
        assert_eq!(normalize_code("ABCD2340"), None); // 0 은 코드 글자가 아님
    }

    #[test]
    fn hub_url_rules() {
        assert!(hub_ok("https://webdesktop-hub.example.workers.dev"));
        assert!(hub_ok("http://127.0.0.1:8787"));
        assert!(!hub_ok("http://example.com"));
        assert!(!hub_ok("https://"));
        assert!(!hub_ok("https://a.dev/"));
        assert_eq!(ws_url("https://h.dev"), "wss://h.dev/agent/ws");
        assert_eq!(
            ws_url("http://127.0.0.1:8787"),
            "ws://127.0.0.1:8787/agent/ws"
        );
    }

    #[test]
    fn config_check() {
        let ok = Config {
            hub_url: "https://h.dev".into(),
            device_id: "0192-ab".into(),
        };
        assert!(ok.check().is_ok());
        assert!(Config {
            device_id: "a b".into(),
            ..ok.clone()
        }
        .check()
        .is_err());
        assert!(Config {
            hub_url: "ftp://h".into(),
            ..ok
        }
        .check()
        .is_err());
    }

    #[test]
    fn backoff_doubles_to_cap() {
        let mut w = 1;
        let seq: Vec<u64> = (0..8)
            .map(|_| {
                let x = w;
                w = next_wait(w);
                x
            })
            .collect();
        assert_eq!(seq, [1, 2, 4, 8, 16, 32, 60, 60]);
    }

    #[test]
    fn only_identity_errors_stop() {
        assert_eq!(fatal(ErrorType::BadSignature), Some(Stop::Reenroll));
        assert_eq!(fatal(ErrorType::DeviceRevoked), Some(Stop::Reenroll));
        assert_eq!(fatal(ErrorType::UpgradeRequired), Some(Stop::Upgrade));
        assert_eq!(fatal(ErrorType::RateLimited), None);
        assert_eq!(fatal(ErrorType::Internal), None);
    }

    #[test]
    fn auth_signature_verifies_like_hub() {
        let key = SigningKey::from_bytes(&[7; 32]);
        let sig = key.sign(&proto::auth_message("nonce", "dev-1"));
        let wire = B64.encode(sig.to_bytes());
        let back = Signature::from_slice(&B64.decode(wire).unwrap()).unwrap();
        assert!(key
            .verifying_key()
            .verify(&proto::auth_message("nonce", "dev-1"), &back)
            .is_ok());
        assert!(key
            .verifying_key()
            .verify(&proto::auth_message("nonce", "dev-2"), &back)
            .is_err());
        assert_eq!(
            B64.encode(key.verifying_key().as_bytes()).len(),
            proto::PUBKEY_B64_LEN
        );
    }

    #[test]
    fn saved_key_loads_back() {
        let home = Home(std::env::temp_dir().join(format!("wd-home-{}", std::process::id())));
        std::fs::create_dir_all(&home.0).unwrap();
        assert!(load(&home).unwrap().is_none());
        let cfg = Config {
            hub_url: "https://h.dev".into(),
            device_id: "dev-1".into(),
        };
        platform::write_private(&home.key(), &platform::protect(&[9; 32]).unwrap()).unwrap();
        std::fs::write(home.config(), serde_json::to_vec(&cfg).unwrap()).unwrap();
        let (c, k) = load(&home).unwrap().unwrap();
        assert_eq!(c, cfg);
        assert_eq!(k.to_bytes(), [9; 32]);
        forget(&home).unwrap();
        forget(&home).unwrap(); // 이미 없어도 성공
        assert!(load(&home).unwrap().is_none());
        std::fs::remove_dir_all(&home.0).unwrap();
    }
}
