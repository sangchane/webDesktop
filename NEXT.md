<!-- NEXT-ACTION -->
L · dev:build · 첫 작업 1 BUILD — T1~T7 통과 + SPEC 02(폰·태블릿 조작감·가상 키보드) + 카카오 로그인(DL-18) 통과, 다음 T8 (2026-11-23)
- 다음 할 일: T8 에이전트(crates/agent, Windows 콘솔): 설정·Ed25519 키(DPAPI)·등록(코드 입력)·허브 WS(hello→auth 서명)·재접속. 웹앱 쪽 약속(DL-16): 뷰어가 offer, 후보 sdpMLineIndex 0, 아는 session_id 의 session.start 거름(DL-15 ⑤). T9 에 더할 것(DL-17): 채널이 닫히면 눌린 키·버튼을 모두 놓기, 호스트 해상도를 폰에 맞추기 검토.
- 시험: `scripts/verify.ps1`, `node --test tools/hub.test.mjs`(23), `node --test tools/web.test.mjs`(10, 실제 Chromium). 배포는 묻지 않고(AGENTS.md).
- 남은 확인: iPhone·iPad 실기기로 가상 키보드·집기·직접 터치 손맛(TAP·LONG_PRESS 값 조정).
- 허브 https://webdesktop-hub.sangchane87.workers.dev(판 af9c2564 = 커밋 1129950, 직전 9bcccdd8) · 웹앱 https://webdesktop-app.sangchane87.workers.dev(판 5180d160 = 커밋 e71be0d, 직전 4f869157)
<!-- /NEXT-ACTION -->
