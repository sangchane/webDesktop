//! 공유 계약 계층 — 허브·에이전트·웹이 같이 쓰는 메시지 이름·오류 이름 (05-api-contract.md).
//! 웹앱(`crates/web`, wasm)도 이 크레이트를 그대로 쓴다 — 타입 생성 단계 없음.
use serde::{Deserialize, Serialize};

/// 판 major. 다르면 상대는 `upgrade_required` 로 닫는다 (05 「규약」).
pub const PROTO_MAJOR: u32 = 1;
/// 03 상수 표.
pub const WS_MSG_MAX_BYTES: usize = 65536;
pub const REG_CODE_LEN: usize = 8;
/// 등록 코드 글자 — 헷갈리는 0·1·I·O 뺌 (05 OpenAPI `^[2-9A-HJ-NP-Z]{8}$`).
pub const REG_CODE_ALPHABET: &[u8] = b"23456789ABCDEFGHJKLMNPQRSTUVWXYZ";
/// 05 OpenAPI 필드 상한.
pub const NAME_MAX: usize = 64;
pub const AGENT_VERSION_MAX: usize = 32;
/// ed25519 공개키 32바이트의 base64(패딩 포함) 길이.
pub const PUBKEY_B64_LEN: usize = 44;
/// 식별자(uuid 문자열)·서명(64바이트 base64 88자)·nonce 상한.
pub const ID_MAX: usize = 64;
pub const SIG_B64_MAX: usize = 128;
/// 03 상수 표 — 등록·유지 신호·오프라인 판정·빈도.
pub const REG_CODE_TTL_MIN: u64 = 10;
pub const REG_CODE_MAX_TRIES: u32 = 5;
pub const INSTALL_LINK_TTL_MIN: u64 = 10;
pub const AGENT_HEARTBEAT_SEC: u64 = 30;
pub const OFFLINE_DETECT_SEC: u64 = 90;
pub const OFFLINE_WARN_HOURS: u64 = 24;
pub const RATE_LIMIT_PER_MIN: u32 = 60;
pub const TURN_CRED_TTL_MIN: u64 = 60;
/// E-13 뷰어 입장권 수명(1회용).
pub const WS_TICKET_TTL_SEC: u64 = 60;
/// 접속 기록에 남기는 뷰어 User-Agent 글자 수 상한.
pub const VIEWER_UA_MAX: usize = 256;

/// E-05 서명 대상 = nonce 글자 바이트 ‖ device_id 글자 바이트. 허브·에이전트가 같은 함수를 쓴다.
pub fn auth_message(nonce: &str, device_id: &str) -> Vec<u8> {
    [nonce.as_bytes(), device_id.as_bytes()].concat()
}

/// 05 「오류 이름」 전체. 직렬화 이름이 RFC 9457 `type` 값이다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorType {
    Unauthorized,
    NotFound,
    BadRequest,
    CodeInvalid,
    CodeExpired,
    DeviceOffline,
    DeviceRevoked,
    BadSignature,
    UpgradeRequired,
    RateLimited,
    TooLarge,
    Internal,
}

impl ErrorType {
    /// 05 표의 HTTP 상태.
    pub fn status(self) -> u16 {
        match self {
            Self::Unauthorized | Self::BadSignature => 401,
            Self::NotFound => 404,
            Self::BadRequest | Self::CodeInvalid | Self::CodeExpired => 400,
            Self::DeviceOffline => 409,
            Self::DeviceRevoked => 403,
            Self::UpgradeRequired => 426,
            Self::RateLimited => 429,
            Self::TooLarge => 413,
            Self::Internal => 500,
        }
    }
}

/// RFC 9457 Problem JSON.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Problem {
    #[serde(rename = "type")]
    pub kind: ErrorType,
    pub title: String,
    pub status: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl Problem {
    pub fn new(kind: ErrorType, detail: Option<String>) -> Self {
        let title = serde_json::to_value(kind)
            .ok()
            .and_then(|v| v.as_str().map(str::to_owned))
            .unwrap_or_default();
        Self {
            kind,
            title,
            status: kind.status(),
            detail,
        }
    }
}

