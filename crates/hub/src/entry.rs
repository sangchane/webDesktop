//! 입구 계층 — DO `Hub` 의 경로·인증(Access JWT·기기 서명)·빈도 제한·꼴 검사. 통과한 값만 service·store 로 (04 「코드 구조」).
use crate::access::{Access, JwksCache};
use crate::crypto::{ed25519_verify, random};
use crate::service::{self, CodeVerdict, RateLimit, SessionPlan, Tickets};
use crate::store::{DeviceRow, NewDevice, SessionRow, Stats, Store};
use base64::engine::general_purpose::{STANDARD as B64, URL_SAFE_NO_PAD as B64URL};
use base64::Engine;
use proto::{
    parse, AccessLog, AccessLogItem, Device, EndReason, EnrollCode, EnrollReq, EnrollResp,
    ErrorType, IceServer, InstallLinks, Os, Perm, SessionReq, SessionResp, Validate, WsMsg,
    WsTicket, WsTicketReq, ACCESS_LOG_DAYS, ACCESS_LOG_PAGE, PROTO_MAJOR, REG_CODE_LEN,
    VIEWER_UA_MAX, WS_MSG_MAX_BYTES,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::cell::RefCell;
use worker::js_sys::{Date, Function, Reflect};
use worker::wasm_bindgen::{JsCast, JsValue};
use worker::*;

const TAG_AGENT: &str = "agent";
const TAG_VIEWER: &str = "viewer";
/// 운영 TURN 자격 API (Cloudflare Realtime TURN 「Generate Credentials」).
const TURN_API: &str = "https://rtc.live.cloudflare.com";
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
    /// 허브가 닫는 중(조용함·새 연결로 대체) — 닫힘 이벤트가 last_seen_at·온라인 표시를 덮지 않게.
    #[serde(default)]
    stale: bool,
}

/// 뷰어 WebSocket 상태 (E-06). viewer_id 는 입장권에서 온다.
#[derive(Serialize, Deserialize)]
struct ViewerAtt {
    viewer_id: String,
    /// viewer.hello 를 받았다.
    ready: bool,
    /// 연 시각 — 아직 ping 을 안 보낸 뷰어가 산 것인지 가린다(FR-023).
    #[serde(default)]
    opened_at: i64,
}

#[durable_object]
pub struct Hub {
    state: State,
    env: Env,
    store: Store,
    limiter: RefCell<RateLimit>,
    jwks: JwksCache,
    tickets: RefCell<Tickets>,
}

