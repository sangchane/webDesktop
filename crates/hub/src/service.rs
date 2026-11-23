//! 서비스 계층 — 등록·오프라인 판정·빈도 규칙. 순수 함수(시계·난수는 인자로) — SQL·HTTP 없음.
use proto::{
    ErrorType, INSTALL_LINK_TTL_MIN, OFFLINE_DETECT_SEC, OFFLINE_WARN_HOURS, RATE_LIMIT_PER_MIN,
    REG_CODE_ALPHABET, REG_CODE_LEN, REG_CODE_MAX_TRIES, REG_CODE_TTL_MIN,
};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::HashMap;

pub const CODE_TTL_MS: i64 = (REG_CODE_TTL_MIN * 60_000) as i64;
pub const INSTALL_TTL_MS: i64 = (INSTALL_LINK_TTL_MIN * 60_000) as i64;
pub const OFFLINE_MS: i64 = (OFFLINE_DETECT_SEC * 1000) as i64;
const OFFLINE_WARN_MS: i64 = (OFFLINE_WARN_HOURS * 3_600_000) as i64;

/// 활성 등록 코드 행 (enroll_code, id=1).
#[derive(Debug, Clone, Deserialize)]
pub struct CodeRow {
    pub code_hash: String,
    pub install_token_hash: String,
    pub expires_at: i64,
    pub tries: u32,
}

