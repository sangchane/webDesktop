//! 입구 계층 — Cloudflare Access JWT(`Cf-Access-Jwt-Assertion`) 서명·aud·iss·만료 확인 (05 「규약」 인증).
//! 공개키는 https://<팀>/cdn-cgi/access/certs, 시험 때는 ACCESS_TEST_JWKS(로컬에서만 — entry 설정 검사).
use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64URL;
use base64::Engine;
use proto::ErrorType;
use serde::Deserialize;
use std::cell::RefCell;

#[derive(Deserialize)]
struct Header {
    alg: String,
    kid: String,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Aud {
    One(String),
    Many(Vec<String>),
}

#[derive(Deserialize)]
struct Claims {
    aud: Aud,
    iss: String,
    exp: i64,
    nbf: Option<i64>,
}

/// 공개키 캐시 수명 — Cloudflare 가 키를 돌리면 이 안에 따라간다.
const JWKS_TTL_MS: i64 = 3_600_000;
/// nbf 시계 차이 허용.
const NBF_SKEW_MS: i64 = 60_000;

#[derive(Deserialize, Clone)]
struct Jwks {
    keys: Vec<serde_json::Value>,
}

pub struct Access {
    pub team_domain: String,
    pub aud: String,
    /// Some = 시험용 JWKS(JSON 글).
    pub test_jwks: Option<String>,
}

/// DO 메모리의 공개키 캐시 (받은 시각, 키) — kid 가 없거나 JWKS_TTL_MS 지나면 다시 받는다.
#[derive(Default)]
pub struct JwksCache(RefCell<Option<(i64, Jwks)>>);

impl Access {
    pub async fn check(
        &self,
        token: Option<String>,
        now_ms: i64,
        cache: &JwksCache,
    ) -> Result<(), ErrorType> {
        let token = token.ok_or(ErrorType::Unauthorized)?;
        let mut parts = token.split('.');
        let (Some(h), Some(p), Some(s), None) =
            (parts.next(), parts.next(), parts.next(), parts.next())
        else {
            return Err(ErrorType::Unauthorized);
        };
        let dec = |x: &str| B64URL.decode(x).map_err(|_| ErrorType::Unauthorized);
        let header: Header =
            serde_json::from_slice(&dec(h)?).map_err(|_| ErrorType::Unauthorized)?;
        let claims: Claims =
            serde_json::from_slice(&dec(p)?).map_err(|_| ErrorType::Unauthorized)?;
        let aud_ok = match &claims.aud {
            Aud::One(a) => *a == self.aud,
            Aud::Many(v) => v.contains(&self.aud),
        };
        if header.alg != "RS256"
            || !aud_ok
            || claims.iss != format!("https://{}", self.team_domain)
            || claims.exp * 1000 <= now_ms
            || claims.nbf.is_some_and(|n| n * 1000 > now_ms + NBF_SKEW_MS)
        {
            return Err(ErrorType::Unauthorized);
        }
        let jwk = self
            .key(&header.kid, now_ms, cache)
            .await?
            .ok_or(ErrorType::Unauthorized)?;
        let signed = format!("{h}.{p}");
        if crate::crypto::rs256_verify(&jwk, &dec(s)?, signed.as_bytes()).await {
            Ok(())
        } else {
            Err(ErrorType::Unauthorized)
        }
    }

    async fn key(
        &self,
        kid: &str,
        now_ms: i64,
        cache: &JwksCache,
    ) -> Result<Option<String>, ErrorType> {
        let find = |j: &Jwks| {
            j.keys
                .iter()
                .find(|k| k["kid"] == kid)
                .map(|k| k.to_string())
        };
        let cached = cache
            .0
            .borrow()
            .as_ref()
            .filter(|(at, _)| now_ms - at < JWKS_TTL_MS)
            .and_then(|(_, j)| find(j));
        if let Some(k) = cached {
            return Ok(Some(k));
        }
        let fresh = self.fetch().await?;
        let k = find(&fresh);
        *cache.0.borrow_mut() = Some((now_ms, fresh));
        Ok(k)
    }

    async fn fetch(&self) -> Result<Jwks, ErrorType> {
        if let Some(t) = &self.test_jwks {
            return serde_json::from_str(t).map_err(|_| ErrorType::Internal);
        }
        let url = format!("https://{}/cdn-cgi/access/certs", self.team_domain);
        let mut res = worker::Fetch::Url(url.parse().map_err(|_| ErrorType::Internal)?)
            .send()
            .await
            .map_err(|_| ErrorType::Internal)?;
        res.json().await.map_err(|_| ErrorType::Internal)
    }
}