/// 요청마다 읽는 설정. 빠지면 500 `internal`(SPEC 바닥선 8).
struct Config {
    access: Access,
    hub_url: String,
    turn_key_id: String,
    turn_key_token: String,
    /// 운영은 TURN_API, 시험은 가짜 TURN 서버(TURN_TEST_URL).
    turn_api: String,
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
        let (Some(team_domain), Some(aud), Some(hub_url), Some(turn_key_id), Some(turn_key_token)) = (
            var("ACCESS_TEAM_DOMAIN"),
            var("ACCESS_AUD"),
            var("HUB_PUBLIC_URL"),
            var("TURN_KEY_ID"),
            var("TURN_KEY_TOKEN"),
        ) else {
            log(
                "error",
                "internal_error",
                json!({ "detail": "config missing" }),
            );
            return Err(ErrorType::Internal);
        };
        let test_jwks = var("ACCESS_TEST_JWKS");
        let turn_test = var("TURN_TEST_URL");
        let local = matches!(
            req.url()
                .ok()
                .and_then(|u| u.host_str().map(str::to_owned))
                .as_deref(),
            Some("localhost" | "127.0.0.1")
        );
        if (test_jwks.is_some() || turn_test.is_some()) && !local {
            log(
                "error",
                "internal_error",
                json!({ "detail": "test vars outside localhost" }),
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
            turn_key_id,
            turn_key_token,
            turn_api: turn_test.unwrap_or_else(|| TURN_API.into()),
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

    fn is_viewer(&self, ws: &WebSocket) -> bool {
        self.state.get_tags(ws).iter().any(|t| t == TAG_VIEWER)
    }

    fn agent_for(&self, device_id: &str) -> Option<WebSocket> {
        self.agent_sockets()
            .into_iter()
            .find(|(_, a)| a.device_id.as_deref() == Some(device_id) && !a.stale)
            .map(|(ws, _)| ws)
    }

    /// viewer.hello 를 마친 뷰어 연결.
    fn viewers(&self) -> Vec<(WebSocket, ViewerAtt)> {
        self.state
            .get_websockets_with_tag(TAG_VIEWER)
            .into_iter()
            .filter_map(|ws| {
                let att = ws.deserialize_attachment::<ViewerAtt>().ok().flatten()?;
                att.ready.then_some((ws, att))
            })
            .collect()
    }

    fn send_viewer(&self, viewer_id: &str, msg: &WsMsg) {
        for (ws, att) in self.viewers() {
            if att.viewer_id == viewer_id {
                let _ = ws.send(msg);
            }
        }
    }

    /// M-01 — 온라인이 바뀐 기기를 모든 뷰어에게.
    fn push_status(&self, device_id: &str, online: bool, now_ms: i64) -> Result<()> {
        let Some(d) = self.store.device_get(device_id)? else {
            return Ok(());
        };
        let msg = status_msg(to_device(d, online, now_ms));
        for (ws, _) in self.viewers() {
            let _ = ws.send(&msg);
        }
        Ok(())
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
            // 허브가 이미 닫는 중인 연결.
            if att.stale {
                continue;
            }
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
                self.push_status(&id, false, now_ms)?;
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
        // 시험은 묶음마다 다른 주소로 — 한 분 안에 여러 묶음이 빈도 한도를 나눠 쓰지 않게.
        let ip_header = if cfg.test {
            "x-test-ip"
        } else {
            "cf-connecting-ip"
        };
        let ip = req
            .headers()
            .get(ip_header)
            .ok()
            .flatten()
            .unwrap_or_else(|| "local".into());
        if !self.limiter.borrow_mut().hit(&ip, now_ms) {
            log("warn", "rate_limited", json!({}));
            return Err(ErrorType::RateLimited);
        }
        let path = req.path();
        if path.starts_with("/api/") {
            let token = req.headers().get("cf-access-jwt-assertion").ok().flatten();
            cfg.access.check(token, now_ms, &self.jwks).await?;
        }
        match (req.method(), path.as_str()) {
            (Method::Post, "/api/enroll-codes") => self.new_code(&cfg, now_ms).map_err(internal),
            (Method::Get, "/api/devices") => self.devices(now_ms).map_err(internal),
            (Method::Get, "/api/access-log") => self.access_log(&req, now_ms),
            (Method::Post, "/api/ws-ticket") => self.ticket(req, &cfg, now_ms).await,
            (Method::Post, "/api/sessions") => self.open_session(req, &cfg, now_ms).await,
            (Method::Get, "/ws/viewer") => self.viewer_ws(&req, now_ms),
            (Method::Post, "/agent/enroll") => self.enroll(req, now_ms).await,
            (Method::Get, "/agent/install/windows" | "/agent/install/macos") => {
                let token = query(&req, "t");
                let row = self.store.code_get().map_err(internal)?;
                service::judge_install(row.as_ref(), &token, now_ms)?;
                self.install_file(&path).await
            }
            (Method::Get, "/agent/ws") => self.agent_ws(&req, now_ms).map_err(internal),
            _ => Err(ErrorType::NotFound),
        }
    }

    /// E-12 — R2 `webdesktop-releases` 의 `<os>/<파일>`. 아직 올리지 않은 OS(macOS — 첫 작업 3)는 not_found.
    async fn install_file(&self, path: &str) -> std::result::Result<Response, ErrorType> {
        let (key, name) = match path {
            "/agent/install/windows" => ("windows/webdesktop-agent.exe", "webdesktop-agent.exe"),
            _ => ("macos/webdesktop-agent", "webdesktop-agent"),
        };
        let bucket = self.env.bucket("RELEASES").map_err(internal)?;
        let obj = bucket
            .get(key)
            .execute()
            .await
            .map_err(internal)?
            .ok_or(ErrorType::NotFound)?;
        let size = obj.size().to_string();
        let body = obj
            .body()
            .ok_or(ErrorType::NotFound)?
            .response_body()
            .map_err(internal)?;
        let mut res = Response::from_body(body).map_err(internal)?;
        let h = res.headers_mut();
        let set = |h: &mut Headers, k: &str, v: &str| h.set(k, v).map_err(internal);
        set(h, "content-type", "application/octet-stream")?;
        set(
            h,
            "content-disposition",
            &format!("attachment; filename=\"{name}\""),
        )?;
        set(h, "cache-control", "no-store")?;
        set(h, "content-length", &size)?;
        log("info", "install.served", json!({ "key": key }));
        Ok(res)
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

    /// E-09 — 커서 `<started_at>_<id>`, 보관 기간(ACCESS_LOG_DAYS) 안만.
    fn access_log(&self, req: &Request, now_ms: i64) -> std::result::Result<Response, ErrorType> {
        let before = query(req, "before");
        let cursor = if before.is_empty() {
            // JS 안전 정수 최댓값 — 숫자가 JS 로 넘어가며 정밀도 경고를 남기지 않게.
            ((1_i64 << 53) - 1, String::new())
        } else {
            let (at, id) = before.split_once('_').ok_or(ErrorType::BadRequest)?;
            (
                at.parse().map_err(|_| ErrorType::BadRequest)?,
                id.to_owned(),
            )
        };
        let since = now_ms - ACCESS_LOG_DAYS as i64 * 86_400_000;
        // 한 줄 더 읽어 다음 쪽이 실제로 있을 때만 next 를 준다.
        let mut rows = self
            .store
            .access_log(since, (cursor.0, &cursor.1), ACCESS_LOG_PAGE + 1)
            .map_err(internal)?;
        let more = rows.len() > ACCESS_LOG_PAGE;
        rows.truncate(ACCESS_LOG_PAGE);
        let next = more
            .then(|| rows.last().map(|r| format!("{}_{}", r.started_at, r.id)))
            .flatten();
        let items = rows
            .into_iter()
            .map(|r| AccessLogItem {
                reconnect_ms: r
                    .reconnect_ms_json
                    .and_then(|j| serde_json::from_str(&j).ok())
                    .unwrap_or_default(),
                session_id: r.id,
                device_id: r.device_id,
                viewer_id: r.viewer_id,
                viewer_ua: r.viewer_ua,
                path: r.path,
                click_to_first_frame_ms: r.click_to_first_frame_ms,
                latency_p50_ms: r.latency_p50_ms,
                reconnect_count: r.reconnect_count,
                reconnect_ms_max: r.reconnect_ms_max,
                restarts: r.restarts,
                new_session_used: r.new_session_used != 0,
                encoder: r.encoder,
                started_at: iso(r.started_at),
                ended_at: r.ended_at.map(iso),
                end_reason: r.end_reason,
            })
            .collect();
        Response::from_json(&AccessLog { items, next }).map_err(internal)
    }

    /// E-02.
    fn devices(&self, now_ms: i64) -> Result<Response> {
        let online = self.sweep(now_ms)?;
        let list: Vec<Device> = self
            .store
            .devices()?
            .into_iter()
            .map(|d| {
                let on = online.contains(&d.id);
                to_device(d, on, now_ms)
            })
            .collect();
        Response::from_json(&list)
    }

    /// E-04 — 05 「트랜잭션」 등록 순서 그대로.
    async fn enroll(
        &self,
        mut req: Request,
        now_ms: i64,
    ) -> std::result::Result<Response, ErrorType> {
        let r: EnrollReq = read_body(&mut req).await?;
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
                let _ = old.serialize_attachment(AgentAtt { stale: true, ..a });
                let _ = old.close(Some(CLOSE_REPLACED), Some("replaced"));
            }
        }
        att.device_id = Some(device_id.clone());
        ws.serialize_attachment(&att)?;
        let t = now();
        self.store.device_seen(&device_id, t)?;
        log("info", "agent.connected", json!({ "device_id": device_id }));
        // 인증이 끝난 뒤에만 — 에이전트는 이걸 받고 기다리는 권한 요청을 다시 낸다(재접속 사이 붙은 뷰어용).
        let _ = ws.send(&WsMsg::AskSync {});
        self.push_status(&device_id, true, t)
    }

    fn agent_gone(&self, ws: &WebSocket) -> Result<()> {
        if let Some(AgentAtt {
            device_id: Some(id),
            stale: false,
            ..
        }) = ws.deserialize_attachment()?
        {
            let t = now();
            self.store.device_seen(&id, t)?;
            log("info", "agent.disconnected", json!({ "device_id": id }));
            self.push_status(&id, false, t)?;
        }
        Ok(())
    }
}

/// 뷰어 입장권·세션·신호 중계 (E-13·E-06·E-07·M-01~M-04·M-10).
impl Hub {
    /// E-13 — 1회용, WS_TICKET_TTL_SEC.
    async fn ticket(
        &self,
        mut req: Request,
        cfg: &Config,
        now_ms: i64,
    ) -> std::result::Result<Response, ErrorType> {
        let r: WsTicketReq = read_body(&mut req).await?;
        let ticket = B64URL.encode(random::<32>());
        let exp = self
            .tickets
            .borrow_mut()
            .issue(&ticket, &r.viewer_id, now_ms);
        let ws = WsTicket {
            ticket,
            hub_ws_url: format!("{}/ws/viewer", cfg.hub_url.replacen("http", "ws", 1)),
            expires_at: iso(exp),
        };
        json_status(&ws, 201).map_err(internal)
    }

