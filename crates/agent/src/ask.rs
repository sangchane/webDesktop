//! 상주 계층 — Claude Code 권한 요청(FR-023, DL-24). 훅의 curl 이 127.0.0.1 로 보낸 요청을 카드로 바꿔 허브에 내고,
//! 웹앱의 답을 훅 출력으로 돌려준다. 답이 없으면 빈 본문(결정 없음 → Claude Code 가 터미널에서 묻는다).
//! 훅 꼴: https://code.claude.com/docs/en/hooks (PermissionRequest).
use proto::{WsMsg, ASK_BODY_MAX, ASK_DETAIL_MAX, ASK_LABEL_MAX, ASK_PENDING_MAX, ASK_WAIT_SEC};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, oneshot};
use tokio::time::{timeout, Instant};
use tracing::{info, warn};

/// 훅이 보낸 요청을 다 읽기까지 기다리는 시간.
const READ_SEC: u64 = 5;
/// 머리글 상한 — 넘으면 HTTP 요청이 아닌 것으로 본다.
const HEAD_MAX: usize = 8192;
const DENY_MESSAGE: &str = "폰에서 거부했습니다";

/// 카드에 보일 것 — 명령 원문은 허브를 지나 웹앱으로만 간다(로그 금지).
#[derive(Debug, Clone, PartialEq)]
pub struct Card {
    pub project: String,
    pub tool: String,
    pub detail: String,
    /// detail 을 잘랐다 — 폰에서는 허용할 수 없다.
    pub truncated: bool,
}

/// 앞에서 max 글자. 잘랐으면 true.
fn clip(s: &str, max: usize) -> (String, bool) {
    if s.chars().count() <= max {
        return (s.to_owned(), false);
    }
    let mut t: String = s.chars().take(max - 1).collect();
    t.push('…');
    (t, true)
}

/// 보이지 않거나 글 방향을 바꾸는 글자(Trojan Source)를 「⟨U+202E⟩」처럼 드러낸다 — 폰에 보인 글 = 실행될 글.
fn visible(s: &str) -> String {
    s.chars()
        .map(|c| match c as u32 {
            0x00AD
            | 0x061C
            | 0x180E
            | 0x200B..=0x200F
            | 0x2028..=0x202E
            | 0x2060..=0x206F
            | 0xFEFF
            | 0xFFF9..=0xFFFB => format!("⟨U+{:04X}⟩", c as u32),
            _ => c.to_string(),
        })
        .collect()
}

fn label(s: &str) -> String {
    clip(
        &visible(&s.replace(|c: char| c.is_control(), " ")),
        ASK_LABEL_MAX,
    )
    .0
}

/// 훅 입력(stdin JSON) → 카드. tool_name 이 없으면 훅 입력이 아니다.
pub fn card(hook: &Value) -> Option<Card> {
    let tool = hook["tool_name"].as_str().filter(|t| !t.is_empty())?;
    let cwd = hook["cwd"].as_str().unwrap_or("");
    let project = cwd
        .trim_end_matches(['/', '\\'])
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or("");
    let input = &hook["tool_input"];
    // 아는 도구만 한 칸을 보인다. 나머지(MCP 등)는 입력 전체 — 한 칸만 보이면 다른 칸의 위험을 못 본다.
    let field = match tool {
        "Bash" | "PowerShell" => "command",
        "Edit" | "MultiEdit" | "Write" | "Read" | "NotebookEdit" => "file_path",
        "WebFetch" => "url",
        _ => "",
    };
    let detail = input[field]
        .as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| input.to_string());
    let (detail, truncated) = clip(&visible(&detail), ASK_DETAIL_MAX);
    Some(Card {
        project: label(project),
        tool: label(tool),
        detail,
        truncated,
    })
}

/// 답 → 훅 출력. None 은 빈 본문(결정 없음).
pub fn hook_output(allow: Option<bool>) -> String {
    let decision = match allow {
        None => return String::new(),
        Some(true) => json!({ "behavior": "allow" }),
        Some(false) => json!({ "behavior": "deny", "message": DENY_MESSAGE }),
    };
    json!({ "hookSpecificOutput": { "hookEventName": "PermissionRequest", "decision": decision } })
        .to_string()
}

