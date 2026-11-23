//! `/api/*` 호출(같은 주소의 app Worker — Access 쿠키가 따라간다)과 뷰어 식별자.
use crate::errors::Fail;
use proto::{ErrorType, Problem};
use serde::{de::DeserializeOwned, Serialize};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{Headers, RequestInit, RequestRedirect, Response, ResponseType};

pub fn window() -> web_sys::Window {
    web_sys::window().expect("브라우저 창")
}

/// 05 「뷰어 식별」 — 처음 열 때 만든 uuid 를 localStorage 에 둔다.
pub fn viewer_id() -> String {
    const KEY: &str = "viewer_id";
    let store = window().local_storage().ok().flatten();
    if let Some(id) = store.as_ref().and_then(|s| s.get_item(KEY).ok().flatten()) {
        return id;
    }
    let id = window().crypto().expect("crypto").random_uuid();
    if let Some(s) = store {
        let _ = s.set_item(KEY, &id);
    }
    id
}

/// `?relay=1` — TURN 경로만 쓰는 시험 스위치(06 「중계 강제」).
pub fn relay_only() -> bool {
    window()
        .location()
        .search()
        .is_ok_and(|q| q.split(['?', '&']).any(|p| p == "relay=1"))
}

pub fn now_ms() -> f64 {
    window().performance().map_or(0.0, |p| p.now())
}

pub async fn get<T: DeserializeOwned>(path: &str) -> Result<T, Fail> {
    call("GET", path, None).await
}

pub async fn post<T: DeserializeOwned>(path: &str, body: &impl Serialize) -> Result<T, Fail> {
    let b = serde_json::to_string(body).map_err(|_| Fail::Hub(ErrorType::BadRequest))?;
    call("POST", path, Some(b)).await
}

async fn call<T: DeserializeOwned>(
    method: &str,
    path: &str,
    body: Option<String>,
) -> Result<T, Fail> {
    let init = RequestInit::new();
    init.set_method(method);
    // Access 로그인이 끝나면 로그인 화면으로 돌려보낸다 — 따라가지 않고 「로그인 끝」으로 본다.
    init.set_redirect(RequestRedirect::Manual);
    if let Some(b) = body {
        let h = Headers::new().map_err(|_| Fail::Network)?;
        let _ = h.set("content-type", "application/json");
        init.set_headers(&h);
        init.set_body(&JsValue::from_str(&b));
    }
    let resp: Response = JsFuture::from(window().fetch_with_str_and_init(path, &init))
        .await
        .map_err(|_| Fail::Network)?
        .unchecked_into();
    if resp.type_() == ResponseType::Opaqueredirect {
        return Err(Fail::Hub(ErrorType::Unauthorized));
    }
    let text = JsFuture::from(resp.text().map_err(|_| Fail::Network)?)
        .await
        .map_err(|_| Fail::Network)?
        .as_string()
        .unwrap_or_default();
    if !resp.ok() {
        let kind = serde_json::from_str::<Problem>(&text).map_or(ErrorType::Internal, |p| p.kind);
        return Err(Fail::Hub(kind));
    }
    serde_json::from_str(&text).map_err(|_| Fail::Hub(ErrorType::Internal))
}
