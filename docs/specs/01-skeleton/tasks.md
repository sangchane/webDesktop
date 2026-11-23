# tasks — 첫 작업 1 (워킹 스켈레톤)
한 번에 한 줄. 검증 출력을 본 뒤에만 「통과: 예」.

| # | 할 일 | 검증 | 통과 |
|---|---|---|---|
| T1 | 작업 공간(Cargo workspace·web·scripts/verify.ps1) + proto 최소(오류 이름·healthz) | `pwsh scripts/verify.ps1` → `VERIFY OK` | 예 (2026-11-23 VERIFY OK) |
| T2 | 허브 뼈대(Worker+DO Hub, /healthz, 정적 웹앱 자리) 배포 → 사용자에게 Access·TURN 안내 | `curl -s https://webdesktop-hub.<계정>.workers.dev/healthz` → `"ok":true` | 예 (healthz 200 {"ok":true,"proto":1}) |
| T3 | proto 전체(이번 범위 메시지) + 웹앱 Rust 크레이트 자리(DL-13) | verify + `cargo test -p proto` | 예 (2026-11-23 VERIFY OK·proto 시험 27 통과, `trunk build --release` 성공(wasm 56KB), 헤드리스 화면 「WebDesktop — 준비 중 (proto 1)」 PASS. 처음 판은 ts-rs 로 TS 생성 — 사용자 원문 「Rust 웹앱」과 어긋나 다시 함, DL-13) |
| T4 | 허브 등록·설치 링크·기기 서명 WS·목록·오프라인 판정 + 마이그레이션 | `node --test tools/hub.test.mjs` 등록·서명 묶음 `# fail 0` | 아니오 |
| T5 | 허브 세션·신호 중계·TURN 자격·이어받기·viewer_stats | `node --test tools/hub.test.mjs` 세션 묶음 `# fail 0` | 아니오 |
| T6 | Access 확인(설정은 Aside 로 제가 함) | `node tools/check-access.mjs` → `ACCESS OK` | 예 (2026-11-23 ACCESS OK 4/4 — /agent/* 공개 확인은 T4 뒤 추가) |
| T7 | 웹앱 목록·기기 추가·원격 화면·입력 변환기·계측 | `cargo test -p web` + `trunk build` + 화면 확인 스크립트 | 아니오 |
| T8 | 에이전트: 키·등록·WS·재접속 | 로컬 허브에 등록 → 목록 온라인 | 아니오 |
| T9 | 에이전트 세션: DXGI → openh264 → str0m, 입력 주입, input.applied | PC Chrome 으로 화면·클릭·타자(C6) | 아니오 |
| T10 | 배포 + PC 브라우저 C5·C6·C7 | `node tools/report.mjs --last` | 아니오 |
| T11 | iPhone LTE 실기기 C8 (사용자 협조) | `node tools/report.mjs --since …` 판정 | 아니오 |
| T12 | 24시간 연결 C9 | `node tools/report.mjs --uptime …` | 아니오 |