/// 입구 꼴 검사 — 허브 `entry`·에이전트가 받은 직후 한 번 부른다.
pub trait Validate {
    fn validate(&self) -> Result<(), ErrorType>;
}

/// 크기 → JSON → 꼴 검사를 한 번에. 크기 초과는 `too_large`, 나머지는 `bad_request`.
pub fn parse<T: serde::de::DeserializeOwned + Validate>(raw: &str) -> Result<T, ErrorType> {
    if raw.len() > WS_MSG_MAX_BYTES {
        return Err(ErrorType::TooLarge);
    }
    let v: T = serde_json::from_str(raw).map_err(|_| ErrorType::BadRequest)?;
    v.validate()?;
    Ok(v)
}

fn check(ok: bool) -> Result<(), ErrorType> {
    if ok {
        Ok(())
    } else {
        Err(ErrorType::BadRequest)
    }
}

fn text_ok(s: &str, max: usize) -> bool {
    !s.is_empty() && s.chars().count() <= max && !s.chars().any(char::is_control)
}

/// 식별자 꼴(uuid 글자) — 허브 입구와 에이전트 설정 검사가 같이 쓴다.
pub fn id_ok(s: &str) -> bool {
    !s.is_empty() && s.len() <= ID_MAX && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
}

fn b64_ok(s: &str) -> bool {
    s.bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'/' | b'='))
}

/// 등록 코드 꼴 — 자리 수와 글자만 본다(맞는지는 허브가 해시로).
pub fn reg_code_ok(s: &str) -> bool {
    s.len() == REG_CODE_LEN && s.bytes().all(|b| REG_CODE_ALPHABET.contains(&b))
}

/// E-11 `/healthz` 응답.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Health {
    pub ok: bool,
    pub proto: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Os {
    Windows,
    Macos,
}

/// 호스트 권한 상태 (ERD `device.perm`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Perm {
    Ok,
    NeedScreen,
    NeedAccess,
    NeedLocalnet,
    NeedMulti,
}

/// 연결 경로 — 뷰어가 본 선택 후보 쌍 (M-10).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NetPath {
    Direct,
    Relay,
}

/// 세션 종료 사유 (05 「규약」).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EndReason {
    ViewerClosed,
    HostClosed,
    TakenOver,
    HostShutdown,
    Failed,
    DeviceRevoked,
}

/// WebRTC `RTCIceServer` 꼴.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IceServer {
    pub urls: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credential: Option<String>,
}

// ---- HTTP (시각은 ISO8601 문자열 — 05 「데이터 규칙」) ----

/// E-02 목록 한 줄 (VPN 칸은 2단계).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Device {
    pub id: String,
    pub name: String,
    pub os: Os,
    pub online: bool,
    pub last_seen_at: Option<String>,
    pub perm: Perm,
    pub connectable: bool,
    pub offline_long: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InstallLinks {
    pub windows: String,
    pub macos: String,
}

/// E-03 응답.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EnrollCode {
    pub code: String,
    pub expires_at: String,
    pub install_links: InstallLinks,
}

/// E-04 요청.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EnrollReq {
    pub code: String,
    pub pubkey: String,
    pub name: String,
    pub os: Os,
    pub agent_version: String,
}

impl Validate for EnrollReq {
    fn validate(&self) -> Result<(), ErrorType> {
        // 꼴이 틀린 코드도 시도 수로 세야 하므로(FR-002) 코드 글자는 허브가 본다 — 여기선 길이만.
        check(
            !self.code.is_empty()
                && self.code.len() <= REG_CODE_LEN
                && self.pubkey.len() == PUBKEY_B64_LEN
                && b64_ok(&self.pubkey)
                && text_ok(&self.name, NAME_MAX)
                && text_ok(&self.agent_version, AGENT_VERSION_MAX),
        )
    }
}

