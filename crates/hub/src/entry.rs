//! 입구 계층 — DO `Hub` 의 경로·인증(Access JWT·기기 서명)·빈도 제한·꼴 검사. 통과한 값만 service·store 로 (04 「코드 구조」).
use crate::access::{Access, JwksCache};
use crate::crypto::{ed25519_verify, random};
use crate::service::{self, CodeVerdict, RateLimit};
use crate::store::{DeviceRow, NewDevice, Store};
use base64::engine::general_purpose::{STANDARD as B64, URL_SAFE_NO_PAD as B64URL};
use base64::Engine;
use proto::{
    parse, Device, EnrollCode, EnrollReq, EnrollResp, ErrorType, InstallLinks, Os, Perm, WsMsg,
    PROTO_MAJOR, REG_CODE_LEN, WS_MSG_MAX_BYTES,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::cell::RefCell;
use worker::js_sys::{Date, Function, Reflect};
use worker::wasm_bindgen::{JsCast, JsValue};
use worker::*;

const TAG_AGENT: &str = "agent";
/// 같은 기기가 다시 붙어 앞 연결을 닫을 때·오래 조용해 닫을 때의 닫힘 코드(응용 영역 4000~).
const CLOSE_REPLACED: u16 = 4000;
const CLOSE_STALE: u16 = 4001;
const CLOSE_REJECTED: u16 = 4002;
/// 인증 전 소켓 상한 — 넘으면 새 /agent/ws 를 rate_limited 로 거절.
const PENDING_MAX: usize = 16;

/// 에이전트 WebSocket 에 붙여 두는 상태 — 휴면 뒤에도 남는다.
#[derive(Serialize, Deserialize)]
struct AgentAtt {
    nonce: String,
    device_id: Option<String>,
    opened_at: i64,
    /// 허브가 「조용함」으로 닫는 중 — 닫힘 이벤트가 last_seen_at 을 지금으로 덮지 않게.
    #[serde(default)]
    stale: bool,
}

#[durable_object]
pub struct Hub {
    state: State,
    env: Env,
    store: Store,
    limiter: RefCell<RateLimit>,
    jwks: JwksCache,
}

/// 요청마다 읽는 설정. 빠지면 500 `internal`(SPEC 바닥선 8).
struct Config {
    access: Access,
    hub_url: String,
    /// 시험 모드(ACCESS_TEST_JWKS 가 있고 로컬 주소) — `x-test-now` 로 시계를 옮길 수 있다.
    test: bool,
}

fn log(level: &str, event: &str, fields: serde_json::Value) {
    let mut v = json!({ "level": level, "event": event });
    if let (Some(o), Some(f)) = (v.as_object_mut(), fields.as_object()) {
        o.extend(f.clone());
    }
    match level {
        "error" => console_error!("{v}"),
        "warn" => console_warn!("{v}"),
        _ => console_log!("{v}"),
    }
}

fn now() -> i64 {
    Date::now() as i64
}

fn iso(ms: i64) -> String {
    Date::new(&JsValue::from_f64(ms as f64))
        .to_iso_string()
        .into()
}

fn json_status<T: Serialize>(v: &T, status: u16) -> Result<Response> {
    Ok(Response::from_json(v)?.with_status(status))
}

fn ws_error(ws: &WebSocket, kind: ErrorType, close: bool) {
    let _ = ws.send(&WsMsg::Error { kind, detail: None });
    if close {
        let _ = ws.close(
            Some(CLOSE_REJECTED),
            Some(serde_json::to_string(&kind).unwrap_or_default()),
        );
    }
}

impl Hub {
    fn config(&self, req: &Request) -> std::result::Result<Config, ErrorType> {
        let var = |k: &str| {
            self.env
                .var(k)
                .ok()
                .map(|v| v.to_string())
                .filter(|v| !v.is_empty())
        };
        let (Some(team_domain), Some(aud), Some(hub_url)) = (
            var("ACCESS_TEAM_DOMAIN"),
            var("ACCESS_AUD"),
            var("HUB_PUBLIC_URL"),
        ) else {
            log(
                "error",
                "internal_error",
                json!({ "detail": "config missing" }),
            );
            return Err(ErrorType::Internal);
        };
        let test_jwks = var("ACCESS_TEST_JWKS");
        let local = matches!(
            req.url()
                .ok()
                .and_then(|u| u.host_str().map(str::to_owned))
                .as_deref(),
            Some("localhost" | "127.0.0.1")
        );
        if test_jwks.is_some() && !local {
            log(
                "error",
                "internal_error",
                json!({ "detail": "ACCESS_TEST_JWKS outside localhost" }),
            );
            return Err(ErrorType::Internal);
        }
        Ok(Config {
            access: Access {
                team_domain,
                aud,
                test_jwks: test_jwks.clone(),
            },
            hub_url,
            test: test_jwks.is_some(),
        })
    }

    fn agent_sockets(&self) -> Vec<(WebSocket, AgentAtt)> {
        self.state
            .get_websockets_with_tag(TAG_AGENT)
            .into_iter()
            .filter_map(|ws| {
                let att = ws.deserialize_attachment::<AgentAtt>().ok().flatten()?;
                Some((ws, att))
            })
            .collect()
    }

    /// 휴면 중 DO 가 `ping` 에 자동 응답한 마지막 시각. workers-rs 에 감싼 함수가 없어 JS 로 부른다.
    fn auto_response_at(&self, ws: &WebSocket) -> Option<i64> {
        let raw: &JsValue = self.state.as_raw().as_ref();
        let f: Function = Reflect::get(raw, &"getWebSocketAutoResponseTimestamp".into())
            .ok()?
            .dyn_into()
            .ok()?;
        let d = f
            .call1(raw, AsRef::<worker::web_sys::WebSocket>::as_ref(ws))
            .ok()?;
        d.dyn_into::<Date>().ok().map(|d| d.get_time() as i64)
    }

    /// 오프라인 판정(05) — 조용한 연결은 닫고, 살아 있는 기기 id 목록을 돌려준다.
    fn sweep(&self, now_ms: i64) -> Result<Vec<String>> {
        let mut online = Vec::new();
        for (ws, att) in self.agent_sockets() {
            let Some(id) = att.device_id else {
                if service::is_stale(att.opened_at, now_ms) {
                    let _ = ws.close(Some(CLOSE_STALE), Some("auth timeout"));
                }
                continue;
            };
            if let Some(t) = self.auto_response_at(&ws) {
                self.store.device_seen(&id, t)?;
            }
            let last = self
                .store
                .device_get(&id)?
                .and_then(|d| d.last_seen_at)
                .unwrap_or(att.opened_at);
            if service::is_stale(last, now_ms) {
                let _ = ws.serialize_attachment(AgentAtt {
                    stale: true,
                    device_id: Some(id.clone()),
                    ..att
                });
                let _ = ws.close(Some(CLOSE_STALE), Some("stale"));
                log(
                    "info",
                    "agent.disconnected",
                    json!({ "device_id": id, "why": "stale" }),
                );
            } else {
                online.push(id);
            }
        }
        Ok(online)
    }

    async fn route(&self, req: Request) -> std::result::Result<Response, ErrorType> {
        let cfg = self.config(&req)?;
        let now_ms = match req.headers().get("x-test-now").ok().flatten() {
            Some(t) if cfg.test => t.parse().map_err(|_| ErrorType::BadRequest)?,
            _ => now(),
        };
        let ip = req
            .headers()
            .get("cf-connecting-ip")
            .ok()
            .flatten()
            .unwrap_or_else(|| "local".into());
        if !self.limiter.borrow_mut().hit(&ip, now_ms) {
            log("warn", "rate_limited", json!({}));
            return Err(ErrorType::RateLimited);
        }
        let path = req.path();
        let internal = |e: Error| {
            log(
                "error",
                "internal_error",
                json!({ "detail": e.to_string() }),
            );
            ErrorType::Internal
        };
        if path.starts_with("/api/") {
            let token = req.headers().get("cf-access-jwt-assertion").ok().flatten();
            cfg.access.check(token, now_ms, &self.jwks).await?;
        }
        match (req.method(), path.as_str()) {
            (Method::Post, "/api/enroll-codes") => self.new_code(&cfg, now_ms).map_err(internal),
            (Method::Get, "/api/devices") => self.devices(now_ms).map_err(internal),
            (Method::Post, "/agent/enroll") => self.enroll(req, now_ms).await,
            (Method::Get, "/agent/install/windows" | "/agent/install/macos") => {
                let token = req
                    .url()
                    .ok()
                    .and_then(|u| {
                        u.query_pairs()
                            .find(|(k, _)| k == "t")
                            .map(|(_, v)| v.into_owned())
                    })
                    .unwrap_or_default();
                let row = self.store.code_get().map_err(internal)?;
                service::judge_install(row.as_ref(), &token, now_ms)?;
                // shortcut: 설치 파일(R2 webdesktop-releases)은 에이전트 빌드(T8) 뒤에 붙인다 — 그때까지 토큰만 확인.
                Err(ErrorType::NotFound)
            }
            (Method::Get, "/agent/ws") => self.agent_ws(&req, now_ms).map_err(internal),
            _ => Err(ErrorType::NotFound),
        }
    }

    /// E-03 — 이전 활성 코드·링크를 버리고 새로 만든다.
    fn new_code(&self, cfg: &Config, now_ms: i64) -> Result<Response> {
        let code = service::make_code(&random::<REG_CODE_LEN>());
        let token = B64URL.encode(random::<32>());
        let expires_at = now_ms + service::CODE_TTL_MS;
        self.store
            .code_replace(&service::hash(&code), &service::hash(&token), expires_at)?;
        let link = |os: &str| format!("{}/agent/install/{os}?t={token}", cfg.hub_url);
        json_status(
            &EnrollCode {
                code,
                expires_at: iso(expires_at),
                install_links: InstallLinks {
                    windows: link("windows"),
                    macos: link("macos"),
                },
            },
            201,
        )
    }

    /// E-02.
    fn devices(&self, now_ms: i64) -> Result<Response> {
        let online = self.sweep(now_ms)?;
        let list: Vec<Device> = self
            .store
            .devices()?
            .into_iter()
            .map(|d| to_device(d, &online, now_ms))
            .collect();
        Response::from_json(&list)
    }

    /// E-04 — 05 「트랜잭션」 등록 순서 그대로.
    async fn enroll(
        &self,
        mut req: Request,
        now_ms: i64,
    ) -> std::result::Result<Response, ErrorType> {
        // 본문을 읽기 전에 크기부터 — 큰 본문이 DO 메모리를 채우지 못하게.
        let len: usize = req
            .headers()
            .get("content-length")
            .ok()
            .flatten()
            .and_then(|v| v.parse().ok())
            .ok_or(ErrorType::BadRequest)?;
        if len > WS_MSG_MAX_BYTES {
            return Err(ErrorType::TooLarge);
        }
        let body = req.text().await.map_err(|_| ErrorType::BadRequest)?;
        let r: EnrollReq = parse(&body)?;
        let internal = |e: Error| {
            log(
                "error",
                "internal_error",
                json!({ "detail": e.to_string() }),
            );
            ErrorType::Internal
        };
        let row = self.store.code_get().map_err(internal)?;
        let reject = |why: &str, kind: ErrorType| {
            log("warn", "enroll.rejected", json!({ "why": why }));
            Err(kind)
        };
        match service::judge_code(row.as_ref(), &r.code, now_ms) {
            CodeVerdict::NoCode => reject("no_code", ErrorType::CodeInvalid),
            CodeVerdict::Expired => {
                self.store.code_delete().map_err(internal)?;
                reject("expired", ErrorType::CodeExpired)
            }
            CodeVerdict::Wrong { tries, discard } => {
                if discard {
                    self.store.code_delete().map_err(internal)?;
                } else {
                    self.store.code_set_tries(tries).map_err(internal)?;
                }
                log(
                    "warn",
                    "enroll.rejected",
                    json!({ "why": "wrong", "tries": tries, "discarded": discard }),
                );
                Err(ErrorType::CodeInvalid)
            }
            CodeVerdict::Ok => {
                let pubkey = B64.decode(&r.pubkey).map_err(|_| ErrorType::BadRequest)?;
                if pubkey.len() != 32 || self.store.pubkey_taken(&r.pubkey).map_err(internal)? {
                    return Err(ErrorType::BadRequest);
                }
                let id = service::uuid_v7(now_ms, &random::<10>());
                let os = match r.os {
                    Os::Windows => "windows",
                    Os::Macos => "macos",
                };
                // 기기를 먼저 만든다 — 실패하면 코드는 남아 다시 시도할 수 있다.
                self.store
                    .device_insert(&NewDevice {
                        id: &id,
                        name: &r.name,
                        os,
                        pubkey: &r.pubkey,
                        agent_version: &r.agent_version,
                        now: now_ms,
                    })
                    .map_err(internal)?;
                self.store.code_delete().map_err(internal)?;
                log("info", "enroll.ok", json!({ "device_id": id }));
                json_status(&EnrollResp { device_id: id }, 201).map_err(internal)
            }
        }
    }

    /// E-05 1) — 받아들이고 nonce 를 보낸다. 서명 확인은 websocket_message 에서.
    fn agent_ws(&self, req: &Request, now_ms: i64) -> Result<Response> {
        let upgrade = req.headers().get("upgrade")?.unwrap_or_default();
        if !upgrade.eq_ignore_ascii_case("websocket") {
            return crate::problem(
                ErrorType::BadRequest,
                Some("websocket upgrade required".into()),
            );
        }
        let mut pending = 0;
        for (ws, att) in self.agent_sockets() {
            if att.device_id.is_none() {
                if service::is_stale(att.opened_at, now_ms) {
                    let _ = ws.close(Some(CLOSE_STALE), Some("auth timeout"));
                } else {
                    pending += 1;
                }
            }
        }
        if pending >= PENDING_MAX {
            log("warn", "rate_limited", json!({ "why": "pending_ws" }));
            return crate::problem(ErrorType::RateLimited, None);
        }
        let pair = WebSocketPair::new()?;
        self.state
            .accept_websocket_with_tags(&pair.server, &[TAG_AGENT]);
        let nonce = B64.encode(random::<32>());
        let att = AgentAtt {
            nonce: nonce.clone(),
            device_id: None,
            opened_at: now_ms,
            stale: false,
        };
        if let Err(e) = pair.server.serialize_attachment(att) {
            let _ = pair.server.close(Some(1011), Some("internal"));
            return Err(e);
        }
        pair.server.send(&WsMsg::Hello {
            nonce,
            proto: PROTO_MAJOR,
        })?;
        Response::from_websocket(pair.client)
    }

    /// E-05 2) — 첫 메시지는 auth 여야 한다.
    async fn agent_auth(&self, ws: &WebSocket, mut att: AgentAtt, msg: WsMsg) -> Result<()> {
        let WsMsg::Auth {
            device_id,
            sig,
            proto,
        } = msg
        else {
            ws_error(ws, ErrorType::Unauthorized, true);
            return Ok(());
        };
        let fail = |kind: ErrorType, why: &str| {
            log(
                "warn",
                "agent.auth_failed",
                json!({ "device_id": device_id, "why": why }),
            );
            ws_error(ws, kind, true);
            Ok(())
        };
        if proto != PROTO_MAJOR {
            return fail(ErrorType::UpgradeRequired, "proto");
        }
        // 모르는 기기도 bad_signature — 기기 id 가 있는지 알려 주지 않는다.
        let Some(dev) = self.store.device_get(&device_id)? else {
            return fail(ErrorType::BadSignature, "unknown");
        };
        let (Ok(pk), Ok(sig)) = (B64.decode(&dev.pubkey), B64.decode(&sig)) else {
            return fail(ErrorType::BadSignature, "b64");
        };
        if !ed25519_verify(&pk, &sig, &proto::auth_message(&att.nonce, &device_id)).await {
            return fail(ErrorType::BadSignature, "sig");
        }
        // 폐기 여부는 서명이 맞은 뒤에만 알려 준다.
        if dev.revoked_at.is_some() {
            return fail(ErrorType::DeviceRevoked, "revoked");
        }
        for (old, a) in self.agent_sockets() {
            if a.device_id.as_deref() == Some(&device_id) {
                let _ = old.close(Some(CLOSE_REPLACED), Some("replaced"));
            }
        }
        att.device_id = Some(device_id.clone());
        ws.serialize_attachment(&att)?;
        self.store.device_seen(&device_id, now())?;
        log("info", "agent.connected", json!({ "device_id": device_id }));
        Ok(())
    }

    fn agent_gone(&self, ws: &WebSocket) -> Result<()> {
        if let Some(AgentAtt {
            device_id: Some(id),
            stale: false,
            ..
        }) = ws.deserialize_attachment()?
        {
            self.store.device_seen(&id, now())?;
            log("info", "agent.disconnected", json!({ "device_id": id }));
        }
        Ok(())
    }
}

fn to_device(d: DeviceRow, online: &[String], now_ms: i64) -> Device {
    let on = online.contains(&d.id);
    let perm: Perm = serde_json::from_value(json!(d.perm)).unwrap_or(Perm::NeedMulti);
    let last = d.last_seen_at.unwrap_or(d.created_at);
    Device {
        os: if d.os == "macos" {
            Os::Macos
        } else {
            Os::Windows
        },
        online: on,
        last_seen_at: d.last_seen_at.map(iso),
        connectable: on && perm == Perm::Ok,
        offline_long: service::offline_long(on, last, now_ms),
        perm,
        name: d.name,
        id: d.id,
    }
}

impl DurableObject for Hub {
    fn new(state: State, env: Env) -> Self {
        let store = Store(state.storage().sql());
        match store.migrate() {
            Ok(n) if n > 0 => log("info", "migrate", json!({ "applied": n })),
            Ok(_) => {}
            Err(e) => log(
                "error",
                "internal_error",
                json!({ "detail": format!("migrate: {e}") }),
            ),
        }
        // M-09 유지 신호 — 휴면한 DO 를 깨우지 않고 런타임이 답한다.
        if let Ok(pair) = worker_sys::WebSocketRequestResponsePair::new("ping", "pong") {
            state.set_websocket_auto_response(&pair);
        }
        Self {
            state,
            env,
            store,
            limiter: RefCell::default(),
            jwks: JwksCache::default(),
        }
    }

    async fn fetch(&self, req: Request) -> Result<Response> {
        match self.route(req).await {
            Ok(r) => Ok(r),
            Err(kind) => crate::problem(kind, None),
        }
    }

    async fn websocket_message(
        &self,
        ws: WebSocket,
        message: WebSocketIncomingMessage,
    ) -> Result<()> {
        let Some(att) = ws.deserialize_attachment::<AgentAtt>()? else {
            return Ok(());
        };
        let WebSocketIncomingMessage::String(text) = message else {
            ws_error(&ws, ErrorType::BadRequest, true);
            return Ok(());
        };
        let msg = match parse::<WsMsg>(&text) {
            Ok(m) => m,
            Err(kind) => {
                // 인증 전 소켓은 꼴이 틀리면 바로 닫는다.
                ws_error(
                    &ws,
                    kind,
                    kind == ErrorType::TooLarge || att.device_id.is_none(),
                );
                return Ok(());
            }
        };
        match &att.device_id {
            None => self.agent_auth(&ws, att, msg).await,
            Some(id) => {
                self.store.device_seen(id, now())?;
                // shortcut: 세션·신호 메시지는 T5 에서 받는다 — 지금은 모두 거절.
                ws_error(&ws, ErrorType::BadRequest, false);
                Ok(())
            }
        }
    }

    async fn websocket_close(
        &self,
        ws: WebSocket,
        _code: usize,
        _reason: String,
        _clean: bool,
    ) -> Result<()> {
        let _ = ws.close(Some(1000), Some("bye"));
        self.agent_gone(&ws)
    }

    async fn websocket_error(&self, ws: WebSocket, _error: Error) -> Result<()> {
        self.agent_gone(&ws)
    }
}
