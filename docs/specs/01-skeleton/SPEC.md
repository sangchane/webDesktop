# SPEC: 첫 작업 1 — 허브 + 웹 + Windows 에이전트 최소 화면 (워킹 스켈레톤)
버전: v1.0 · 2026-11-23 · 기준 03 v1.2 · 05 v1.2 · 08 v1.0

## 배경
- 설계(autopilot/webdesktop/)가 끝났다. 위험 상위 P1(iPhone 웹 조작감)·H1(DO 휴면 연결)·H2(Access 경로 우회)를 실제로 확인해야 나머지가 의미 있다(08 「첫 작업 1」).
- 이 작업은 핵심 여정 하나를 얇게 끝까지 뚫는다: **PC 추가 → iPhone 웹앱에서 목록 → 눌러서 화면 → 클릭·한글 입력**.

## 현재 상태
- 코드 없음(빈 저장소, 2026-11-23 `git init`). 도구: Rust 1.100(msvc) + wasm32 타깃, Node 26, VS 2022 Build Tools, Windows SDK 10.0.26100, wrangler(npx) OAuth 로그인(Workers Paid 계정 — AGENTS.md).

## 제안 변경 (이 작업에서 만드는 것)
| 부품 | 파일 | 이번 범위 | 미룸(다음 작업) |
|---|---|---|---|
| 공유 계약 | `crates/proto/src/lib.rs` | 05 의 E-03·E-04·E-05·E-06·E-07, M-01~M-04·M-09·M-10, D-01·D-02·D-03·D-08 메시지 타입, 오류 이름 전체, 꼴 검사(`validate()`·`parse()`) — 웹앱도 같은 크레이트 | M-05~M-08·M-11~M-13, D-04~D-07 |
| 허브 | `crates/hub/` (Worker + DO `Hub`) | `/healthz`, 정적 웹앱, Access JWT 검증, 등록 코드(활성 1개), `/agent/install/windows?t=`, `/agent/enroll`, `/agent/ws`(nonce 서명), `/api/ws-ticket` + `/ws/viewer`(DL-10), `/api/devices`, `/api/sessions`(TURN 자격 + 이어받기), 신호 중계, `session.viewer_stats` 저장, 오프라인 판정, ping 자동 응답 | 접속 기록 화면 E-09·기기 제거 E-08·업데이트 E-10·만료 정리 알람·VPN |
| 웹앱 | `crates/web/` (Rust → wasm, Leptos CSR, trunk — DL-13) | 목록·기기 추가(코드·설치 링크)·원격 화면(WebRTC 수신), 입력 변환기(마우스·터치 트랙패드·`text` 한글·특수키 막대 최소: Esc·Tab·Ctrl·Alt·Win·Del), `?relay=1`, M-10 계측(첫 프레임·SC-003 측정법) | 클립보드·모니터 선택·화질·Keyboard Lock·접속 기록 화면 |
| 에이전트 | `crates/agent/` (Windows, 콘솔 실행) | 설정 파일·Ed25519 키(DPAPI + 파일 ACL), 등록(코드 입력 프롬프트), 허브 WS·재접속, 세션: DXGI Desktop Duplication(기본 모니터) → `openh264` → `str0m` H.264 트랙, DataChannel 입력 → `SendInput`(마우스·HID 키·유니코드 글자), D-08 `input.applied` | Windows 서비스·SYSTEM 도우미·보안 데스크톱(첫 작업 2), HW 인코더, macOS(첫 작업 3), 자동 업데이트 |

- **웹앱도 Rust**: 사용자 원문 「RUST 기반 … 웹앱」(DL-13). Leptos CSR 하나만 더하고 WebRTC·입력은 web-sys 로 직접. 사유: 의존 최소(바닥선 17).
- **`just` 대신 `scripts/verify.ps1`**: 새 도구 설치를 피한다(07 의 `just verify` 를 이 스크립트로 대신 — decision-log 에 기록).
- **DXGI·SendInput 은 사용자 세션의 일반 프로세스로**(이번은 콘솔 실행) — 잠금 화면은 첫 작업 2.

## 코드 위치와 바닥선
- 04 「코드 구조」 그대로: `crates/proto` · `crates/hub/src/{entry,service,store}` · `crates/agent/src/{resident,helper,platform/windows}` · `crates/web/src`.
- 바닥선:
  - 1 계층: 허브 `entry`(경로·인증·꼴 검사) → `service`(규칙) → `store`(SQL). 에이전트 `resident`(허브 연결·등록) / `helper`(세션) / `platform/windows`(OS 호출).
  - 3 오류: `proto::ErrorType` 로 거절, 그 밖은 `internal` + error 로그.
  - 4 꼴 검사: `proto` 의 `validate()` 를 허브 `entry` 한 곳에서.
  - 5 로그: 허브 `console_log!` 에 JSON 한 줄(사건 이름은 07 목록), 에이전트 `tracing-subscriber` JSON. 금지 칸 지킴.
  - 6 DB: `crates/hub/migrations/0001_init.sql`(device·enroll_code·session), DO 생성 때 적용, 두 번째는 적용 0. 새 쿼리는 05 색인 대조표 안에서만.
  - 6 트랜잭션: 등록 = 05 「트랜잭션」 그대로(`transactionSync` 대신 DO 단일 스레드 + SQL 한 묶음).
  - 8 설정: 허브 비밀 검사(빠지면 500 `internal`), 에이전트 설정 검사 실패 시 기동 거부.
  - 11 빈도 제한: 허브 입구 IP·기기 RATE_LIMIT_PER_MIN(DO 메모리 계수기 — 휴면하면 초기화, 1인이라 수용).
  - 15 공유 계약: 허브·에이전트·웹앱이 `proto` 크레이트 하나를 씀(생성 단계 없음).
  - 16·17 품질·의존: `scripts/verify.ps1`, `Cargo.lock`·`package-lock.json`, `cargo deny`(AGPL/GPL 금지).