/// E-04 응답.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EnrollResp {
    pub device_id: String,
}

/// E-13 요청.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WsTicketReq {
    pub viewer_id: String,
}

impl Validate for WsTicketReq {
    fn validate(&self) -> Result<(), ErrorType> {
        check(id_ok(&self.viewer_id))
    }
}

/// E-13 응답.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WsTicket {
    pub ticket: String,
    pub hub_ws_url: String,
    pub expires_at: String,
}

/// E-07 요청.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionReq {
    pub device_id: String,
    pub viewer_id: String,
}

impl Validate for SessionReq {
    fn validate(&self) -> Result<(), ErrorType> {
        check(id_ok(&self.device_id) && id_ok(&self.viewer_id))
    }
}

/// E-07 응답.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionResp {
    pub session_id: String,
    pub ice_servers: Vec<IceServer>,
}

// ---- WebSocket (E-05·E-06). `t` 로 구분. 문자열 `ping`/`pong`(M-09)은 JSON 이 아니라 여기 없다. ----
// 한 열거형에 모든 방향을 둔다. 보낸 쪽이 보낼 수 없는 `t` 는 허브 entry 가 match 에서 거절한다.

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "t")]
pub enum WsMsg {
    /// E-05 1) 허브→에이전트.
    #[serde(rename = "hello")]
    Hello { nonce: String, proto: u32 },
    /// E-05 2) 에이전트→허브. sig = ed25519(nonce‖device_id), base64.
    #[serde(rename = "auth")]
    Auth {
        device_id: String,
        sig: String,
        proto: u32,
    },
    /// E-06 뷰어→허브 첫 메시지.
    #[serde(rename = "viewer.hello")]
    ViewerHello { viewer_id: String, proto: u32 },
    /// M-01 허브→뷰어.
    #[serde(rename = "device.status")]
    DeviceStatus {
        device_id: String,
        online: bool,
        perm: Perm,
        connectable: bool,
        last_seen_at: Option<String>,
    },
    /// M-02 허브→에이전트.
    #[serde(rename = "session.start")]
    SessionStart {
        session_id: String,
        ice_servers: Vec<IceServer>,
    },
    /// M-03 양방향.
    #[serde(rename = "signal.offer")]
    SignalOffer { session_id: String, sdp: String },
    #[serde(rename = "signal.answer")]
    SignalAnswer { session_id: String, sdp: String },
    #[serde(rename = "signal.candidate")]
    SignalCandidate {
        session_id: String,
        candidate: String,
    },
    /// ICE 재시작 요청 — 새 offer 는 `signal.offer` 로 따로 간다.
    #[serde(rename = "signal.restart")]
    SignalRestart { session_id: String },
    /// M-04 양방향.
    #[serde(rename = "session.end")]
    SessionEnd {
        session_id: String,
        reason: EndReason,
    },
    /// M-10 뷰어→허브.
    #[serde(rename = "session.viewer_stats")]
    ViewerStats {
        session_id: String,
        click_to_first_frame_ms: Option<u32>,
        latency_p50_ms: Option<u32>,
        reconnect_ms: Vec<u32>,
        restarts: u32,
        new_session_used: bool,
        path: Option<NetPath>,
    },
    /// WebSocket 오류 `{t:"error", type, detail}` (05 「규약」).
    #[serde(rename = "error")]
    Error {
        #[serde(rename = "type")]
        kind: ErrorType,
        detail: Option<String>,
    },
}

/// 재연결 기록 배열 상한 — 한 세션에 이보다 많으면 꼴이 틀린 것으로 본다.
pub const RECONNECT_LOG_MAX: usize = 100;

