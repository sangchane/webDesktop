//! 입구 계층 보조 — Workers WebCrypto(난수·Ed25519·RS256 확인·카카오 중계 RS256 서명). 순수 Rust 서명 크레이트를 더 들이지 않으려고 런타임 것을 쓴다.
use worker::js_sys::{global, Array, Object, Reflect, Uint8Array, JSON};
use worker::wasm_bindgen::{JsCast, JsValue};
use worker::wasm_bindgen_futures::JsFuture;
use worker::web_sys::{Crypto, CryptoKey};

fn crypto() -> Crypto {
    Reflect::get(&global(), &"crypto".into())
        .expect("Workers 에는 crypto 가 있다")
        .unchecked_into()
}

pub fn random<const N: usize>() -> [u8; N] {
    let mut b = [0u8; N];
    crypto()
        .get_random_values_with_u8_array(&mut b)
        .expect("getRandomValues");
    b
}

async fn verify(format: &str, key: &Object, alg: &str, sig: &[u8], data: &[u8]) -> bool {
    let run = async {
        let subtle = crypto().subtle();
        let algo: Object = JSON::parse(alg)?.unchecked_into();
        let usages = Array::of1(&"verify".into());
        let key: CryptoKey =
            JsFuture::from(subtle.import_key_with_object(format, key, &algo, false, &usages)?)
                .await?
                .unchecked_into();
        let ok = JsFuture::from(
            subtle.verify_with_object_and_u8_array_and_u8_array(&algo, &key, sig, data)?,
        )
        .await?;
        Ok::<_, JsValue>(ok.as_bool() == Some(true))
    };
    run.await.unwrap_or(false)
}

/// Ed25519 공개키(32바이트) 서명 확인 — E-05.
pub async fn ed25519_verify(pubkey: &[u8], sig: &[u8], data: &[u8]) -> bool {
    let key: Object = Uint8Array::from(pubkey).into();
    verify("raw", &key, r#"{"name":"Ed25519"}"#, sig, data).await
}

/// RS256(RSASSA-PKCS1-v1_5 + SHA-256) 를 JWK 공개키로 확인 — Access JWT.
pub async fn rs256_verify(jwk_json: &str, sig: &[u8], data: &[u8]) -> bool {
    let Ok(key) = JSON::parse(jwk_json) else {
        return false;
    };
    verify(
        "jwk",
        key.unchecked_ref(),
        r#"{"name":"RSASSA-PKCS1-v1_5","hash":"SHA-256"}"#,
        sig,
        data,
    )
    .await
}

const RS256: &str = r#"{"name":"RSASSA-PKCS1-v1_5","hash":"SHA-256"}"#;

/// RSA 개인키(PKCS#8 DER) 가져오기 — 카카오 중계가 ID 토큰을 서명할 때.
async fn import_pkcs8(der: &[u8], extractable: bool) -> Result<CryptoKey, JsValue> {
    let algo: Object = JSON::parse(RS256)?.unchecked_into();
    let data: Object = Uint8Array::from(der).into();
    let usages = Array::of1(&"sign".into());
    let key =
        crypto()
            .subtle()
            .import_key_with_object("pkcs8", &data, &algo, extractable, &usages)?;
    Ok(JsFuture::from(key).await?.unchecked_into())
}

/// RS256 서명.
pub async fn rs256_sign(pkcs8: &[u8], data: &[u8]) -> Option<Vec<u8>> {
    let run = async {
        let key = import_pkcs8(pkcs8, false).await?;
        let algo: Object = JSON::parse(RS256)?.unchecked_into();
        let sig = crypto()
            .subtle()
            .sign_with_object_and_u8_array(&algo, &key, data)?;
        Ok::<_, JsValue>(Uint8Array::new(&JsFuture::from(sig).await?).to_vec())
    };
    run.await.ok()
}

/// 개인키에서 공개 JWK(kty·n·e + kid) 만 꺼낸다 — /idp/kakao/jwks.
pub async fn public_jwk(pkcs8: &[u8], kid: &str) -> Option<String> {
    let run = async {
        let key = import_pkcs8(pkcs8, true).await?;
        let jwk = JsFuture::from(crypto().subtle().export_key("jwk", &key)?).await?;
        let s: String = JSON::stringify(&jwk)?.into();
        Ok::<_, JsValue>(s)
    };
    let full: serde_json::Value = serde_json::from_str(&run.await.ok()?).ok()?;
    Some(
        serde_json::json!({
            "kty": full["kty"], "n": full["n"], "e": full["e"],
            "kid": kid, "alg": "RS256", "use": "sig",
        })
        .to_string(),
    )
}
