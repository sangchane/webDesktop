//! 공유 계약 계층 — 허브·에이전트·웹이 같이 쓰는 메시지 이름·오류 이름 (05-api-contract.md).
use serde::{Deserialize, Serialize};

/// 판 major. 다르면 상대는 `upgrade_required` 로 닫는다 (05 「규약」).
pub const PROTO_MAJOR: u32 = 1;

/// 05 「오류 이름」 전체. 직렬화 이름이 RFC 9457 `type` 값이다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorType {
    Unauthorized,
    NotFound,
    CodeInvalid,
    CodeExpired,
    DeviceOffline,
    DeviceRevoked,
    BadSignature,
    UpgradeRequired,
    RateLimited,
    TooLarge,
    Internal,
}

impl ErrorType {
    /// 05 표의 HTTP 상태.
    pub fn status(self) -> u16 {
        match self {
            Self::Unauthorized | Self::BadSignature => 401,
            Self::NotFound => 404,
            Self::CodeInvalid | Self::CodeExpired => 400,
            Self::DeviceOffline => 409,
            Self::DeviceRevoked => 403,
            Self::UpgradeRequired => 426,
            Self::RateLimited => 429,
            Self::TooLarge => 413,
            Self::Internal => 500,
        }
    }
}

/// RFC 9457 Problem JSON.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Problem {
    #[serde(rename = "type")]
    pub kind: ErrorType,
    pub title: String,
    pub status: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl Problem {
    pub fn new(kind: ErrorType, detail: Option<String>) -> Self {
        let title = serde_json::to_value(kind)
            .ok()
            .and_then(|v| v.as_str().map(str::to_owned))
            .unwrap_or_default();
        Self {
            kind,
            title,
            status: kind.status(),
            detail,
        }
    }
}

/// E-11 `/healthz` 응답.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Health {
    pub ok: bool,
    pub proto: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn problem_uses_snake_case_type_and_status() {
        let p = Problem::new(ErrorType::CodeExpired, None);
        let v = serde_json::to_value(&p).unwrap();
        assert_eq!(v["type"], "code_expired");
        assert_eq!(v["status"], 400);
        let back: Problem = serde_json::from_value(v).unwrap();
        assert_eq!(back, p);
    }
}
