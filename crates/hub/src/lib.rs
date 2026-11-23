//! 허브 입구 계층 — Worker fetch 가 경로를 나눠 DO `Hub`(인스턴스 `owner`)로 넘긴다 (04 「코드 구조」).
use proto::{ErrorType, Health, Problem, PROTO_MAJOR};
use worker::*;

mod hub;
pub use hub::Hub;

/// 모든 상태는 이 한 인스턴스에 있다(1인 — 04 「데이터 저장」).
const HUB_INSTANCE: &str = "owner";

#[event(fetch)]
async fn fetch(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    let path = req.path();
    if path == "/healthz" {
        return Response::from_json(&Health {
            ok: true,
            proto: PROTO_MAJOR,
        });
    }
    if path.starts_with("/api/") || path.starts_with("/agent/") {
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
