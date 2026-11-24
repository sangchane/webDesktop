<!-- NEXT-ACTION -->
L · dev:build · 첫 작업 1 BUILD — T1~T8 통과 + SPEC 02 + 카카오 로그인(DL-18), 다음 T9 (2026-11-24)
- 다음 할 일: T9 에이전트 세션(crates/agent/src/helper + platform/windows): DXGI Desktop Duplication(기본 모니터) → openh264 → str0m H.264 트랙, DataChannel 입력 → SendInput(마우스·HID 키·유니코드), D-08 input.applied. resident 의 hub.msg 자리(resident.rs connect 루프)에서 session.start·signal.* 를 도우미로 넘긴다. 웹앱 쪽 약속(DL-16): 뷰어가 offer, 후보 sdpMLineIndex 0, 아는 session_id 의 session.start 거름(DL-15 ⑤). DL-17: 채널이 닫히면 눌린 키·버튼 모두 놓기, 호스트 해상도를 폰에 맞추기 검토.
- 시험: `scripts/verify.ps1`(agent 포함), `node --test tools/hub.test.mjs`(23), `node --test tools/agent.test.mjs`(4, 실제 에이전트), `node --test tools/web.test.mjs`(10, 실제 Chromium). 배포는 묻지 않고(AGENTS.md).
- 남은 확인: iPhone·iPad 실기기로 가상 키보드·집기·직접 터치 손맛. 에이전트 폴더 주인 확인은 첫 작업 2(설치 프로그램).
- 허브 https://webdesktop-hub.sangchane87.workers.dev(판 af9c2564 = 커밋 1129950, 직전 9bcccdd8) · 웹앱 https://webdesktop-app.sangchane87.workers.dev(판 5180d160 = 커밋 e71be0d, 직전 4f869157)
<!-- /NEXT-ACTION -->
