//! 허브 WebSocket — 입장권(E-13) → `/ws/viewer`(E-06) → viewer.hello.
//! 목록 갱신(M-01)과 세션 신호(M-03·M-04)를 받는다. 끊기면 새 입장권으로 다시 붙는다.
use crate::api::{self, window};
use crate::errors::Fail;
use proto::{parse, ErrorType, WsMsg, WsTicket, WsTicketReq, AGENT_HEARTBEAT_SEC, PROTO_MAJOR};
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::{prelude::Closure, JsCast};
use web_sys::{MessageEvent, WebSocket};

/// 다시 붙기 전 대기.
const RETRY_MS: i32 = 2000;

thread_local! {
    static WS: RefCell<Option<WebSocket>> = const { RefCell::new(None) };
}

type OnMsg = Rc<dyn Fn(WsMsg)>;
/// Ok = 붙음, Err = 끊김(이유).
type OnState = Rc<dyn Fn(Result<(), Fail>)>;

pub fn start(on_msg: OnMsg, on_state: OnState) {
    wasm_bindgen_futures::spawn_local(connect(on_msg, on_state));
    // M-09 유지 신호 — DO 가 깨지 않고 `pong` 으로 답한다.
    let ping = Closure::<dyn Fn()>::new(|| {
        WS.with(|w| {
            if let Some(ws) = &*w.borrow() {
                if ws.ready_state() == WebSocket::OPEN {
                    let _ = ws.send_with_str("ping");
                }
            }
        })
    });
    let _ = window().set_interval_with_callback_and_timeout_and_arguments_0(
        ping.as_ref().unchecked_ref(),
        (AGENT_HEARTBEAT_SEC * 1000) as i32,
    );
    ping.forget();
}

pub fn send(m: &WsMsg) -> bool {
    WS.with(|w| match &*w.borrow() {
        Some(ws) if ws.ready_state() == WebSocket::OPEN => serde_json::to_string(m)
            .ok()
            .is_some_and(|s| ws.send_with_str(&s).is_ok()),
        _ => false,
    })
}

fn retry(on_msg: OnMsg, on_state: OnState, why: Fail) {
    on_state(Err(why));
    // 로그인이 끝났거나 판이 다르면 새로 고칠 때까지 다시 붙지 않는다.
    if matches!(
        why,
        Fail::Hub(ErrorType::Unauthorized | ErrorType::UpgradeRequired)
    ) {
        return;
    }
    let again =
        Closure::once_into_js(move || wasm_bindgen_futures::spawn_local(connect(on_msg, on_state)));
    let _ = window()
        .set_timeout_with_callback_and_timeout_and_arguments_0(again.unchecked_ref(), RETRY_MS);
}

async fn connect(on_msg: OnMsg, on_state: OnState) {
    let t: WsTicket = match api::post(
        "/api/ws-ticket",
        &WsTicketReq {
            viewer_id: api::viewer_id(),
        },
    )
    .await
    {
        Ok(t) => t,
        Err(e) => return retry(on_msg, on_state, e),
    };
    let Ok(ws) = WebSocket::new(&format!("{}?ticket={}", t.hub_ws_url, t.ticket)) else {
        return retry(on_msg, on_state, Fail::Network);
    };
    // 허브가 닫으며 알려 준 이유(error 메시지) — 닫힘 처리에서 쓴다.
    let why = Rc::new(RefCell::new(Fail::Network));

    let hello_ws = ws.clone();
    let up = on_state.clone();
    let onopen = Closure::<dyn Fn()>::new(move || {
        let hello = WsMsg::ViewerHello {
            viewer_id: api::viewer_id(),
            proto: PROTO_MAJOR,
        };
        if let Ok(s) = serde_json::to_string(&hello) {
            let _ = hello_ws.send_with_str(&s);
        }
        up(Ok(()));
    });

    let (m, w) = (on_msg.clone(), why.clone());
    let onmessage = Closure::<dyn Fn(MessageEvent)>::new(move |e: MessageEvent| {
        let Some(text) = e.data().as_string() else {
            return;
        };
        if text == "pong" {
            return;
        }
        match parse::<WsMsg>(&text) {
            Ok(WsMsg::Error { kind, .. }) => {
                *w.borrow_mut() = Fail::Hub(kind);
                m(WsMsg::Error { kind, detail: None });
            }
            Ok(msg) => m(msg),
            Err(_) => {}
        }
    });

    let onclose = Closure::<dyn Fn()>::new(move || {
        WS.with(|w| *w.borrow_mut() = None);
        let why = *why.borrow();
        retry(on_msg.clone(), on_state.clone(), why);
    });

    ws.set_onopen(Some(onopen.as_ref().unchecked_ref()));
    ws.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
    ws.set_onclose(Some(onclose.as_ref().unchecked_ref()));
    // shortcut: 다시 붙을 때마다 닫힌 소켓의 처리기 3개가 남는다(작음), 오래 켜 두는 탭에서 문제가 되면 소켓과 함께 보관해 지운다.
    onopen.forget();
    onmessage.forget();
    onclose.forget();
    WS.with(|w| *w.borrow_mut() = Some(ws));
}
