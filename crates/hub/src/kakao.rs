//! 카카오 로그인 중계 — Cloudflare Access(Generic OIDC)가 카카오에 바로 붙지 못하는 두 가지만 고친다(DL-18).
//! ① Access 는 scope 에 `email profile` 을 늘 더하는데 카카오는 이 이름을 몰라 KOE205 로 거절한다 → `openid` 만 남긴다.
//! ② 카카오 토큰 주소는 비밀값을 본문(client_secret_post)으로만 받는다 → Basic 헤더로 오면 본문으로 옮긴다.
//! ③ Access 는 이메일이 없는 신원을 거절하는데 카카오 이메일은 비즈 앱만 받는다 → 카카오 ID 토큰을 확인한 뒤
//!    같은 회원번호로 「<회원번호>@kakao.webdesktop」 를 담은 ID 토큰을 허브 키로 다시 서명해 준다(공개키 /idp/kakao/jwks).
//! 저장하는 것은 없다. 출입 규칙은 Access 가 이 주소 하나로 거른다.
use base64::engine::general_purpose::{STANDARD as B64, URL_SAFE_NO_PAD as B64URL};
use base64::Engine;
use serde_json::{json, Value};

pub const AUTHORIZE: &str = "https://kauth.kakao.com/oauth/authorize";
pub const TOKEN: &str = "https://kauth.kakao.com/oauth/token";
pub const JWKS: &str = "https://kauth.kakao.com/.well-known/jwks.json";
const KAKAO_ISS: &str = "https://kauth.kakao.com";
/// 허브가 서명한 ID 토큰의 키 이름과 수명 — Access 는 받자마자 쓴다.
pub const KID: &str = "kakao-bridge-1";
const BRIDGE_TTL_S: i64 = 300;
/// 카카오와 허브의 시계 차이 허용.
const SKEW_S: i64 = 60;

/// Access 가 보낸 인가 요청 쿼리 → 카카오 인가 주소. scope 만 `openid` 로 바꾼다.
pub fn authorize_url(query: &str) -> String {
    let mut parts: Vec<String> = query
        .split('&')
        .filter(|p| !p.is_empty() && !p.starts_with("scope="))
        .map(str::to_owned)
        .collect();
    parts.push("scope=openid".into());
    format!("{AUTHORIZE}?{}", parts.join("&"))
}

/// 토큰 요청 본문. Basic 헤더(RFC 6749 2.3.1 — 값은 이미 폼 인코딩됨)가 있고 본문에 client_id 가 없으면 본문에 더한다.
pub fn token_form(body: &str, authorization: Option<&str>) -> String {
    let has = |k: &str| body.split('&').any(|p| p.starts_with(&format!("{k}=")));
    let basic = authorization
        .and_then(|a| a.strip_prefix("Basic "))
        .and_then(|b| B64.decode(b.trim()).ok())
        .and_then(|b| String::from_utf8(b).ok());
    match basic.as_deref().and_then(|s| s.split_once(':')) {
        Some((id, secret)) if !has("client_id") => {
            let sep = if body.is_empty() { "" } else { "&" };
            format!("{body}{sep}client_id={id}&client_secret={secret}")
        }
        _ => body.to_owned(),
    }
}

/// 폼 본문에서 값 하나(폼 인코딩 그대로).
pub fn form_value<'a>(body: &'a str, key: &str) -> Option<&'a str> {
    body.split('&')
        .find_map(|p| p.strip_prefix(key)?.strip_prefix('='))
}

/// JWT → (서명할 부분 「머리.몸」, 머리 JSON, 몸 JSON, 서명).
pub fn split_jwt(token: &str) -> Option<(String, Value, Value, Vec<u8>)> {
    let mut it = token.split('.');
    let (h, p, s) = (it.next()?, it.next()?, it.next()?);
    if it.next().is_some() {
        return None;
    }
    let dec = |x: &str| B64URL.decode(x).ok();
    let header = serde_json::from_slice(&dec(h)?).ok()?;
    let payload = serde_json::from_slice(&dec(p)?).ok()?;
    Some((format!("{h}.{p}"), header, payload, dec(s)?))
}

/// 카카오 ID 토큰 몸 확인(서명은 부른 쪽이 따로 확인) — 발급자·대상 앱·만료.
pub fn check_kakao(payload: &Value, client_id: &str, now_s: i64) -> Result<(), &'static str> {
    if payload["iss"] != KAKAO_ISS {
        return Err("iss");
    }
    if payload["aud"] != client_id {
        return Err("aud");
    }
    match payload["exp"].as_i64() {
        Some(exp) if exp + SKEW_S > now_s => {}
        _ => return Err("exp"),
    }
    match payload["sub"].as_str() {
        Some(s) if !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric()) => Ok(()),
        _ => Err("sub"),
    }
}