/// 받은 바이트에서 HTTP 요청 하나. Ok(None) = 더 읽어야 함, Err = 돌려줄 상태 번호.
/// 신뢰 경계: `POST /ask` · `x-webdesktop: 1` 필수 · Origin 거절(브라우저 페이지가 카드를 못 만들게) · 본문 상한.
pub fn read_request(buf: &[u8]) -> Result<Option<&[u8]>, u16> {
    let Some(end) = buf.windows(4).position(|w| w == b"\r\n\r\n") else {
        return if buf.len() > HEAD_MAX {
            Err(431)
        } else {
            Ok(None)
        };
    };
    let head = std::str::from_utf8(&buf[..end]).map_err(|_| 400u16)?;
    let mut lines = head.split("\r\n");
    if !lines.next().is_some_and(|l| l.starts_with("POST /ask ")) {
        return Err(404);
    }
    let (mut marked, mut len) = (false, None);
    for l in lines {
        let Some((k, v)) = l.split_once(':') else {
            return Err(400);
        };
        match k.trim().to_ascii_lowercase().as_str() {
            "origin" => return Err(403),
            "x-webdesktop" => marked = v.trim() == "1",
            "content-length" => len = Some(v.trim().parse::<usize>().map_err(|_| 400u16)?),
            _ => {}
        }
    }
    if !marked {
        return Err(403);
    }
    let len = len.ok_or(411u16)?;
    if len > ASK_BODY_MAX {
        return Err(413);
    }
    let body = &buf[end + 4..];
    Ok((body.len() >= len).then(|| &body[..len]))
}

/// 입구 → 장부.
pub enum Ev {
    New {
        id: String,
        card: Card,
        /// 훅이 온 시각 — 허브가 끊겨 늦게 꺼내도 기다림은 여기서 잰다.
        at: Instant,
        reply: oneshot::Sender<Option<bool>>,
    },
    /// 훅 쪽이 기다림을 끝냄(시간 초과·curl 끊김).
    Gone(String),
}

struct Pending {
    card: Card,
    until: Instant,
    reply: oneshot::Sender<Option<bool>>,
}

/// 기다리는 요청 장부 — 허브 재접속을 넘어 산다(허브는 저장하지 않는다, DL-24).
pub struct Asks {
    pending: HashMap<String, Pending>,
    pub tx: mpsc::UnboundedSender<Ev>,
    pub rx: mpsc::UnboundedReceiver<Ev>,
}

fn new_msg(id: &str, device_id: &str, p: &Pending, now: Instant) -> Option<WsMsg> {
    let left = p
        .until
        .saturating_duration_since(now)
        .as_millis()
        .div_ceil(1000) as u32;
    (left > 0).then(|| WsMsg::AskNew {
        ask_id: id.to_owned(),
        device_id: device_id.to_owned(),
        project: p.card.project.clone(),
        tool: p.card.tool.clone(),
        detail: p.card.detail.clone(),
        truncated: p.card.truncated,
        wait_sec: left.min(ASK_WAIT_SEC),
    })
}

