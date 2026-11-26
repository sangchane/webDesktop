<!-- NEXT-ACTION -->
M · dev:build · SPEC 04 Claude 권한 요청(FR-023) SHIP 끝 — 허브·웹앱·에이전트 모두 배포 (2026-11-26)
- 다음 할 일: ① 2026-11-27 09:21(KST) 뒤 T12 `node tools/report.mjs --uptime DEV` 판정(새 에이전트 연결 2026-11-26T00:20:55Z 부터). ② 사용자 폰으로 실제 권한 요청 1회(Claude Code 대화형 → 폰 카드 허용·거부). ③ 사용자 순서(2026-11-25): 다음 후보 — 웹 푸시(웹앱 닫힌 폰에도 권한 요청 알림), 클립보드(FR-011). 폰 마이크·받아쓰기는 맨 마지막.
- 확인함(2026-11-26): 배포 허브 + 새 에이전트에 훅 꼴 요청 → `ask.new` → 허브 `ask.no_viewer` 0.12초 → 빈 답(터미널로 되돌아감). 훅은 `~/.claude/settings.json` PermissionRequest 에 들어감(보관 `settings.json.webdesktop.bak`).
- 주의: 에이전트 실행 파일은 모르는 인자(`--help` 도)를 받아도 상주 에이전트로 켜진다 — 같은 PC 에서 켜면 돌던 에이전트가 「Replaced」로 끝난다(2026-11-26 실제로 일어남).
- 시험: `scripts/verify.ps1`, `node --test tools/hub.test.mjs`(23), `node --test tools/agent.test.mjs`(6), `node --test tools/web.test.mjs`(12), `node tools/check-access.mjs`. 허브 배포는 `PATH` 에 `~/.cargo/bin` 이 있어야 함. 실제 claude 확인 정답지 2/2(decision-log REVIEW 04).
- 허브 https://webdesktop-hub.sangchane87.workers.dev(판 ccf25de0 = 커밋 6780960, 직전 8abe2201) · 웹앱 https://webdesktop-app.sangchane87.workers.dev(판 398bf5cd = 커밋 6780960, 직전 3037719c) · 설치 파일 R2 webdesktop-releases/windows/webdesktop-agent.exe(sha256 0bdf2578… = 커밋 6780960) · 에이전트 이 PC 에서 실행 중(`C:\Users\dev\Downloads\webdesktop-agent.exe` 새 판, PID 47704 — 사용자 것, 끄지 않음. 옛 판은 같은 폴더 `webdesktop-agent.old.exe`)
- 남은 확인(옛): C6 손맛, iPhone·iPad 실기기, 첫 작업 2(SC-004 잠긴 화면), T11 SC-006, ICE 재시작(FR-010), macOS(첫 작업 3 — docs/specs/03-macos/START.md).
<!-- /NEXT-ACTION -->