/// 허브가 서명할 ID 토큰 몸 — 회원번호·별명·nonce 를 옮기고 이메일 자리를 채운다.
pub fn bridge_claims(kakao: &Value, client_id: &str, issuer: &str, now_s: i64) -> Value {
    let sub = kakao["sub"].as_str().unwrap_or_default();
    let mut c = json!({
        "iss": issuer,
        "aud": client_id,
        "sub": sub,
        "email": format!("{sub}@kakao.webdesktop"),
        "email_verified": true,
        "iat": now_s,
        "exp": now_s + BRIDGE_TTL_S,
    });
    for k in ["nonce", "nickname", "auth_time"] {
        if !kakao[k].is_null() {
            c[k] = kakao[k].clone();
        }
    }
    c
}

/// 「머리.몸」 — 서명만 붙이면 되는 JWT 앞부분.
pub fn jwt_body(claims: &Value) -> String {
    let h = json!({ "alg": "RS256", "typ": "JWT", "kid": KID });
    format!(
        "{}.{}",
        B64URL.encode(h.to_string()),
        B64URL.encode(claims.to_string())
    )
}

pub fn b64url(b: &[u8]) -> String {
    B64URL.encode(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authorize_keeps_everything_but_scope() {
        let q = "client_id=abc&redirect_uri=https%3A%2F%2Fx&response_type=code&scope=openid+email+profile&state=s1&code_challenge=c&code_challenge_method=S256";
        assert_eq!(
            authorize_url(q),
            "https://kauth.kakao.com/oauth/authorize?client_id=abc&redirect_uri=https%3A%2F%2Fx&response_type=code&state=s1&code_challenge=c&code_challenge_method=S256&scope=openid"
        );
        assert_eq!(authorize_url(""), format!("{AUTHORIZE}?scope=openid"));
    }

    fn kakao_payload() -> Value {
        json!({"iss": KAKAO_ISS, "aud": "app1", "sub": "4012345678", "exp": 2000, "iat": 1000,
               "nonce": "n1", "nickname": "상찬", "auth_time": 999})
    }

    #[test]
    fn kakao_token_checks() {
        let p = kakao_payload();
        assert_eq!(check_kakao(&p, "app1", 1500), Ok(()));
        assert_eq!(check_kakao(&p, "app2", 1500), Err("aud"), "다른 앱의 토큰");
        assert_eq!(check_kakao(&p, "app1", 2100), Err("exp"));
        let mut q = p.clone();
        q["iss"] = json!("https://evil.example");
        assert_eq!(check_kakao(&q, "app1", 1500), Err("iss"));
        q = p.clone();
        q["sub"] = json!("1@x.y");
        assert_eq!(
            check_kakao(&q, "app1", 1500),
            Err("sub"),
            "주소를 꾸밀 글자는 거절"
        );
    }

    #[test]
    fn bridge_token_carries_member_number_and_nonce() {
        let c = bridge_claims(&kakao_payload(), "app1", "https://hub/idp/kakao", 1500);
        assert_eq!(c["email"], "4012345678@kakao.webdesktop");
        assert_eq!(c["sub"], "4012345678");
        assert_eq!(c["aud"], "app1");
        assert_eq!(c["nonce"], "n1");
        assert_eq!(c["exp"], 1500 + BRIDGE_TTL_S);
        let (signed, h, p, sig) = split_jwt(&format!("{}.c2ln", jwt_body(&c))).unwrap();
        assert_eq!(h["kid"], KID);
        assert_eq!(p, c);
        assert_eq!(sig, b"sig");
        assert!(signed.ends_with(&signed[signed.find('.').unwrap()..]));
        assert!(split_jwt("a.b").is_none());
        assert_eq!(
            form_value("a=1&client_id=x%2B&c=3", "client_id"),
            Some("x%2B")
        );
        assert_eq!(form_value("client_idx=1", "client_id"), None);
    }

    #[test]
    fn basic_auth_moves_into_body_once() {
        let auth = format!("Basic {}", B64.encode("id1:sec%2B1"));
        assert_eq!(
            token_form("grant_type=authorization_code&code=c", Some(&auth)),
            "grant_type=authorization_code&code=c&client_id=id1&client_secret=sec%2B1"
        );
        // 이미 본문에 있으면 그대로(두 번 넣지 않음).
        let body = "grant_type=authorization_code&client_id=id1&client_secret=s";
        assert_eq!(token_form(body, Some(&auth)), body);
        assert_eq!(token_form(body, None), body);
        assert_eq!(token_form("a=b", Some("Bearer x")), "a=b");
    }
}
