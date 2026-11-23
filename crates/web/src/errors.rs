//! 오류 이름 → 화면 문구 (05 「사용자 영향」 — 표 하나). 원인과 할 일을 같이 쓴다.
use proto::{EndReason, ErrorType};

/// 화면이 받는 실패 — 허브가 이름을 준 것과 허브에 닿지 못한 것.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fail {
    Hub(ErrorType),
    Network,
}

pub fn text(f: Fail) -> &'static str {
    use ErrorType::*;
    match f {
        Fail::Network => "허브에 연결할 수 없습니다",
        Fail::Hub(e) => match e {
            Unauthorized => "로그인이 끝났습니다. 새로 고쳐 다시 로그인하세요",
            NotFound => "기기를 찾을 수 없습니다. 목록을 새로 고치세요",
            BadRequest => "요청 꼴이 맞지 않습니다. 웹앱을 새로 고치세요",
            CodeInvalid => "코드가 맞지 않습니다",
            CodeExpired => "코드가 만료되었습니다. 새 코드를 받으세요",
            DeviceOffline => "기기가 꺼져 있습니다",
            DeviceRevoked => "제거한 기기입니다",
            BadSignature => "기기 서명이 맞지 않습니다",
            UpgradeRequired => "웹앱이 옛 판입니다. 새로 고치세요",
            RateLimited => "요청이 너무 잦습니다. 1분 뒤 다시 하세요",
            TooLarge => "보낸 내용이 너무 큽니다",
            Internal => "허브에서 오류가 났습니다. 잠시 뒤 다시 하세요",
        },
    }
}

/// 상대가 끝낸 세션의 이유 → 목록 위 알림. 내가 닫은 것은 알리지 않는다.
pub fn ended(r: EndReason) -> Option<&'static str> {
    use EndReason::*;
    match r {
        ViewerClosed => None,
        TakenOver => Some("다른 기기에서 이 화면을 열었습니다"),
        HostClosed => Some("기기가 화면 공유를 끝냈습니다"),
        HostShutdown => Some("기기가 꺼졌습니다"),
        Failed => Some("연결이 끊겼습니다"),
        DeviceRevoked => Some("제거한 기기입니다"),
    }
}