impl Validate for WsMsg {
    fn validate(&self) -> Result<(), ErrorType> {
        use WsMsg::*;
        match self {
            Hello { nonce, .. } => {
                check(!nonce.is_empty() && nonce.len() <= SIG_B64_MAX && b64_ok(nonce))
            }
            Auth { device_id, sig, .. } => check(
                id_ok(device_id) && !sig.is_empty() && sig.len() <= SIG_B64_MAX && b64_ok(sig),
            ),
            ViewerHello { viewer_id, .. } => check(id_ok(viewer_id)),
            DeviceStatus { device_id, .. } => check(id_ok(device_id)),
            SessionStart { session_id, .. }
            | SignalRestart { session_id }
            | SessionEnd { session_id, .. } => check(id_ok(session_id)),
            // sdp·candidate 길이는 parse() 의 WS_MSG_MAX_BYTES 가 막는다.
            SignalOffer { session_id, sdp } | SignalAnswer { session_id, sdp } => {
                check(id_ok(session_id) && !sdp.is_empty())
            }
            SignalCandidate { session_id, .. } => check(id_ok(session_id)),
            ViewerStats {
                session_id,
                reconnect_ms,
                ..
            } => check(id_ok(session_id) && reconnect_ms.len() <= RECONNECT_LOG_MAX),
            Error { .. } => Ok(()),
        }
    }
}

// ---- DataChannel (뷰어↔도우미). JSON 글 — decision-log DL-12. ----

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

/// 채널 `input` (D-01·D-02·D-03). 좌표는 모니터 기준 0~1.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum Input {
    MouseMove {
        seq: u32,
        x: f32,
        y: f32,
    },
    MouseButton {
        seq: u32,
        button: MouseButton,
        down: bool,
    },
    Wheel {
        seq: u32,
        dx: f32,
        dy: f32,
    },
    /// USB HID 키보드 페이지(0x07) 사용 번호.
    Key {
        seq: u32,
        hid_usage: u16,
        down: bool,
    },
    /// 완성 문자열(한글 조합이 끝난 글자).
    Text {
        seq: u32,
        text: String,
    },
}

/// 한 번에 보내는 글자 수 상한.
pub const INPUT_TEXT_MAX: usize = 1024;

impl Validate for Input {
    fn validate(&self) -> Result<(), ErrorType> {
        let unit = |v: f32| (0.0..=1.0).contains(&v);
        match self {
            Input::MouseMove { x, y, .. } => check(unit(*x) && unit(*y)),
            Input::Wheel { dx, dy, .. } => check(dx.is_finite() && dy.is_finite()),
            Input::MouseButton { .. } | Input::Key { .. } => Ok(()),
            Input::Text { text, .. } => {
                check(!text.is_empty() && text.chars().count() <= INPUT_TEXT_MAX)
            }
        }
    }
}

/// 채널 `control` (이번 범위는 D-08 만).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "t")]
pub enum Control {
    /// D-08 도우미→뷰어: 입력 반영 뒤 첫 인코딩 프레임의 RTP 타임스탬프.
    #[serde(rename = "input.applied")]
    InputApplied { seq: u32, rtp_ts: u32 },
}

impl Validate for Control {
    fn validate(&self) -> Result<(), ErrorType> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn problem_uses_snake_case_type_and_status() {
        let p = Problem::new(ErrorType::CodeExpired, None);
        let v = serde_json::to_value(&p).unwrap();
        assert_eq!(v["type"], "code_expired");
        assert_eq!(v["status"], 400);
        let back: Problem = serde_json::from_value(v).unwrap();
        assert_eq!(back, p);
    }