    /// E-06 — 입장권을 먼저 쓴다(틀리면 업그레이드 전에 401). 업그레이드가 아닌 요청이 입장권을 써 버려도
    /// 뷰어는 새로 받으면 된다 — 대신 일반 HTTP 로도 401 을 확인할 수 있다(운영 확인 check-access).
    fn viewer_ws(&self, req: &Request, now_ms: i64) -> std::result::Result<Response, ErrorType> {
        let viewer_id = self
            .tickets
            .borrow_mut()
            .take(&query(req, "ticket"), now_ms)?;
        let upgrade = req
            .headers()
            .get("upgrade")
            .ok()
            .flatten()
            .unwrap_or_default();
        if !upgrade.eq_ignore_ascii_case("websocket") {
            return Err(ErrorType::BadRequest);
        }
        let pair = WebSocketPair::new().map_err(internal)?;
        self.state
            .accept_websocket_with_tags(&pair.server, &[TAG_VIEWER]);
        let att = ViewerAtt {
            viewer_id,
            ready: false,
            opened_at: now_ms,
        };
        if let Err(e) = pair.server.serialize_attachment(att) {
            let _ = pair.server.close(Some(1011), Some("internal"));
            return Err(internal(e));
        }
        Response::from_websocket(pair.client).map_err(internal)
    }

