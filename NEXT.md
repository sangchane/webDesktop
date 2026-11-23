<!-- NEXT-ACTION -->
L · dev:build · 첫 작업 1 BUILD — T1·T2·T3·T4·T6 통과, 다음 T5 (2026-11-23)
- 다음 할 일: T5 허브 세션(E-07 이어받기)·E-13 입장권 + /ws/viewer(E-06)·신호 중계(M-02~M-04)·TURN 자격·viewer_stats(M-10)·device.status 밀어주기(M-01). 장부 docs/specs/01-skeleton/tasks.md.
- 허브 코드: crates/hub/src/{entry,access,crypto,service,store}.rs, 마이그레이션 crates/hub/migrations/0001_init.sql. 통합 시험 `node --test tools/hub.test.mjs`(로컬 wrangler dev 두 개, 15 통과) — T5 묶음을 같은 파일에 더한다.
- 허브 https://webdesktop-hub.sangchane87.workers.dev(배포 판 cd85a766) · 웹앱 https://webdesktop-app.sangchane87.workers.dev
<!-- /NEXT-ACTION -->
