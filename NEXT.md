<!-- NEXT-ACTION -->
L · dev:build · 첫 작업 1 BUILD — T1~T10 통과, T11(iPhone)·T12(24시간) 판정 도구 준비됨, 둘 다 기다림 (2026-11-24)
- 맥에서 열었으면: 첫 작업 3(macOS 에이전트) — `docs/specs/03-macos/START.md` 부터. 아래 「다음 할 일」은 Windows PC 몫.
- 다음 할 일: ① (끝남) GitHub 푸시 — origin sangchane/webDesktop, 커밋마다 push. ② 지연 2차(DL-23 남은 것): 하드웨어 인코더(Media Foundation H.264)·입력 직후 바로 캡처·폰 크기 해상도 — 지금 같은 PC 에서 입력→반영 70~130ms, 가끔 230ms. ③ 사용자 PC 에이전트를 새 판으로 바꾸면 iPhone 으로 지연 다시 재기(바꾸면 T12 24시간이 0부터). ④ T12 `node tools/report.mjs --uptime DEV`(2026-11-25 13:06 뒤, 옛 판 그대로면). ⑤ T11 문장 SC-006, 첫 작업 2(SC-004 잠긴 화면).
- 남은 확인: C6 실제 클릭·타자 손맛(사용자), iPhone·iPad 실기기, 에이전트 폴더 주인 확인(첫 작업 2), ICE 재시작 받기(FR-010), 설치 때 Chrome·SmartScreen 경고 창 수(setup-check.md).
- 시험: `scripts/verify.ps1`, `node --test tools/hub.test.mjs`(23), `node --test tools/agent.test.mjs`(5), `node --test tools/web.test.mjs`(10), `node tools/check-access.mjs`(9). 판정: `node tools/report.mjs --last | --since | --uptime`(로그인된 Aside 브라우저로 읽음). 배포는 묻지 않고(AGENTS.md).
- 허브 https://webdesktop-hub.sangchane87.workers.dev(판 8abe2201 = 커밋 595a1b0, 직전 af9c2564) · 웹앱 https://webdesktop-app.sangchane87.workers.dev(판 3037719c = 커밋 367c524, 직전 d17568e8) · 설치 파일 R2 webdesktop-releases/windows/webdesktop-agent.exe(sha256 29a05d5a… = 커밋 5f56852, 지연 1차 개선 DL-23 — 사용자 PC 에서 도는 것은 아직 옛 판 06afd828) · 에이전트 이 PC 에서 실행 중(PID 21540, 13:04 시작 — 사용자 것, 끄지 않음)
<!-- /NEXT-ACTION -->
