<!-- NEXT-ACTION -->
L · dev:build · 첫 작업 1 BUILD — T1~T10 통과, T11(iPhone)·T12(24시간) 판정 도구 준비됨, 둘 다 기다림 (2026-11-24)
- 맥에서 열었으면: 첫 작업 3(macOS 에이전트) — `docs/specs/03-macos/START.md` 부터. 아래 「다음 할 일」은 Windows PC 몫.
- 다음 할 일: ① (끝남) GitHub 푸시 — origin sangchane/webDesktop, 커밋마다 push. ② 지연(DL-23 1~4차 끝): 같은 PC 입력→반영 보통 45ms(34~88, 가끔 186). GPU 길(캡처 사본→영상 처리기 NV12→NVENC) 동작, 실패 시 CPU 길로 넘어감 확인. 다음 후보 — 폰 크기 해상도, 세션 기록 encoder 칸 채우기. ③ 사용자 결정(2026-11-24 「내일 바꾸고」): 2026-11-25 13:06 뒤 T12 `--uptime DEV` 판정 → 그다음 사용자 PC 에이전트를 R2 새 판으로 바꾸고 iPhone LTE 로 지연 다시 재기. ④ T12 `node tools/report.mjs --uptime DEV`(2026-11-25 13:06 뒤, 옛 판 그대로면). ⑤ T11 문장 SC-006, 첫 작업 2(SC-004 잠긴 화면).
- 남은 확인: C6 실제 클릭·타자 손맛(사용자), iPhone·iPad 실기기, 에이전트 폴더 주인 확인(첫 작업 2), ICE 재시작 받기(FR-010), 설치 때 Chrome·SmartScreen 경고 창 수(setup-check.md).
- 시험: `scripts/verify.ps1`, `node --test tools/hub.test.mjs`(23), `node --test tools/agent.test.mjs`(5), `node --test tools/web.test.mjs`(10), `node tools/check-access.mjs`(9). 판정: `node tools/report.mjs --last | --since | --uptime`(로그인된 Aside 브라우저로 읽음). 배포는 묻지 않고(AGENTS.md).
- 허브 https://webdesktop-hub.sangchane87.workers.dev(판 8abe2201 = 커밋 595a1b0, 직전 af9c2564) · 웹앱 https://webdesktop-app.sangchane87.workers.dev(판 3037719c = 커밋 367c524, 직전 d17568e8) · 설치 파일 R2 webdesktop-releases/windows/webdesktop-agent.exe(sha256 da1d0001… = 커밋 374687c, GPU 길 DL-23 4차 — 사용자 PC 에서 도는 것은 아직 옛 판 06afd828) · 에이전트 이 PC 에서 실행 중(PID 21540, 13:04 시작 — 사용자 것, 끄지 않음)
<!-- /NEXT-ACTION -->
