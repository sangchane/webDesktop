# 첫 작업 3 — macOS 에이전트 시작점 (2026-11-24, Windows 세션이 남김)

사용자 2026-11-24 「git hub 링크 통해서 소스 받아서 맥에서 작업할게」. 맥에서 이 저장소를 받아 macOS 호스트(에이전트)를 만든다.
목표·범위: `autopilot/webdesktop/08-readiness-report.md` 첫 작업 3(macOS 최소 화면 + 권한 유지 + 로그인 창, 가정 B12·M1, MAC_OS_PROMPTS 실측), 설계 `04-architecture.md` 「macOS 캡처」「인코딩」「입력 전달」 줄.

## 같이 쓰는 것 (고치지 않음)
허브(`crates/hub`)·웹앱(`crates/web`, `crates/app`)·계약(`crates/proto`)·에이전트 상주(`resident.rs` 등록·재접속)·세션 고리(`helper/mod.rs` str0m·STUN·D-08). 웹앱 목록에 Windows PC 와 맥이 같이 뜬다.

## 맥에서 지금 막히는 곳 (Windows 에서 확인한 것 — 맥에서 빌드해 본 것은 아님)
1. `crates/agent/Cargo.toml` 의 str0m 이 `features = ["wincrypto"]`(Windows 전용). 맥은 `apple-crypto` 기능이 있다(str0m 0.26) → 대상별 의존으로 나눈다.
2. `crates/agent/src/platform/mod.rs` 가 `#[cfg(windows)]` 만 있다. 맥 구현이 채울 것(같은 이름):
   - `capture::Capturer::{new, next}` → ScreenCaptureKit (BGRA 한 장, 바뀐 장만 Some)
   - `inject::Injector` → CGEvent (HID 사용 코드 → 맥 키 코드 표, 글자는 `CGEventKeyboardSetUnicodeString`, Drop 때 눌린 키 놓기)
   - `protect`/`unprotect`/`write_private`/`create_private_dir` → 키체인(root) 또는 0600 파일 (04 B2-S·B4)
3. 인코딩은 VideoToolbox(하드웨어)가 설계 기본, 실패 시 openh264(`helper/video.rs`). Windows 는 아직 openh264 뿐이고 지연 대부분이 인코딩(DL-23).
4. `scripts/verify.ps1` 은 PowerShell — 맥은 `brew install powershell` 후 `pwsh scripts/verify.ps1`, 또는 안의 cargo 명령을 그대로.
5. 시험 `tools/agent.test.mjs` 는 실제 에이전트를 띄워 실제 화면을 받는다 — 맥에서는 화면 기록 권한을 먼저 준다.

## 맥에서 처음 할 일
1. `rustup`·`node`·`npm i`(playwright) 설치, `cargo build -p agent` 로 위 1·2 오류부터 본다.
2. 운영 허브에 붙이려면 등록 코드가 필요 — 웹앱 「기기 추가」(사용자 카카오 로그인). Cloudflare 배포가 필요하면 맥에서 `npx wrangler login`(같은 계정, AGENTS.md — 값은 옮겨 적지 않는다).
3. 두 PC 가 같은 `main` 에 올린다 — 시작 전 `git pull --rebase`, 커밋마다 `git push`. NEXT.md 는 양쪽이 쓰므로 맥 진행은 이 파일 아래 「진행」에 적고 NEXT.md 는 한 줄만 고친다.

## 진행
(맥 세션이 여기에 적는다)