## 완료 기준 (진행 장부: tasks.md)
| # | 기준 | 요구 ID | 검증 |
|---|---|---|---|
| C1 | `scripts/verify.ps1` 가 fmt·clippy(웹앱 wasm 포함)·test·deny 를 한 번에 돌리고 종료 코드 0 | 바닥선 16 | `pwsh scripts/verify.ps1` → `VERIFY OK` |
| C2 | 허브가 Cloudflare 에 올라가 `/healthz` 가 `{"ok":true,"proto":1}` | E-11 | `curl -s https://webdesktop-hub.<계정>.workers.dev/healthz` |
| C3 | Access 켠 뒤: `/api/devices` 는 로그인 없이 401 또는 Access 로그인 화면, `/agent/enroll`·`/healthz` 는 허브 응답(우회) | FR-001, H2 | `node tools/check-access.mjs` → `ACCESS OK` |
| C4 | 허브 통합 시험(로컬 `wrangler dev`): 등록 코드 만료·대입·재사용·새 코드, nonce 서명 성공/실패, 세션 이어받기, 오프라인 판정이 06 표대로 | FR-002·003·004 | `node --test tools/hub.test.mjs` → `# fail 0` |
| C5 | 깨끗한 PC 시나리오: 웹앱 「기기 추가」 → 설치 링크로 에이전트 받기 → 실행 → 코드 입력 → 목록 「온라인」. 사용자 행동 3, 로그인·허브 주소 입력 0 | SC-001(Windows 일부 — 서비스 설치 UAC 는 첫 작업 2) | 체크리스트 `docs/specs/01-skeleton/setup-check.md` |
| C6 | PC 브라우저(같은 PC 의 Chrome)에서 집 PC 화면이 보이고 클릭·타자가 된다 | FR-004·005·006 | 수동 1회 + 접속 기록 M-10 값 |
| C7 | `?relay=1` 세션이 TURN 경로로 연결되고 path=relay 가 저장된다 | FR-004 | `node tools/report.mjs --last` → `path=relay` |
| C8 | **iPhone Safari LTE** 로 집 PC 를 열어 클릭·한글 「안녕하세요 원격 테스트」 입력 | SC-002·003·006, 위험 P1 | `node tools/report.mjs --since <시각>` → SC-002 9/10·SC-003 p50·SC-006 일치 판정 |
| C9 | DO 휴면 연결 24시간 유지(에이전트 온라인 표시 유지) | 위험 H1 | `node tools/report.mjs --uptime <device>` |

정답지 채점: C8 의 SC-002·003·006 은 실기기 계측 값을 03 상수(CONNECT_MAX_SEC·LATENCY_P50_MS)와 문장 일치로 판정 — 맞은 수/전체로 보고.

## 테스트 계획
- 단위(`cargo test`): proto 왕복·꼴 검사, 허브 service(등록 규칙·코드 생성 알파벳·이어받기·오프라인 판정 — 순수 함수로 떼어 시계 주입), 에이전트 좌표 변환·HID→VK 표·Ed25519 서명.
- 단위(웹, `cargo test -p web` — 입력 변환은 순수 함수로 떼어 native 에서): 입력 변환기(두 손가락 탭 → 오른쪽, composition 중복 없음, 특수키).
- 통합: `wrangler dev` 위 허브에 가짜 에이전트·가짜 뷰어(Node WebSocket) — Access 는 시험용 JWKS 주입(`ACCESS_TEST_JWKS` vars, 운영 배포에는 없음 — 설정 검사가 운영에서 이 값이 있으면 거부).
- E2E: C5~C8 실기기.

## 검증 방법
1. `pwsh scripts/verify.ps1` → `VERIFY OK`.
2. `node --test tools/hub.test.mjs` → `# fail 0`.
3. 배포 후 C2·C3 명령.
4. 실기기 C5~C8, `tools/report.mjs` 판정.

## 롤백 계획
- 허브: `npx wrangler rollback`(직전 판). 첫 배포 실패면 Worker 삭제 전 사용자 확인.
- 에이전트: 콘솔 실행이라 닫으면 끝. 키·설정 파일 `%ProgramData%\WebDesktop\` 삭제로 초기화.

## 안 할 것
- Windows 서비스·SYSTEM 도우미·잠금 화면·UAC(첫 작업 2), macOS(첫 작업 3), HW 인코더, 오디오·파일·클립보드·모니터 선택·화질, 업데이트, 접속 기록 화면, VPN, 설치 파일(이번은 exe 하나 — 설치 프로그램은 첫 작업 2).

## 참조 파일
- 설계: `autopilot/webdesktop/03-prd.md`(상수 표), `05-api-contract.md`, `04-architecture.md`(코드 구조·시퀀스), `06-test-design.md`(SC-003 측정법).
- workers-rs: 템플릿 `templates/hello-world`, DO 휴면 WebSocket 예 `test/src/counter.rs`(github cloudflare/workers-rs, v0.9).
- str0m: README 「Frame level」·「NIC enumeration and TURN」, 예제 `examples/chat.rs`(구현 때 읽음).