    /// TURN 자격(TURN_CRED_TTL_MIN). 실패하면 `internal`.
    async fn turn(&self, cfg: &Config) -> std::result::Result<Vec<IceServer>, ErrorType> {
        let call = async {
            let headers = Headers::new();
            headers.set("authorization", &format!("Bearer {}", cfg.turn_key_token))?;
            headers.set("content-type", "application/json")?;
            let mut init = RequestInit::new();
            init.with_method(Method::Post)
                .with_headers(headers)
                .with_body(Some(
                    json!({ "ttl": service::TURN_TTL_SEC }).to_string().into(),
                ));
            let url = format!(
                "{}/v1/turn/keys/{}/credentials/generate-ice-servers",
                cfg.turn_api, cfg.turn_key_id
            );
            let mut res = Fetch::Request(Request::new_with_init(&url, &init)?)
                .send()
                .await?;
            Ok::<_, Error>((res.status_code(), res.text().await?))
        };
        let (status, body) = call.await.map_err(internal)?;
        match (status, service::ice_servers(&body)) {
            (200..=299, Some(ice)) => {
                log("info", "turn.issued", json!({}));
                Ok(ice)
            }
            _ => {
                log(
                    "error",
                    "internal_error",
                    json!({ "detail": "turn credentials", "status": status }),
                );
                Err(ErrorType::Internal)
            }
        }
    }

