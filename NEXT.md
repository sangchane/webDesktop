<!-- NEXT-ACTION -->
L · dev:build · 첫 작업 1 BUILD — T1~T7 통과, 다음 T8 (2026-11-23)
- 다음 할 일: T8 에이전트(crates/agent, Windows 콘솔): 설정·Ed25519 키(DPAPI)·등록(코드 입력)·허브 WS(hello→auth 서명)·재접속. 검증: 로컬 허브에 등록 → 목록 온라인. 웹앱 쪽 약속(DL-16): 뷰어가 offer(영상 recvonly, DataChannel `input`·`control` 을 뷰어가 엶), 후보는 sdpMLineIndex 0, 이미 아는 session_id 의 session.start 는 거름(DL-15 ⑤).
- 시험: `pwsh scripts/verify.ps1`, `node --test tools/hub.test.mjs`(23), `node --test tools/web.test.mjs`(7, 실제 Chromium). 배포는 묻지 않고(AGENTS.md).
- 허브 https://webdesktop-hub.sangchane87.workers.dev(판 9bcccdd8) · 웹앱 https://webdesktop-app.sangchane87.workers.dev(판 4f869157 = 커밋 fc168a9, 직전 01a5a09b)
<!-- /NEXT-ACTION -->
