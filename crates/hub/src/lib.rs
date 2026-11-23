//! 허브 입구 계층 — 공개 경로만 받는다: /healthz·/idp/kakao/*(카카오 로그인 중계)는 여기서, /agent/*·/ws/* 는 DO `Hub`(인스턴스 `owner`)로 (DL-10·DL-18).
use proto::{ErrorType, Health, Problem, PROTO_MAJOR};
use worker::*;

mod access;
mod crypto;
mod entry;
mod kakao;
mod service;
mod store;
pub use entry::Hub;

/// 모든 상태는 이 한 인스턴스에 있다(1인 — 04 「데이터 저장」).
const HUB_INSTANCE: &str = "owner";

#[event(fetch)]
async fn fetch(mut req: Request, env: Env, _ctx: Context) -> Result<Response> {
    let path = req.path();
    if path == "/healthz" {
        return Response::from_json(&Health {
            ok: true,
            proto: PROTO_MAJOR,
        });
    }
    // 카카오 로그인 중계(Access 의 Generic OIDC 가 부른다 — DL-18).
    match (req.method(), path.as_str()) {
        (Method::Get, "/idp/kakao/authorize") => {
            let q = req.url()?.query().unwrap_or("").to_owned();
            return Response::redirect_with_status(Url::parse(&kakao::authorize_url(&q))?, 302);
        }
        (Method::Post, "/idp/kakao/token") => {
            let auth = req.headers().get("authorization")?;
            let body = kakao::token_form(&req.text().await?, auth.as_deref());
            let headers = Headers::new();
            headers.set(
                "content-type",
                "application/x-www-form-urlencoded;charset=utf-8",
            )?;
            let mut init = RequestInit::new();
            init.with_method(Method::Post)
                .with_headers(headers)
                .with_body(Some(body.into()));
            let mut res = Fetch::Request(Request::new_with_init(kakao::TOKEN, &init)?)
                .send()
                .await?;
            let status = res.status_code();
            let mut out = Response::ok(res.text().await?)?.with_status(status);
            out.headers_mut().set("content-type", "application/json")?;
            out.headers_mut().set("cache-control", "no-store")?;
            return Ok(out);
        }
        _ => {}
    }
    // /api/* 는 여기서 받지 않는다 — Access 뒤의 webdesktop-app 만 DO 에 넘긴다.
    if path.starts_with("/agent/") || path.starts_with("/ws/") {
        let stub = env
            .durable_object("HUB")?
            .id_from_name(HUB_INSTANCE)?
            .get_stub()?;
        return stub.fetch_with_request(req).await;
    }
    problem(ErrorType::NotFound, None)
}

/// RFC 9457 응답 하나를 만든다 — 거절 갈래의 유일한 출구.
pub(crate) fn problem(kind: ErrorType, detail: Option<String>) -> Result<Response> {
    let p = Problem::new(kind, detail);
    let mut res = Response::from_json(&p)?.with_status(p.status);
    res.headers_mut()
        .set("content-type", "application/problem+json")?;
    Ok(res)
}
