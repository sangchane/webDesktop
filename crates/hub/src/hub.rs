//! Durable Object `Hub` — 기기·세션·신호의 단일 상태 (05 「저장소 설계」). T2 는 뼈대만.
use proto::ErrorType;
use worker::*;

#[durable_object]
pub struct Hub {
    #[allow(dead_code)]
    state: State,
    #[allow(dead_code)]
    env: Env,
}

impl DurableObject for Hub {
    fn new(state: State, env: Env) -> Self {
        Self { state, env }
    }

    async fn fetch(&self, _req: Request) -> Result<Response> {
        crate::problem(ErrorType::NotFound, None)
    }
}