/// 코드·토큰은 원문을 저장하지 않는다 — sha256 hex.
pub fn hash(s: &str) -> String {
    Sha256::digest(s.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// 난수 바이트 → 등록 코드. 글자 32개라 `b % 32` 가 고르다.
pub fn make_code(rand: &[u8; REG_CODE_LEN]) -> String {
    rand.iter()
        .map(|b| REG_CODE_ALPHABET[*b as usize % REG_CODE_ALPHABET.len()] as char)
        .collect()
}

/// uuid v7 (RFC 9562): 48비트 밀리초 + 판·변형 비트 + 난수.
pub fn uuid_v7(now_ms: i64, rand: &[u8; 10]) -> String {
    let mut b = [0u8; 16];
    b[..6].copy_from_slice(&(now_ms as u64).to_be_bytes()[2..]);
    b[6..].copy_from_slice(rand);
    b[6] = 0x70 | (b[6] & 0x0f);
    b[8] = 0x80 | (b[8] & 0x3f);
    let h: String = b.iter().map(|x| format!("{x:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &h[..8],
        &h[8..12],
        &h[12..16],
        &h[16..20],
        &h[20..]
    )
}

/// 등록 판정 결과 (05 「트랜잭션」 등록).
#[derive(Debug, PartialEq)]
pub enum CodeVerdict {
    /// 맞음 → 코드 행 삭제 + 기기 생성.
    Ok,
    /// 활성 코드 없음 → `code_invalid`.
    NoCode,
    /// 만료 → 행 삭제 + `code_expired`.
    Expired,
    /// 틀림 → tries 갱신, 상한이면 행 삭제(`discard`) + `code_invalid`.
    Wrong { tries: u32, discard: bool },
}

pub fn judge_code(row: Option<&CodeRow>, given: &str, now_ms: i64) -> CodeVerdict {
    let Some(row) = row else {
        return CodeVerdict::NoCode;
    };
    if now_ms >= row.expires_at {
        return CodeVerdict::Expired;
    }
    if hash(given) == row.code_hash {
        return CodeVerdict::Ok;
    }
    let tries = row.tries + 1;
    CodeVerdict::Wrong {
        tries,
        discard: tries >= REG_CODE_MAX_TRIES,
    }
}

/// 설치 링크 토큰 판정 (E-12). 수명 안에서는 몇 번이고 통과.
pub fn judge_install(row: Option<&CodeRow>, token: &str, now_ms: i64) -> Result<(), ErrorType> {
    match row {
        Some(r) if hash(token) == r.install_token_hash => {
            if now_ms >= r.expires_at - CODE_TTL_MS + INSTALL_TTL_MS {
                Err(ErrorType::CodeExpired)
            } else {
                Ok(())
            }
        }
        _ => Err(ErrorType::CodeInvalid),
    }
}

/// 마지막 신호가 OFFLINE_DETECT_SEC 넘게 없으면 끊긴 연결로 본다(half-open 포함).
pub fn is_stale(last_ms: i64, now_ms: i64) -> bool {
    now_ms - last_ms > OFFLINE_MS
}

/// 목록의 「오래 꺼짐」 (FR-003).
pub fn offline_long(online: bool, last_ms: i64, now_ms: i64) -> bool {
    !online && now_ms - last_ms > OFFLINE_WARN_MS
}

/// IP 마다 분당 RATE_LIMIT_PER_MIN. DO 메모리라 휴면하면 초기화(1인이라 수용 — SPEC 바닥선 11).
#[derive(Default)]
pub struct RateLimit {
    hits: HashMap<String, (i64, u32)>,
}

impl RateLimit {
    /// 허용이면 true.
    pub fn hit(&mut self, key: &str, now_ms: i64) -> bool {
        let minute = now_ms / 60_000;
        self.hits.retain(|_, (m, _)| *m == minute);
        let e = self.hits.entry(key.to_owned()).or_insert((minute, 0));
        e.1 += 1;
        e.1 <= RATE_LIMIT_PER_MIN
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(code: &str, tries: u32) -> CodeRow {
        CodeRow {
            code_hash: hash(code),
            install_token_hash: hash("tok"),
            expires_at: 1_000 + CODE_TTL_MS,
            tries,
        }
    }

    #[test]
    fn code_uses_alphabet_only() {
        let c = make_code(&[0, 31, 32, 255, 7, 8, 100, 200]);
        assert!(proto::reg_code_ok(&c), "{c}");
    }

    #[test]
    fn uuid_v7_shape() {
        let id = uuid_v7(0x0123_4567_89ab, &[0xff; 10]);
        assert_eq!(id, "01234567-89ab-7fff-bfff-ffffffffffff");
    }

    #[test]
    fn judge_code_rules() {
        let r = row("ABCD2345", 0);
        assert_eq!(judge_code(None, "ABCD2345", 1_000), CodeVerdict::NoCode);
        assert_eq!(judge_code(Some(&r), "ABCD2345", 1_000), CodeVerdict::Ok);
        assert_eq!(
            judge_code(Some(&r), "ABCD2345", 1_000 + CODE_TTL_MS),
            CodeVerdict::Expired
        );
        assert_eq!(
            judge_code(Some(&r), "WRONG", 1_000),
            CodeVerdict::Wrong {
                tries: 1,
                discard: false
            }
        );
        let last = row("ABCD2345", REG_CODE_MAX_TRIES - 1);
        assert_eq!(
            judge_code(Some(&last), "WRONG", 1_000),
            CodeVerdict::Wrong {
                tries: REG_CODE_MAX_TRIES,
                discard: true
            }
        );
    }

    #[test]
    fn judge_install_rules() {
        let r = row("ABCD2345", 0);
        assert_eq!(judge_install(Some(&r), "tok", 1_000), Ok(()));
        assert_eq!(
            judge_install(Some(&r), "bad", 1_000),
            Err(ErrorType::CodeInvalid)
        );
        assert_eq!(
            judge_install(None, "tok", 1_000),
            Err(ErrorType::CodeInvalid)
        );
        assert_eq!(
            judge_install(Some(&r), "tok", 1_000 + INSTALL_TTL_MS),
            Err(ErrorType::CodeExpired)
        );
    }

    #[test]
    fn offline_rules() {
        assert!(!is_stale(0, OFFLINE_MS));
        assert!(is_stale(0, OFFLINE_MS + 1));
        assert!(!offline_long(true, 0, i64::MAX));
        assert!(offline_long(false, 0, OFFLINE_WARN_MS + 1));
    }

    #[test]
    fn rate_limit_per_minute() {
        let mut rl = RateLimit::default();
        for _ in 0..RATE_LIMIT_PER_MIN {
            assert!(rl.hit("ip", 0));
        }
        assert!(!rl.hit("ip", 59_999));
        assert!(rl.hit("other", 0));
        assert!(rl.hit("ip", 60_000));
    }
}