    /// E-07 — 05 「규약」 멱등·이어받기.
    async fn open_session(
        &self,
        mut req: Request,
        cfg: &Config,
        now_ms: i64,
    ) -> std::result::Result<Response, ErrorType> {
        let r: SessionReq = read_body(&mut req).await?;
        let ua: Option<String> = req
            .headers()
            .get("user-agent")
            .ok()
            .flatten()
            .map(|u| u.chars().take(VIEWER_UA_MAX).collect());
        let dev = self
            .store
            .device_get(&r.device_id)
            .map_err(internal)?
            .ok_or(ErrorType::NotFound)?;
        if dev.revoked_at.is_some() {
            return Err(ErrorType::DeviceRevoked);
        }
        if !self.sweep(now_ms).map_err(internal)?.contains(&dev.id) {
            return Err(ErrorType::DeviceOffline);
        }
        let ice = self.turn(cfg).await?;
        // await 뒤라 다시 본다. 아래 쓰기는 await 없이 한 묶음으로 커밋된다.
        let revoked = self
            .store
            .device_get(&dev.id)
            .map_err(internal)?
            .is_none_or(|d| d.revoked_at.is_some());
        if revoked {
            return Err(ErrorType::DeviceRevoked);
        }
        let agent = self.agent_for(&dev.id).ok_or(ErrorType::DeviceOffline)?;
        let open = self.store.session_open(&dev.id).map_err(internal)?;
        match (
            service::session_plan(open.as_ref().map(|s| s.viewer_id.as_str()), &r.viewer_id),
            open,
        ) {
            (SessionPlan::Reuse, Some(s)) => {
                // 에이전트가 그사이 다시 붙었으면 이 세션을 모른다 — 다시 알린다(에이전트는 session_id 로 중복을 거른다).
                let start = WsMsg::SessionStart {
                    session_id: s.id.clone(),
                    ice_servers: ice.clone(),
                };
                if agent.send(&start).is_err() {
                    return Err(ErrorType::DeviceOffline);
                }
                let resp = SessionResp {
                    session_id: s.id,
                    ice_servers: ice,
                };
                return json_status(&resp, 200).map_err(internal);
            }
            (SessionPlan::TakeOver, Some(old)) => {
                self.end_session(&old, EndReason::TakenOver, now_ms)
                    .map_err(internal)?;
                log(
                    "info",
                    "session.taken_over",
                    json!({ "session_id": old.id }),
                );
            }
            _ => {}
        }
        let id = service::uuid_v7(now_ms, &random::<10>());
        self.store
            .session_insert(&id, &dev.id, &r.viewer_id, ua.as_deref(), now_ms)
            .map_err(internal)?;
        let start = WsMsg::SessionStart {
            session_id: id.clone(),
            ice_servers: ice.clone(),
        };
        if agent.send(&start).is_err() {
            self.store
                .session_end(&id, &name_of(EndReason::Failed), now_ms)
                .map_err(internal)?;
            log(
                "warn",
                "session.failed",
                json!({ "session_id": id, "why": "agent_send" }),
            );
            return Err(ErrorType::DeviceOffline);
        }
        log(
            "info",
            "session.started",
            json!({ "session_id": id, "device_id": dev.id }),
        );
        let resp = SessionResp {
            session_id: id,
            ice_servers: ice,
        };
        json_status(&resp, 201).map_err(internal)
    }

