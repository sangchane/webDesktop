<!-- NEXT-ACTION -->
L · dev:build · 첫 작업 1 BUILD — T1~T9 통과 + SPEC 02 + 카카오 로그인(DL-18), 다음 T10 (2026-11-24)
- 다음 할 일: T10 배포 + PC 브라우저 C5·C6·C7. ① 에이전트 release 빌드(`cargo build --release -p agent` → target/release/webdesktop-agent.exe, 허브 주소 기본값 = 운영 허브) ② 설치 링크(E-12)가 줄 파일을 R2 `webdesktop-` 버킷에 올리는 길 — 지금 허브는 토큰 확인 뒤 not_found(T4) ③ 사용자 PC Chrome 으로 운영 웹앱 → 기기 추가 → 에이전트 실행·코드 → 목록 켜짐(C5 setup-check.md) → 화면·클릭·타자(C6) → `?relay=1` 로 path=relay(C7, `tools/report.mjs` 만들 것).
- 시험: `scripts/verify.ps1`, `node --test tools/hub.test.mjs`(23), `node --test tools/agent.test.mjs`(5, 실제 에이전트·실제 화면), `node --test tools/web.test.mjs`(10). 배포는 묻지 않고(AGENTS.md).
- 남은 확인: iPhone·iPad 실기기 손맛, 에이전트 폴더 주인 확인(첫 작업 2), ICE 재시작 받기(FR-010 웹앱과 같이).
- 허브 https://webdesktop-hub.sangchane87.workers.dev(판 af9c2564 = 커밋 1129950, 직전 9bcccdd8) · 웹앱 https://webdesktop-app.sangchane87.workers.dev(판 5180d160 = 커밋 e71be0d, 직전 4f869157)
<!-- /NEXT-ACTION -->
