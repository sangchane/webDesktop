//! 웹앱 입구 계층 — /api/* 를 허브 DO `owner` 로 넘긴다. 화면은 정적 자산 (decision-log DL-10).
use proto::{ErrorType, Problem};
use worker::*;

#[event(fetch)]
async fn fetch(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    if req.path().starts_with("/api/") {
        let stub = env
            .durable_object("HUB")?
            .id_from_name("owner")?
            .get_stub()?;
        return stub.fetch_with_request(req).await;
    }
    let p = Problem::new(ErrorType::NotFound, None);
    Ok(Response::from_json(&p)?.with_status(p.status))
}