    /// 허브가 세션을 닫을 때 — 양쪽에 session.end.
    fn end_session(&self, s: &SessionRow, reason: EndReason, now_ms: i64) -> Result<()> {
        self.store.session_end(&s.id, &name_of(reason), now_ms)?;
        let msg = WsMsg::SessionEnd {
            session_id: s.id.clone(),
            reason,
        };
        self.send_viewer(&s.viewer_id, &msg);
        if let Some(a) = self.agent_for(&s.device_id) {
            let _ = a.send(&msg);
        }
        Ok(())
    }

    /// E-06 — 첫 메시지는 viewer.hello, 그 뒤 신호·종료·계측.
    fn viewer_message(&self, ws: &WebSocket, message: WebSocketIncomingMessage) -> Result<()> {
        let Some(mut att) = ws.deserialize_attachment::<ViewerAtt>()? else {
            return Ok(());
        };
        let WebSocketIncomingMessage::String(text) = message else {
            ws_error(ws, ErrorType::BadRequest, true);
            return Ok(());
        };
        let msg = match parse::<WsMsg>(&text) {
            Ok(m) => m,
            Err(kind) => {
                ws_error(ws, kind, kind == ErrorType::TooLarge || !att.ready);
                return Ok(());
            }
        };
        if !att.ready {
            match msg {
                WsMsg::ViewerHello { viewer_id, proto } if viewer_id == att.viewer_id => {
                    if proto != PROTO_MAJOR {
                        ws_error(ws, ErrorType::UpgradeRequired, true);
                        return Ok(());
                    }
                    att.ready = true;
                    ws.serialize_attachment(&att)?;
                    // 붙은 순간의 목록 — 목록 조회와 연결 사이에 바뀐 것을 놓치지 않게.
                    let now_ms = now();
                    let online = self.sweep(now_ms)?;
                    for d in self.store.devices()? {
                        let on = online.contains(&d.id);
                        let _ = ws.send(&status_msg(to_device(d, on, now_ms)));
                    }
                    // 기다리는 권한 요청은 에이전트가 갖고 있다 — 다시 보내 달라고(DL-24).
                    for (a, att) in self.agent_sockets() {
                        if att.device_id.is_some() && !att.stale {
                            let _ = a.send(&WsMsg::AskSync {});
                        }
                    }
                }
                _ => ws_error(ws, ErrorType::Unauthorized, true),
            }
            return Ok(());
        }
        // FR-023 — 뷰어는 소유자 한 명이라 어느 기기의 요청에도 답할 수 있다.
        if let WsMsg::AskAnswer {
            ask_id, device_id, ..
        } = &msg
        {
            match self.agent_for(device_id) {
                Some(a) => {
                    log(
                        "info",
                        "ask.answer",
                        json!({ "device_id": device_id, "ask_id": ask_id }),
                    );
                    let _ = a.send(&msg);
                }
                None => ws_error(ws, ErrorType::DeviceOffline, false),
            }
            return Ok(());
        }
        let Some(sid) = session_of(&msg) else {
            ws_error(ws, ErrorType::BadRequest, false);
            return Ok(());
        };
        // 이 뷰어의 세션만. 계측은 닫힌 뒤에도 받는다.
        let stats = matches!(msg, WsMsg::ViewerStats { .. });
        let s = match self.store.session_get(sid)? {
            Some(s) if s.viewer_id == att.viewer_id && (stats || s.ended_at.is_none()) => s,
            _ => {
                ws_error(ws, ErrorType::NotFound, false);
                return Ok(());
            }
        };
        match &msg {
            WsMsg::ViewerStats {
                click_to_first_frame_ms,
                latency_p50_ms,
                reconnect_ms,
                restarts,
                new_session_used,
                path,
                ..
            } => {
                let path = path.map(name_of);
                self.store.session_stats(
                    &s.id,
                    &Stats {
                        path: path.as_deref(),
                        click_to_first_frame_ms: *click_to_first_frame_ms,
                        latency_p50_ms: *latency_p50_ms,
                        reconnect_ms,
                        restarts: *restarts,
                        new_session_used: *new_session_used,
                    },
                )?;
            }
            WsMsg::SessionEnd { reason, .. } => {
                if !service::end_reason_ok(*reason, false) {
                    ws_error(ws, ErrorType::BadRequest, false);
                    return Ok(());
                }
                self.store.session_end(&s.id, &name_of(*reason), now())?;
                log_end(&s.id, *reason);
                if let Some(a) = self.agent_for(&s.device_id) {
                    let _ = a.send(&msg);
                }
            }
            // 신호 — 에이전트로.
            _ => match self.agent_for(&s.device_id) {
                Some(a) => {
                    let _ = a.send(&msg);
                }
                None => ws_error(ws, ErrorType::DeviceOffline, false),
            },
        }
        Ok(())
    }