impl Asks {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        Self {
            pending: HashMap::new(),
            tx,
            rx,
        }
    }

    /// 입구 사건 → 허브로 보낼 글.
    pub fn on_ev(&mut self, ev: Ev, device_id: &str) -> Option<WsMsg> {
        match ev {
            Ev::New {
                id,
                card,
                at,
                reply,
            } => {
                // 허브가 끊긴 사이 훅이 이미 포기했으면 카드를 내지 않는다.
                if reply.is_closed() {
                    return None;
                }
                if self.pending.len() >= ASK_PENDING_MAX {
                    let _ = reply.send(None);
                    return None;
                }
                info!(event = "ask.new", ask_id = %id, tool = %card.tool);
                let p = Pending {
                    card,
                    until: at + Duration::from_secs(ASK_WAIT_SEC as u64),
                    reply,
                };
                let m = new_msg(&id, device_id, &p, Instant::now());
                self.pending.insert(id, p);
                m
            }
            Ev::Gone(id) => self.pending.remove(&id).map(|_| WsMsg::AskDone {
                ask_id: id,
                device_id: device_id.to_owned(),
            }),
        }
    }

    /// 허브 → 에이전트(ask.*). 허브로 다시 보낼 글을 돌려준다.
    pub fn on_hub(&mut self, m: WsMsg, device_id: &str) -> Vec<WsMsg> {
        match m {
            WsMsg::AskAnswer { ask_id, allow, .. } => match self.pending.remove(&ask_id) {
                Some(p) => {
                    // 잘린 명령은 폰이 허용 단추를 막는다 — 그래도 온 allow 는 결정 없음으로(터미널에서 전체를 본다).
                    let allow = if allow && p.card.truncated {
                        warn!(event = "ask.allow_truncated", ask_id = %ask_id);
                        None
                    } else {
                        Some(allow)
                    };
                    info!(event = "ask.answered", ask_id = %ask_id, allow = ?allow);
                    let _ = p.reply.send(allow);
                    vec![WsMsg::AskDone {
                        ask_id,
                        device_id: device_id.to_owned(),
                    }]
                }
                // 이미 끝난 요청 — 다른 폰의 카드도 지우게 끝났다고 다시 알린다.
                None => vec![WsMsg::AskDone {
                    ask_id,
                    device_id: device_id.to_owned(),
                }],
            },
            // 허브: 웹앱이 열린 뷰어가 없다 → 곧바로 터미널 창으로.
            WsMsg::AskDone { ask_id, .. } => {
                if let Some(p) = self.pending.remove(&ask_id) {
                    info!(event = "ask.no_viewer", ask_id = %ask_id);
                    let _ = p.reply.send(None);
                }
                vec![]
            }
            WsMsg::AskSync {} => self.resend(device_id),
            _ => vec![],
        }
    }

    /// 허브와 끊긴 동안 — 새 요청은 곧바로 결정 없음(터미널 창), 시간 초과는 장부에서 뺀다.
    pub fn offline(&mut self, ev: Ev) {
        match ev {
            Ev::New { reply, .. } => {
                let _ = reply.send(None);
            }
            Ev::Gone(id) => {
                self.pending.remove(&id);
            }
        }
    }

    /// 허브의 ask.sync(인증 뒤·새 뷰어) — 기다리는 요청을 남은 초로 다시 보낸다.
    pub fn resend(&self, device_id: &str) -> Vec<WsMsg> {
        let now = Instant::now();
        self.pending
            .iter()
            .filter_map(|(id, p)| new_msg(id, device_id, p, now))
            .collect()
    }
}

/// 훅 명령 — 에이전트 실행 파일 경로에 기대지 않게 curl(Windows 10+ 기본). 에이전트가 꺼져 있으면 curl 이 실패하고 Claude Code 는 평소대로 묻는다.
pub fn hook_command(port: u16) -> String {
    format!(
        "curl -s -m {} -H \"x-webdesktop: 1\" --data-binary @- http://127.0.0.1:{port}/ask",
        proto::ASK_HOOK_TIMEOUT_SEC - 5
    )
}

/// settings.json 값에 우리 PermissionRequest 훅을 넣는다. 이미 있으면 false(다른 훅은 그대로).
pub fn add_hook(settings: &mut Value, port: u16) -> Result<bool, String> {
    let root = settings
        .as_object_mut()
        .ok_or("settings.json 이 객체가 아닙니다")?;
    let list = root
        .entry("hooks")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or("hooks 가 객체가 아닙니다")?
        .entry("PermissionRequest")
        .or_insert_with(|| json!([]))
        .as_array_mut()
        .ok_or("hooks.PermissionRequest 가 배열이 아닙니다")?;
    let ours = |h: &Value| {
        h["command"]
            .as_str()
            .is_some_and(|c| c.contains("x-webdesktop") && c.contains("/ask"))
    };
    if list
        .iter()
        .any(|m| m["hooks"].as_array().is_some_and(|hs| hs.iter().any(ours)))
    {
        return Ok(false);
    }
    list.push(json!({
        "matcher": "*",
        "hooks": [{ "type": "command", "command": hook_command(port), "timeout": proto::ASK_HOOK_TIMEOUT_SEC }]
    }));
    Ok(true)
}