    #[test]
    fn ws_tags_match_contract() {
        let cases = [
            (json!({"t":"hello","nonce":"abc","proto":1}), "hello"),
            (
                json!({"t":"signal.candidate","session_id":"s1","candidate":"candidate:1 1 udp 1 1.2.3.4 5 typ host"}),
                "signal.candidate",
            ),
            (
                json!({"t":"session.end","session_id":"s1","reason":"taken_over"}),
                "session.end",
            ),
            (
                json!({"t":"session.viewer_stats","session_id":"s1","click_to_first_frame_ms":1200,
                       "latency_p50_ms":null,"reconnect_ms":[300],"restarts":1,"new_session_used":false,"path":"relay"}),
                "session.viewer_stats",
            ),
            (
                json!({"t":"error","type":"device_offline","detail":null}),
                "error",
            ),
        ];
        for (v, t) in cases {
            let m: WsMsg = parse(&v.to_string()).unwrap_or_else(|e| panic!("{t}: {e:?}"));
            assert_eq!(serde_json::to_value(&m).unwrap(), v, "{t} 왕복");
        }
    }

    #[test]
    fn parse_rejects_bad_shapes() {
        assert_eq!(
            parse::<WsMsg>(&"x".repeat(WS_MSG_MAX_BYTES + 1)),
            Err(ErrorType::TooLarge)
        );
        assert_eq!(
            parse::<WsMsg>("{\"t\":\"nope\"}"),
            Err(ErrorType::BadRequest)
        );
        assert_eq!(
            parse::<WsMsg>(r#"{"t":"viewer.hello","viewer_id":"a b","proto":1}"#),
            Err(ErrorType::BadRequest)
        );
        assert_eq!(
            parse::<SessionReq>(r#"{"device_id":"","viewer_id":"v"}"#),
            Err(ErrorType::BadRequest)
        );
    }

    #[test]
    fn enroll_req_checks_fields() {
        let ok = EnrollReq {
            code: "ABCD2345".into(),
            pubkey: format!("{}=", "A".repeat(43)),
            name: "집 PC".into(),
            os: Os::Windows,
            agent_version: "0.1.0".into(),
        };
        assert_eq!(ok.validate(), Ok(()));
        let bad = |f: fn(&mut EnrollReq)| {
            let mut r = ok.clone();
            f(&mut r);
            r.validate()
        };
        assert!(bad(|r| {
            r.pubkey.pop();
        })
        .is_err());
        assert!(bad(|r| r.name = "가".repeat(NAME_MAX + 1)).is_err());
        assert!(bad(|r| r.name = "a\nb".into()).is_err());
        assert!(bad(|r| r.code = "ABCD23456".into()).is_err());
        assert!(parse::<EnrollReq>(
            r#"{"code":"A","pubkey":"","name":"n","os":"linux","agent_version":"1"}"#
        )
        .is_err());
    }

    #[test]
    fn reg_code_alphabet_has_no_lookalikes() {
        assert_eq!(REG_CODE_ALPHABET.len(), 32);
        assert!(reg_code_ok("ABCD2345"));
        for bad in ["ABCD234O", "ABCD2341", "ABCD234I", "abcd2345", "ABC"] {
            assert!(!reg_code_ok(bad), "{bad}");
        }
    }

    #[test]
    fn input_checks_range_and_text() {
        let m: Input = parse(r#"{"t":"mouse_move","seq":1,"x":0.5,"y":1.0}"#).unwrap();
        assert_eq!(
            m,
            Input::MouseMove {
                seq: 1,
                x: 0.5,
                y: 1.0
            }
        );
        assert!(parse::<Input>(r#"{"t":"mouse_move","seq":1,"x":1.5,"y":0}"#).is_err());
        assert!(parse::<Input>(r#"{"t":"text","seq":2,"text":""}"#).is_err());
        let t: Input = parse(r#"{"t":"text","seq":3,"text":"안녕하세요 원격 테스트"}"#).unwrap();
        assert!(matches!(t, Input::Text { .. }));
        let c: Control = parse(r#"{"t":"input.applied","seq":3,"rtp_ts":90000}"#).unwrap();
        assert_eq!(
            c,
            Control::InputApplied {
                seq: 3,
                rtp_ts: 90000
            }
        );
    }
}