    /// 에이전트 WS — 인증 전이면 auth, 뒤면 신호·종료.
    async fn agent_ws_message(
        &self,
        ws: &WebSocket,
        message: WebSocketIncomingMessage,
    ) -> Result<()> {
        let Some(att) = ws.deserialize_attachment::<AgentAtt>()? else {
            return Ok(());
        };
        let WebSocketIncomingMessage::String(text) = message else {
            ws_error(ws, ErrorType::BadRequest, true);
            return Ok(());
        };
        let msg = match parse::<WsMsg>(&text) {
            Ok(m) => m,
            Err(kind) => {
                // 인증 전 소켓은 꼴이 틀리면 바로 닫는다.
                ws_error(
                    ws,
                    kind,
                    kind == ErrorType::TooLarge || att.device_id.is_none(),
                );
                return Ok(());
            }
        };
        match &att.device_id {
            None => self.agent_auth(ws, att, msg).await,
            Some(id) => {
                self.store.device_seen(id, now())?;
                self.agent_message(ws, id, msg)
            }
        }
    }

    /// 인증된 에이전트의 신호·종료 — 그 세션의 뷰어로.
    fn agent_message(&self, ws: &WebSocket, device_id: &str, msg: WsMsg) -> Result<()> {
        // FR-023 — 저장하지 않고 뷰어 전부에게. device_id 는 인증된 값으로 덮는다. 명령 원문(detail)은 로그에 남기지 않는다.
        match msg {
            WsMsg::AskNew {
                ask_id,
                project,
                tool,
                detail,
                truncated,
                wait_sec,
                ..
            } => {
                let viewers = self.viewers();
                // 뒤로 간 폰 탭은 연결만 남고 ping 이 멎는다 — 그런 뷰어만 있으면 터미널을 막지 않는다.
                // 마지막 ping(없으면 연 시각)이 OFFLINE_DETECT_SEC 안인 뷰어만 산 것으로.
                let now_ms = now();
                let live = viewers
                    .iter()
                    .filter(|(v, att)| {
                        let t = self.auto_response_at(v).unwrap_or(att.opened_at);
                        now_ms - t <= (proto::OFFLINE_DETECT_SEC * 1000) as i64
                    })
                    .count();
                log(
                    "info",
                    "ask.new",
                    json!({ "device_id": device_id, "ask_id": ask_id, "tool": tool, "viewers": live }),
                );
                if live == 0 {
                    let _ = ws.send(&WsMsg::AskDone {
                        ask_id,
                        device_id: device_id.to_owned(),
                    });
                    return Ok(());
                }
                let m = WsMsg::AskNew {
                    ask_id,
                    device_id: device_id.to_owned(),
                    project,
                    tool,
                    detail,
                    truncated,
                    wait_sec,
                };
                for (v, _) in viewers {
                    let _ = v.send(&m);
                }
                return Ok(());
            }
            WsMsg::AskDone { ask_id, .. } => {
                let m = WsMsg::AskDone {
                    ask_id,
                    device_id: device_id.to_owned(),
                };
                for (v, _) in self.viewers() {
                    let _ = v.send(&m);
                }
                return Ok(());
            }
            _ => {}
        }
        let sid = match session_of(&msg) {
            Some(sid) if !matches!(msg, WsMsg::ViewerStats { .. }) => sid,
            _ => {
                ws_error(ws, ErrorType::BadRequest, false);
                return Ok(());
            }
        };
        let s = match self.store.session_get(sid)? {
            Some(s) if s.device_id == device_id && s.ended_at.is_none() => s,
            _ => {
                ws_error(ws, ErrorType::NotFound, false);
                return Ok(());
            }
        };
        if let WsMsg::SessionEnd { reason, .. } = &msg {
            if !service::end_reason_ok(*reason, true) {
                ws_error(ws, ErrorType::BadRequest, false);
                return Ok(());
            }
            self.store.session_end(&s.id, &name_of(*reason), now())?;
            log_end(&s.id, *reason);
        }
        self.send_viewer(&s.viewer_id, &msg);
        Ok(())
    }
}

/// 본문을 크기 → JSON → 꼴 검사 순으로 읽는다. 크기는 읽기 전에 — 큰 본문이 DO 메모리를 채우지 못하게.
async fn read_body<T: serde::de::DeserializeOwned + Validate>(
    req: &mut Request,
) -> std::result::Result<T, ErrorType> {
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
    parse(&body)
}

fn query(req: &Request, key: &str) -> String {
    req.url()
        .ok()
        .and_then(|u| {
            u.query_pairs()
                .find(|(k, _)| k == key)
                .map(|(_, v)| v.into_owned())
        })
        .unwrap_or_default()
}

fn internal(e: Error) -> ErrorType {
    log(
        "error",
        "internal_error",
        json!({ "detail": e.to_string() }),
    );
    ErrorType::Internal
}

/// serde 이름(snake_case) 그대로 — DB 칸에 넣을 때.
fn name_of<T: Serialize>(v: T) -> String {
    serde_json::to_value(v)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_default()
}

fn log_end(session_id: &str, reason: EndReason) {
    let event = if reason == EndReason::Failed {
        "session.failed"
    } else {
        "session.ended"
    };
    log(
        "info",
        event,
        json!({ "session_id": session_id, "reason": name_of(reason) }),
    );
}

/// 신호·종료·계측 메시지의 세션 id. 나머지는 None.
fn session_of(msg: &WsMsg) -> Option<&str> {
    match msg {
        WsMsg::SignalOffer { session_id, .. }
        | WsMsg::SignalAnswer { session_id, .. }
        | WsMsg::SignalCandidate { session_id, .. }
        | WsMsg::SignalRestart { session_id }
        | WsMsg::SessionEnd { session_id, .. }
        | WsMsg::ViewerStats { session_id, .. } => Some(session_id),
        _ => None,
    }
}

fn status_msg(d: Device) -> WsMsg {
    WsMsg::DeviceStatus {
        device_id: d.id,
        online: d.online,
        perm: d.perm,
        connectable: d.connectable,
        last_seen_at: d.last_seen_at,
    }
}

fn to_device(d: DeviceRow, on: bool, now_ms: i64) -> Device {
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
            tickets: RefCell::default(),
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
        let handled = if self.is_viewer(&ws) {
            self.viewer_message(&ws, message)
        } else {
            self.agent_ws_message(&ws, message).await
        };
        // 저장 오류를 런타임에 넘기지 않고 남긴다 — 보낸 쪽도 실패를 안다.
        if let Err(e) = handled {
            internal(e);
            ws_error(&ws, ErrorType::Internal, false);
        }
        Ok(())
    }

    async fn websocket_close(
        &self,
        ws: WebSocket,
        _code: usize,
        _reason: String,
        _clean: bool,
    ) -> Result<()> {
        let _ = ws.close(Some(1000), Some("bye"));
        if self.is_viewer(&ws) {
            return Ok(());
        }
        self.agent_gone(&ws)
    }

    async fn websocket_error(&self, ws: WebSocket, _error: Error) -> Result<()> {
        if self.is_viewer(&ws) {
            return Ok(());
        }
        self.agent_gone(&ws)
    }
}
