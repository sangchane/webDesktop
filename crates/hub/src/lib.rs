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
async fn fetch(req: Request, env: Env, _ctx: Context) -> Result<Response> {
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
        (Method::Post, "/idp/kakao/token") => return kakao_token(req, &env).await,
        (Method::Get, "/idp/kakao/jwks") => {
            let jwk = crypto::public_jwk(&bridge_key(&env)?, kakao::KID)
                .await
                .ok_or("공개키 만들기 실패")?;
            let mut res = Response::ok(format!(r#"{{"keys":[{jwk}]}}"#))?;
            res.headers_mut().set("content-type", "application/json")?;
            return Ok(res);
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

/// 카카오 중계가 ID 토큰을 서명하는 RSA 개인키(PKCS#8 DER, 비밀 KAKAO_BRIDGE_KEY 는 base64).
fn bridge_key(env: &Env) -> Result<Vec<u8>> {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD
        .decode(env.secret("KAKAO_BRIDGE_KEY")?.to_string().trim())
        .map_err(|_| "KAKAO_BRIDGE_KEY 가 base64 가 아님".into())
}

/// 카카오에 인가 코드를 바꿔 받고, 카카오 ID 토큰을 확인한 뒤 허브가 서명한 ID 토큰으로 바꿔 준다(kakao.rs ③).
async fn kakao_token(mut req: Request, env: &Env) -> Result<Response> {
    let auth = req.headers().get("authorization")?;
    // 공개 입구 — 큰 본문은 읽기 전에 거절. 길이 없는 요청은 Access 로그인을 막지 않게 그대로(Workers 자체 상한).
    let len = req
        .headers()
        .get("content-length")?
        .and_then(|v| v.parse::<usize>().ok());
    if len.is_some_and(|n| n > proto::WS_MSG_MAX_BYTES) {
        return problem(ErrorType::TooLarge, None);
    }
    let body = kakao::token_form(&req.text().await?, auth.as_deref());
    let client_id = kakao::form_value(&body, "client_id")
        .unwrap_or_default()
        .to_owned();
    // 우리 카카오 앱만 — 다른 앱의 코드로 허브 서명을 받아 가지 못하게(서명 대행 막기).
    if client_id != env.secret("KAKAO_CLIENT_ID")?.to_string() {
        console_warn!("{{\"event\":\"kakao_bridge_reject\",\"why\":\"client_id\"}}");
        return problem(ErrorType::Unauthorized, None);
    }
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
    let text = res.text().await?;
    if status != 200 {
        let mut out = Response::ok(text)?.with_status(status);
        out.headers_mut().set("cache-control", "no-store")?;
        return Ok(out);
    }
    let reject = |why: &str| {
        console_warn!("{{\"event\":\"kakao_bridge_reject\",\"why\":\"{why}\"}}");
        problem(ErrorType::Unauthorized, None)
    };
    let Ok(mut tokens) = serde_json::from_str::<serde_json::Value>(&text) else {
        return reject("json");
    };
    let Some((signed, header, payload, sig)) =
        tokens["id_token"].as_str().and_then(kakao::split_jwt)
    else {
        return reject("id_token");
    };
    let now_s = (Date::now().as_millis() / 1000) as i64;
    if let Err(why) = kakao::check_kakao(&payload, &client_id, now_s) {
        return reject(why);
    }
    // 카카오 서명 확인 — 공개키는 요청마다 받는다(1인·하루 몇 번).
    let jwks: serde_json::Value = Fetch::Url(Url::parse(kakao::JWKS)?)
        .send()
        .await?
        .json()
        .await?;
    let key = jwks["keys"]
        .as_array()
        .and_then(|ks| ks.iter().find(|k| k["kid"] == header["kid"]))
        .map(|k| k.to_string());
    let verified = match key {
        Some(k) if header["alg"] == "RS256" => {
            crypto::rs256_verify(&k, &sig, signed.as_bytes()).await
        }
        _ => false,
    };
    if !verified {
        return reject("signature");
    }
    let issuer = format!("{}/idp/kakao", env.var("HUB_PUBLIC_URL")?);
    let claims = kakao::bridge_claims(&payload, &client_id, &issuer, now_s);
    let body = kakao::jwt_body(&claims);
    let sig = crypto::rs256_sign(&bridge_key(env)?, body.as_bytes())
        .await
        .ok_or("서명 실패")?;
    tokens["id_token"] = format!("{body}.{}", kakao::b64url(&sig)).into();
    let mut out = Response::ok(tokens.to_string())?;
    out.headers_mut().set("content-type", "application/json")?;
    out.headers_mut().set("cache-control", "no-store")?;
    Ok(out)
}

/// RFC 9457 응답 하나를 만든다 — 거절 갈래의 유일한 출구.
pub(crate) fn problem(kind: ErrorType, detail: Option<String>) -> Result<Response> {
    let p = Problem::new(kind, detail);
    let mut res = Response::from_json(&p)?.with_status(p.status);
    res.headers_mut()
        .set("content-type", "application/problem+json")?;
    Ok(res)
}
