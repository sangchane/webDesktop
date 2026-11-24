<!-- NEXT-ACTION -->
L · dev:build · 첫 작업 1 BUILD — T10 진행 중: 배포·C5 통과·C7 연결 통과, 화면 잠김으로 영상 확인 남음 (2026-11-24)
- 다음 할 일: 화면을 켠 상태(잠금 해제)에서 ① `%USERPROFILE%\Downloads\webdesktop-agent.exe` 실행(저장 키로 바로 붙음) ② 웹앱에서 DEV 열기 → 화면·클릭·타자(C6, 사용자) ③ `?relay=1` 로 영상까지(C7) → `node tools/report.mjs --last` → `path=relay` 이면 T10 「예」. 그다음 T11(iPhone LTE).
- 알게 된 것: 잠긴 화면이면 뷰어가 「여는 중…」에서 이유 없이 멈추고, 에이전트는 화면 복제 실패를 로그에 남기지 않음(crates/agent/src/platform/capture.rs:97) — 첫 작업 2(SC-004) 때 「화면 잠김」 안내와 같이.
- 시험: `scripts/verify.ps1`, `node --test tools/hub.test.mjs`(23), `node --test tools/agent.test.mjs`(5), `node --test tools/web.test.mjs`(10), `node tools/check-access.mjs`(9). 판정: `node tools/report.mjs --last`(로그인된 Aside 브라우저로 접속 기록을 읽음). 배포는 묻지 않고(AGENTS.md).
- 남은 확인: iPhone·iPad 실기기 손맛, 에이전트 폴더 주인 확인(첫 작업 2), ICE 재시작 받기(FR-010), 설치 때 Chrome·SmartScreen 경고 창 수(setup-check.md).
- 허브 https://webdesktop-hub.sangchane87.workers.dev(판 8abe2201 = 커밋 595a1b0, 직전 af9c2564) · 웹앱 https://webdesktop-app.sangchane87.workers.dev(판 59f5407e = 커밋 595a1b0, 직전 5180d160) · 설치 파일 R2 webdesktop-releases/windows/webdesktop-agent.exe(sha256 06afd828…)
<!-- /NEXT-ACTION -->
