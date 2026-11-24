<!-- NEXT-ACTION -->
L · dev:build · 첫 작업 1 BUILD — T1~T10 통과, T11(iPhone)·T12(24시간) 판정 도구 준비됨, 둘 다 기다림 (2026-11-24)
- 다음 할 일: ① 2026-11-25 13:06(KST) 뒤 `node tools/report.mjs --uptime DEV` → `UPTIME OK` 면 T12 「예」(에이전트 창이 닫히면 0부터 — 그때는 다시 실행하고 24시간). ② T11 은 사용자가 iPhone LTE 로: 시작 시각 기록 → 기기 10번 열고 닫기 → 한 번은 1분 넘게 클릭·타자 → 메모장에 「안녕하세요 원격 테스트」 → 전체 선택·복사 → `node tools/report.mjs --since <시작 시각>` → `정답지 3/3`. ③ 기다리는 동안 첫 작업 2(SC-004 잠긴 화면 안내 — capture.rs:97 로그 없음, 뷰어 「여는 중…」 멈춤)로.
- 남은 확인: C6 실제 클릭·타자 손맛(사용자), iPhone·iPad 실기기, 에이전트 폴더 주인 확인(첫 작업 2), ICE 재시작 받기(FR-010), 설치 때 Chrome·SmartScreen 경고 창 수(setup-check.md).
- 시험: `scripts/verify.ps1`, `node --test tools/hub.test.mjs`(23), `node --test tools/agent.test.mjs`(5), `node --test tools/web.test.mjs`(10), `node tools/check-access.mjs`(9). 판정: `node tools/report.mjs --last | --since | --uptime`(로그인된 Aside 브라우저로 읽음). 배포는 묻지 않고(AGENTS.md).
- 허브 https://webdesktop-hub.sangchane87.workers.dev(판 8abe2201 = 커밋 595a1b0, 직전 af9c2564) · 웹앱 https://webdesktop-app.sangchane87.workers.dev(판 59f5407e = 커밋 595a1b0, 직전 5180d160) · 설치 파일 R2 webdesktop-releases/windows/webdesktop-agent.exe(sha256 06afd828…) · 에이전트 이 PC 에서 실행 중(PID 21540, 13:04 시작 — 사용자 것, 끄지 않음)
<!-- /NEXT-ACTION -->
