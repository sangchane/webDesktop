//! 입구 계층 보조 — Workers WebCrypto(난수·Ed25519·RS256 검증). 순수 Rust 서명 크레이트를 더 들이지 않으려고 런타임 것을 쓴다.
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
