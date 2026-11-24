# WebDesktop — 작업 규약

## 네비게이션
- 목표 한 장: `docs/goals.md` — 새 요청은 먼저 목표 번호와 대조한다.
- 설계: `autopilot/webdesktop/` (03 요구사항·상수 표, 05 계약, 08 착수 조건). 조사 근거: `autopilot/webdesktop/research/`.
- 재개 포인터: `NEXT.md`.

## 규약
- 서버는 사용자의 기존 Cloudflare Workers Paid 계정을 쓴다(사용자 2026-11-23 「search-real-estate에 클라우드 플레어 월 5$ 요금제 활성화 해둔 접속 정보 있으니까 그거 사용해서 하자」). 인증은 wrangler OAuth 로그인. 값은 어디에도 옮겨 적지 않는다.
- 같은 계정에 listing 앱이 있다 → 우리 자원(Worker·DO·R2·Access 앱) 이름은 모두 `webdesktop-` 접두.
- 배포 전 계정 한도: `D:\_personal\search-real-estate\docs\cloudflare-limits.md` 를 대조한다.
- 코드 복사 금지 라이선스: AGPL·GPL(RustDesk·Sunshine 등) — 구조만 참고. `cargo deny` 로 막는다.
- Jump Desktop·Tailscale 실행 파일은 디컴파일하지 않는다(약관 금지 — research/local-inspection.md 관문).
- 제품 코드는 전부 Rust 다 — 허브·웹앱·에이전트(사용자 원문 2026-11-23 「RUST 기반으로 … 웹앱」, 다시 확인 같은 날 「rust로 만들어 달라그랬잖아」). 웹앱은 `crates/web`(Leptos CSR → wasm, trunk). TypeScript·JS 는 시험·점검 도구(`tools/`)에만.
- GitHub: origin = https://github.com/sangchane/webDesktop (공개 저장소, 사용자 2026-11-24 「여기에 푸시해」). 이 저장소만 `sangchane` 계정(로컬 git 설정 — Git Credential Manager), 다른 저장소는 gh 의 `kimsangchan`. 커밋하면 묻지 않고 `git push`. 비밀(.env·.dev.vars)은 올리지 않는다(.gitignore).
- 배포는 묻지 않고 한다(사용자 2026-11-23 「배포는 니가 알아서해 묻지말고」). 순서: 커밋 → `crates/hub`·`crates/app` 에서 `npx wrangler deploy` → `node tools/check-access.mjs` → `ACCESS OK`. 실패하면 `npx wrangler rollback` 으로 직전 판. 배포한 판 ID 는 NEXT.md 에.
- 상수(상한·기간·임계)는 `03-prd.md` 상수 표가 단일 출처. 코드는 이름으로 옮긴다.
