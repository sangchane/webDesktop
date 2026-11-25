<!-- NEXT-ACTION -->
M · dev:build · SPEC 04 Claude 권한 요청(FR-023) SHIP 중 — 커밋 6780960, 웹앱만 배포함 (2026-11-25)
- 다음 할 일: ① 13:06 뒤 T12 `node tools/report.mjs --uptime DEV` 판정(허브 배포 전에 — 배포하면 연결 기록이 다시 시작됨). ② 허브 배포(`crates/hub` `npx wrangler deploy` → `node tools/check-access.mjs`). ③ 에이전트 배포판 빌드 → R2 `webdesktop-releases/windows/webdesktop-agent.exe` 올림 → 사용자 PC 에이전트 교체(NEXT 옛 ③) → `webdesktop-agent --claude-hook` 으로 훅 설치 → 실제 Claude Code 대화형 1회(터미널 되돌아감 확인).
- 사용자 순서(2026-11-25): 폰 마이크·받아쓰기는 맨 마지막. 다음 후보 — 웹 푸시(웹앱 닫힌 폰에도 권한 요청 알림), 클립보드(FR-011).
- 시험: `scripts/verify.ps1`, `node --test tools/hub.test.mjs`(23), `node --test tools/agent.test.mjs`(6), `node --test tools/web.test.mjs`(12), `node tools/check-access.mjs`. 실제 claude 확인 스크립트는 스크래치(정답지 2/2, decision-log REVIEW 04).
- 허브 https://webdesktop-hub.sangchane87.workers.dev(판 8abe2201 = 커밋 595a1b0 — 아직 권한 요청 없음) · 웹앱 https://webdesktop-app.sangchane87.workers.dev(판 398bf5cd = 커밋 6780960, 직전 3037719c) · 설치 파일 R2(sha256 da1d0001… = 커밋 374687c) · 에이전트 이 PC 에서 실행 중(PID 21540 — 사용자 것, 끄지 않음)
- 남은 확인(옛): C6 손맛, iPhone·iPad 실기기, 첫 작업 2(SC-004 잠긴 화면), T11 SC-006, ICE 재시작(FR-010), macOS(첫 작업 3 — docs/specs/03-macos/START.md).
<!-- /NEXT-ACTION -->