/// `webdesktop-agent --claude-hook` — 이 사용자의 Claude Code 설정에 훅을 넣는다. 바꾸기 전 `.webdesktop.bak` 로 보관.
pub fn install_hook() -> Result<String, String> {
    let dir = std::env::var_os("CLAUDE_CONFIG_DIR")
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::env::var_os("USERPROFILE").map(|h| std::path::PathBuf::from(h).join(".claude"))
        })
        .ok_or("사용자 폴더를 모릅니다")?;
    let path = dir.join("settings.json");
    let mut v: Value = match std::fs::read_to_string(&path) {
        Ok(t) => {
            serde_json::from_str(&t).map_err(|e| format!("{} 꼴이 틀림: {e}", path.display()))?
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => json!({}),
        Err(e) => return Err(format!("{} 읽기: {e}", path.display())),
    };
    if !add_hook(&mut v, proto::ASK_PORT)? {
        return Ok(format!("이미 들어 있습니다: {}", path.display()));
    }
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    if path.exists() {
        std::fs::copy(&path, path.with_extension("json.webdesktop.bak"))
            .map_err(|e| format!("보관 실패: {e}"))?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(
        &tmp,
        serde_json::to_string_pretty(&v).map_err(|e| e.to_string())?,
    )
    .and_then(|_| std::fs::rename(&tmp, &path))
    .map_err(|e| format!("{} 쓰기: {e}", path.display()))?;
    Ok(format!("넣었습니다: {}", path.display()))
}

fn new_id() -> String {
    let mut b = [0u8; 16];
    let _ = getrandom::fill(&mut b);
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// 127.0.0.1 입구. 포트를 못 열면(에이전트 둘) 이 기능만 끈다.
pub async fn serve(port: u16, tx: mpsc::UnboundedSender<Ev>) {
    let l = match TcpListener::bind(("127.0.0.1", port)).await {
        Ok(l) => l,
        Err(e) => {
            warn!(event = "ask.listen_failed", port, why = %e);
            return;
        }
    };
    info!(event = "ask.listening", port);
    loop {
        match l.accept().await {
            Ok((s, _)) => {
                tokio::spawn(handle(s, tx.clone()));
            }
            Err(e) => warn!(event = "ask.accept_failed", why = %e),
        }
    }
}

async fn handle(mut s: TcpStream, tx: mpsc::UnboundedSender<Ev>) {
    let mut buf = Vec::new();
    let body = loop {
        let mut chunk = [0u8; 4096];
        let n = match timeout(Duration::from_secs(READ_SEC), s.read(&mut chunk)).await {
            Ok(Ok(n)) if n > 0 => n,
            _ => return,
        };
        buf.extend_from_slice(&chunk[..n]);
        match read_request(&buf) {
            Ok(None) => continue,
            Ok(Some(b)) => break b.to_vec(),
            Err(code) => return respond(&mut s, code, "").await,
        }
    };
    let Some(card) = serde_json::from_slice::<Value>(&body)
        .ok()
        .as_ref()
        .and_then(card)
    else {
        return respond(&mut s, 400, "").await;
    };
    let id = new_id();
    let (reply, rx) = oneshot::channel();
    if tx
        .send(Ev::New {
            id: id.clone(),
            card,
            at: Instant::now(),
            reply,
        })
        .is_err()
    {
        return respond(&mut s, 200, "").await;
    }
    let mut probe = [0u8; 1];
    let allow = tokio::select! {
        r = timeout(Duration::from_secs(ASK_WAIT_SEC as u64), rx) => match r {
            Ok(Ok(a)) => a,
            _ => {
                let _ = tx.send(Ev::Gone(id));
                None
            }
        },
        // curl 이 먼저 끊음(Claude Code 에서 Esc) — 카드를 거둔다.
        _ = s.read(&mut probe) => {
            let _ = tx.send(Ev::Gone(id));
            return;
        }
    };
    respond(&mut s, 200, &hook_output(allow)).await;
}

async fn respond(s: &mut TcpStream, status: u16, body: &str) {
    let r = format!(
        "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = s.write_all(r.as_bytes()).await;
    let _ = s.shutdown().await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn card_from_hook_input() {
        // 공식 문서 예시 입력.
        let bash = json!({"session_id":"abc123","cwd":"C:\\work\\WebDesktop\\","permission_mode":"default",
            "hook_event_name":"PermissionRequest","tool_name":"Bash",
            "tool_input":{"command":"rm -rf node_modules","description":"Remove node_modules directory"}});
        assert_eq!(
            card(&bash),
            Some(Card {
                project: "WebDesktop".into(),
                tool: "Bash".into(),
                detail: "rm -rf node_modules".into(),
                truncated: false,
            })
        );
        let edit = json!({"cwd":"/home/u/app","tool_name":"Edit","tool_input":{"file_path":"/home/u/app/a.rs","old_string":"x"}});
        assert_eq!(card(&edit).unwrap().detail, "/home/u/app/a.rs");
        // 모르는 도구는 한 칸이 아니라 입력 전체 — url 옆의 sql 도 보인다.
        let mcp = json!({"cwd":"","tool_name":"mcp__db__run","tool_input":{"url":"u","sql":"DROP TABLE x"}});
        assert_eq!(
            card(&mcp).unwrap().detail,
            r#"{"url":"u","sql":"DROP TABLE x"}"#
        );
        let fetch = json!({"tool_name":"WebFetch","tool_input":{"url":"https://a","prompt":"p"}});
        assert_eq!(card(&fetch).unwrap().detail, "https://a");
        // 글 방향 바꿈·보이지 않는 글자는 드러낸다(Trojan Source).
        let bidi =
            json!({"tool_name":"Bash","tool_input":{"command":"echo ok\u{202E}fr- mr\u{200B}"}});
        assert_eq!(card(&bidi).unwrap().detail, "echo ok⟨U+202E⟩fr- mr⟨U+200B⟩");
        assert_eq!(card(&json!({"cwd":"/a"})), None);
        let long =
            json!({"tool_name":"Bash","tool_input":{"command":"가".repeat(ASK_DETAIL_MAX + 5)}});
        let d = card(&long).unwrap();
        assert_eq!(d.detail.chars().count(), ASK_DETAIL_MAX);
        assert!(d.detail.ends_with('…') && d.truncated);
        assert!(!card(&bash).unwrap().truncated);
        let ctl = json!({"cwd":"/a/b\nc","tool_name":"Ba\tsh","tool_input":{}});
        let c = card(&ctl).unwrap();
        assert!(!c.project.contains('\n') && !c.tool.contains('\t'));
    }

    #[test]
    fn hook_output_matches_docs_shape() {
        assert_eq!(hook_output(None), "");
        let allow: Value = serde_json::from_str(&hook_output(Some(true))).unwrap();
        assert_eq!(
            allow,
            json!({"hookSpecificOutput":{"hookEventName":"PermissionRequest","decision":{"behavior":"allow"}}})
        );
        let deny: Value = serde_json::from_str(&hook_output(Some(false))).unwrap();
        assert_eq!(deny["hookSpecificOutput"]["decision"]["behavior"], "deny");
        assert_eq!(
            deny["hookSpecificOutput"]["decision"]["message"],
            DENY_MESSAGE
        );
    }

    #[test]
    fn request_reader_guards_the_door() {
        let ok = b"POST /ask HTTP/1.1\r\nHost: 127.0.0.1\r\nX-WebDesktop: 1\r\nContent-Length: 2\r\n\r\n{}";
        assert_eq!(read_request(ok), Ok(Some(&b"{}"[..])));
        assert_eq!(read_request(&ok[..ok.len() - 1]), Ok(None));
        assert_eq!(read_request(b"POST /ask HTTP/1.1\r\nHost: x"), Ok(None));
        assert_eq!(
            read_request(b"POST /ask HTTP/1.1\r\nContent-Length: 2\r\n\r\n{}"),
            Err(403)
        );
        assert_eq!(
            read_request(b"POST /ask HTTP/1.1\r\nx-webdesktop: 1\r\nOrigin: https://evil\r\nContent-Length: 2\r\n\r\n{}"),
            Err(403)
        );
        assert_eq!(
            read_request(b"GET /ask HTTP/1.1\r\nx-webdesktop: 1\r\n\r\n"),
            Err(404)
        );
        let big = format!(
            "POST /ask HTTP/1.1\r\nx-webdesktop: 1\r\nContent-Length: {}\r\n\r\n",
            ASK_BODY_MAX + 1
        );
        assert_eq!(read_request(big.as_bytes()), Err(413));
        assert_eq!(
            read_request(b"POST /ask HTTP/1.1\r\nx-webdesktop: 1\r\n\r\n"),
            Err(411)
        );
        assert_eq!(read_request(&[b'a'; HEAD_MAX + 1]), Err(431));
    }

    #[test]
    fn hook_install_keeps_other_hooks_and_is_idempotent() {
        let mut v = json!({"model":"x","hooks":{"PermissionRequest":[{"matcher":"Bash","hooks":[{"type":"command","command":"other"}]}],
            "Stop":[{"hooks":[{"type":"command","command":"s"}]}]}});
        assert_eq!(add_hook(&mut v, 47913), Ok(true));
        assert_eq!(add_hook(&mut v, 47913), Ok(false));
        let list = v["hooks"]["PermissionRequest"].as_array().unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0]["hooks"][0]["command"], "other");
        assert_eq!(list[1]["hooks"][0]["timeout"], proto::ASK_HOOK_TIMEOUT_SEC);
        assert_eq!(
            list[1]["hooks"][0]["command"],
            "curl -s -m 115 -H \"x-webdesktop: 1\" --data-binary @- http://127.0.0.1:47913/ask"
        );
        assert_eq!(v["hooks"]["Stop"][0]["hooks"][0]["command"], "s");
        assert_eq!(v["model"], "x");
        let mut empty = json!({});
        assert_eq!(add_hook(&mut empty, 1), Ok(true));
        assert!(add_hook(&mut json!({"hooks":[]}), 1).is_err());
    }

    fn c(truncated: bool) -> Card {
        Card {
            project: "p".into(),
            tool: "Bash".into(),
            detail: "ls".into(),
            truncated,
        }
    }

    fn new(id: &str, card: Card) -> (Ev, oneshot::Receiver<Option<bool>>) {
        let (reply, rx) = oneshot::channel();
        let ev = Ev::New {
            id: id.into(),
            card,
            at: Instant::now(),
            reply,
        };
        (ev, rx)
    }

    fn answer(id: &str, allow: bool) -> WsMsg {
        WsMsg::AskAnswer {
            ask_id: id.into(),
            device_id: "d1".into(),
            allow,
        }
    }

    #[tokio::test]
    async fn ledger_answers_syncs_and_limits() {
        let mut a = Asks::new();
        let (ev, rx) = new("a1", c(false));
        let m = a.on_ev(ev, "d1");
        assert!(
            matches!(m, Some(WsMsg::AskNew { ref ask_id, wait_sec, .. }) if ask_id == "a1" && wait_sec == ASK_WAIT_SEC)
        );
        // 새 뷰어·재인증 → 다시 보냄
        assert_eq!(a.on_hub(WsMsg::AskSync {}, "d1").len(), 1);
        let out = a.on_hub(answer("a1", false), "d1");
        assert!(matches!(&out[..], [WsMsg::AskDone { ask_id, .. }] if ask_id == "a1"));
        assert_eq!(rx.await.unwrap(), Some(false));
        assert!(a.on_hub(WsMsg::AskSync {}, "d1").is_empty());
        // 늦은 두 번째 답은 아무것도 허용하지 않는다
        assert!(matches!(
            &a.on_hub(answer("a1", true), "d1")[..],
            [WsMsg::AskDone { .. }]
        ));

        // 뷰어 없음 → 결정 없음
        let (ev, rx) = new("a2", c(false));
        a.on_ev(ev, "d1");
        let done = WsMsg::AskDone {
            ask_id: "a2".into(),
            device_id: "d1".into(),
        };
        assert!(a.on_hub(done, "d1").is_empty());
        assert_eq!(rx.await.unwrap(), None);

        // 잘린 명령에 온 allow → 결정 없음(터미널에서 전체를 본다), deny 는 그대로
        let (ev, rx) = new("t1", c(true));
        a.on_ev(ev, "d1");
        a.on_hub(answer("t1", true), "d1");
        assert_eq!(rx.await.unwrap(), None);
        let (ev, rx) = new("t2", c(true));
        a.on_ev(ev, "d1");
        a.on_hub(answer("t2", false), "d1");
        assert_eq!(rx.await.unwrap(), Some(false));

        // 훅이 이미 포기한 요청은 내지 않는다
        let (ev, rx) = new("a3", c(false));
        drop(rx);
        assert!(a.on_ev(ev, "d1").is_none());

        // 허브와 끊긴 동안 → 곧바로 결정 없음
        let (ev, rx) = new("off", c(false));
        a.offline(ev);
        assert_eq!(rx.await.unwrap(), None);

        // 상한
        let mut keep = vec![];
        for i in 0..ASK_PENDING_MAX {
            let (ev, rx) = new(&format!("b{i}"), c(false));
            keep.push(rx);
            assert!(a.on_ev(ev, "d1").is_some());
        }
        let (ev, rx) = new("over", c(false));
        assert!(a.on_ev(ev, "d1").is_none());
        assert_eq!(rx.await.unwrap(), None);
        // 시간 초과 → 뷰어 카드 지움
        assert!(matches!(
            a.on_ev(Ev::Gone("b0".into()), "d1"),
            Some(WsMsg::AskDone { .. })
        ));
        assert!(a.on_ev(Ev::Gone("b0".into()), "d1").is_none());
    }
}
