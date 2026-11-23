<!-- NEXT-ACTION -->
L · dev:build · 첫 작업 1 BUILD — T1~T6 통과, 다음 T7 (2026-11-23)
- 다음 할 일: T7 웹앱(crates/web, Leptos CSR) 목록·기기 추가·원격 화면(WebRTC 수신)·입력 변환기·계측. 허브 쪽 계약: 입장권 POST /api/ws-ticket → `${hub_ws_url}?ticket=` → viewer.hello, 세션 POST /api/sessions(201 새·200 재사용), DL-15. 장부 docs/specs/01-skeleton/tasks.md.
- 허브 시험 `node --test tools/hub.test.mjs`(23 통과). 배포는 묻지 않고(AGENTS.md).
- 허브 https://webdesktop-hub.sangchane87.workers.dev(배포 판 9bcccdd8 = 커밋 46e09f4, 직전 cd85a766) · 웹앱 https://webdesktop-app.sangchane87.workers.dev
<!-- /NEXT-ACTION -->
