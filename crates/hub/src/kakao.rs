//! 카카오 로그인 중계 — Cloudflare Access(Generic OIDC)가 카카오에 바로 붙지 못하는 두 가지만 고친다(DL-18).
//! ① Access 는 scope 에 `email profile` 을 늘 더하는데 카카오는 이 이름을 몰라 KOE205 로 거절한다 → `openid` 만 남긴다.
//! ② 카카오 토큰 주소는 비밀값을 본문(client_secret_post)으로만 받는다 → Basic 헤더로 오면 본문으로 옮긴다.
//! ID 토큰은 카카오가 서명한 그대로 돌려준다 — 이 중계는 아무것도 서명·저장하지 않는다.
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;

pub const AUTHORIZE: &str = "https://kauth.kakao.com/oauth/authorize";
pub const TOKEN: &str = "https://kauth.kakao.com/oauth/token";

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
