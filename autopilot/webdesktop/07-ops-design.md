# 배포·운영 설계 — WebDesktop
버전: v1.2 · 기준 03 v1.2 · 개정: GATE 1차(백업=PITR, 시험 허브 없앰, 알림 줄임, 업데이트=R2, 상수 이름화)

근거 (진입 사전조사)
- 정량: 이 Cloudflare 계정은 2026-11-16부터 Workers Paid(search-real-estate AGENTS.md:54, 한도 요약 `D:\_personal\search-real-estate\docs\cloudflare-limits.md`). TURN 월 TURN_FREE_GB 포함. DO SQLite 시점 복구 PITR_DAYS.
- 정성: 1인 운영 — 알림을 받아 줄 당번이 없다. 문제는 웹앱 목록·설정 화면과 월 1회 점검으로 본다.
- 사용자 영향: 운영 손이 거의 안 가야 한다(G1). 서버 관리 없음, 에이전트는 스스로 업데이트.

## 배포
- 런타임·형상:
  - 허브: Cloudflare Worker `webdesktop-hub` + Durable Object 클래스 `Hub`(인스턴스 `owner`) + 정적 자산(웹앱) + R2 버킷 `webdesktop-releases`(설치·업데이트 파일, `latest-{os}.json`). `wrangler.jsonc` 는 `crates/hub/`. 같은 계정의 listing 앱과 섞이지 않게 자원 이름은 모두 `webdesktop-` 접두(DL-6).
  - Access: 앱 1개(`/` 전체, 정책 = 소유자 이메일) + 우회 앱 2개(`/agent/*`, `/healthz`) — 03 가정 H2.
  - 에이전트: Windows = 설치 파일(서비스 등록 + 도우미), macOS = `.app` 번들 + LaunchDaemon/LaunchAgent plist. 허브 주소는 빌드 때 상수로 넣음. 서명: Windows 없음(SmartScreen 1회), macOS 자체 서명 인증서(03 가정 B12).
- 검증·배포 명령(`just`):
  1. `just verify` — `cargo fmt --check` · `cargo clippy -D warnings` · `cargo test` · `cargo deny check`(AGPL/GPL 차단) · 웹 `tsc --noEmit` + 단위 시험 · `proto` → TS 타입 재생성 후 차이 없음 · 허브 통합 시험(로컬 `wrangler dev`).
  2. `just deploy-hub` — 작업 폴더가 깨끗해야 실행 → `worker-build` → `wrangler deploy` → `/healthz` 의 proto 판 확인 → 배포 커밋 해시를 `deploy.log` 에 기록. 실패하면 `wrangler rollback`.
  3. `just release-agent <os>` — 빌드 → UPDATE_SIGN_ALG 서명 → `wrangler r2 object put` 으로 파일과 `latest-{os}.json` 업로드. 허브는 에이전트가 붙을 때 이 json 과 판을 비교해 M-07 을 보낸다(관리 API 없음).
- 설정·비밀:
  - Worker 비밀(`wrangler secret put`): `TURN_KEY_ID`, `TURN_KEY_TOKEN`, `ACCESS_AUD`, `ACCESS_TEAM_DOMAIN`. 공개값: `UPDATE_PUBKEY`(wrangler vars).
  - 업데이트 서명 개인키: 개발 PC 에만(`%USERPROFILE%\.webdesktop\update-sign.key`), 저장소 밖.
  - 허브는 요청마다 설정 묶음을 한 번 검사(첫 요청 때 캐시)하고, 빠지면 500 `internal` + error 로그 1줄(Worker 는 「기동 거부」가 없음).
  - 에이전트 설정 파일(device_id 등) 검사 실패 시 기동하지 않고 error 로그.
  - Cloudflare 계정 2단계 인증 켜기 — 설치 안내 첫 줄(04 위협 ④-1).
- 종료: 에이전트는 종료 신호에 열린 세션에 `session.end{host_shutdown}` 을 보내고 닫는다. 허브 상태는 DO SQLite 에 변경 즉시 저장.
- 의존 패키지: `Cargo.lock`·`package-lock.json` 커밋, `cargo deny`·`npm audit` 는 `just verify` 안에서.

## 관측성
- SLI: 세션 성공률(첫 프레임까지 간 세션 / 시작한 세션), 첫 프레임 시간(M-10), 재연결 시간(M-10), 경로 비율(M-06).
- SLO: 주간 세션 성공률 ≥ SLO_SUCCESS_PCT, 첫 프레임 p90 ≤ CONNECT_MAX_SEC.
- 4 골든 시그널: 지연 = M-10 / 트래픽 = 세션 수·TURN GB / 오류 = 허브 `internal_error` 수·`session.failed` / 포화 = Worker CPU 초과 오류. 살아 있음 `/healthz`. 빈도 제한 RATE_LIMIT_PER_MIN.
- 로깅:
  - 사건 이름 고정 목록: `enroll.ok` `enroll.rejected` `agent.connected` `agent.disconnected` `agent.auth_failed` `session.started` `session.ended` `session.failed` `session.taken_over` `turn.issued` `device.revoked` `update.offered` `update.applied` `update.rejected` `update.rolled_back` `rate_limited` `internal_error`.
  - 어디에: 허브 = Workers Logs(대시보드·`wrangler tail`). 에이전트 = Windows `%ProgramData%\WebDesktop\logs\`(ACL: SYSTEM·Administrators), macOS `/Library/Logs/WebDesktop/`.
  - 얼마나: 에이전트 로그 하루 1파일, AGENT_LOG_FILES 개 회전. 업무 기록은 DO `session` 표 ACCESS_LOG_DAYS.
  - 금지: 키·TURN 자격·코드 원문·토큰·이메일·화면/입력/클립보드 내용.
- 집계: `tools/report.py` 가 E-09 를 읽어 SLI·SC-002·003·005 판정을 한 화면으로 낸다(웹앱 설정 화면도 같은 값을 보여 줌).

## 알림
1인이라 호출형 알림은 두지 않는다. 웹앱이 보여 준다.

| 조건 | 심각도 | 수신자 | 연결 런북 |
|---|---|---|---|
| 기기가 OFFLINE_WARN_HOURS 넘게 꺼짐 | 낮음 | 웹앱 목록 「오래 꺼짐」(FR-003) | RB-1 |
| 주간 세션 성공률 < SLO_SUCCESS_PCT | 중간 | 웹앱 설정 화면 배너 | RB-2 |
| TURN 월 사용량 ≥ TURN_FREE_GB 의 TURN_WARN_PCT | 중간 | 월 1회 대시보드 점검(사용량 API 가 있으면 배너로 — 검증 필요, 확인 방법: Cloudflare Realtime 사용량 API 문서 확인) | RB-3 |

## 장애·복구
| 장애 | 감지 | 영향 | 복구 절차 | RTO/RPO |
|---|---|---|---|---|
| RB-1 에이전트가 허브에 안 붙음 | 목록 「꺼짐」 | 그 기기 접속 불가 | 호스트에서 `sc query WebDesktop`(Win) / `sudo launchctl print system/com.webdesktop.resident`(Mac) → 로그의 마지막 `agent.auth_failed`·오류 확인 → 서비스 재시작 → 키 손상이면 기기 제거 후 재등록 | 현장 필요 시 귀가 때 / 0 |
| RB-2 세션 실패 증가 | 설정 화면 배너 | 접속 불안정 | `tools/report.py` 로 실패 세션의 경로·종료 사유 확인 → 중계 실패면 TURN 자격·키 확인, 첫 프레임 실패면 해당 호스트 에이전트 로그 | 원인별 / 0 |
| RB-3 TURN 사용량 많음 | 월 점검 | TURN_PRICE_USD_PER_GB 과금 시작 | 중계 비율 확인 → 직접 연결이 안 되는 호스트 망 점검(UPnP) → 절약 화질 기본값 | — |
| 새 허브 배포 깨짐 | 배포 직후 `/healthz` 실패·세션 실패 | 전체 접속 불가 | `wrangler rollback` → 원인 수정 후 재배포 | 5분 / 0 |
| 새 에이전트 판 연결 실패 | 에이전트 스스로 | 그 기기 일시 불가 | 이전 판 자동 복귀(FR-017), `update.rolled_back` | 5분 / 0 |
| DO 데이터 손상·실수 삭제 | 목록이 비거나 오류 | 기기 목록·기록 잃음 | DO 시점 복구(PITR_DAYS 안 시점으로) → 안 되면 기기 재등록 | 30분 / 복구 시점까지 |
| RB-5 뷰어 기기(폰) 분실 | 사용자 | 남이 웹앱 세션으로 접속 가능 | Cloudflare Zero Trust 대시보드 → Users → 세션 폐기 → 필요하면 Access 세션 기간 줄임 | 5분 / — |
| Cloudflare 장애 | 웹앱 안 열림 | 전체 불가 | 대체 경로 없음(R15). 상태 페이지 확인 후 대기 | Cloudflare 복구까지 / 0 |

- 백업: DO SQLite 시점 복구(PITR_DAYS) — 별도 덤프 없음(1인, 잃어도 기기 재등록으로 복구). 복원 연습: 설치 직후 1회 — 기기 1개 등록 → 시각 기록 → 기기 제거 → 그 시각으로 복구 → 기기가 돌아오는지 확인(검증 필요 — 확인 방법: 이 연습 자체가 PITR 의 Rust API 사용 가능 여부 확인).
- 런북 골격(RB 공통): 알림 → 영향 → 진단 명령 → 해결 → 확인(목록 온라인·세션 1회) → 되돌리기.

## 착수 자산
디렉터리 구조
```
WebDesktop/
├─ crates/
│  ├─ proto/      공유 메시지·오류 이름·꼴 검사, TS 타입 생성
│  ├─ hub/        Cloudflare Worker + DO (entry/service/store, migrations/, wrangler.jsonc)
│  └─ agent/      호스트 에이전트 (resident/helper/platform)
├─ web/           웹앱 PWA (TypeScript)
├─ tools/         report.py(계측 판정), 설치 시험 체크리스트
├─ docs/          goals.md, 설치 안내
└─ autopilot/     설계 문서
```

`.env.example` (값 없음)
```
# 개발 PC 에서만 쓰는 값 (Worker 비밀은 wrangler secret put 으로)
WEBDESKTOP_HUB_URL=        # 에이전트 빌드 때 넣을 허브 주소 (예: https://webdesktop-hub.<계정>.workers.dev)
WEBDESKTOP_UPDATE_KEY=     # 업데이트 서명 개인키 파일 경로
```

첫 작업 3개 = 워킹 스켈레톤 (Impact × Uncertainty 큰 것부터)
1. **허브 + 웹 + Windows 에이전트 최소 화면** — `proto`·허브(등록 코드·기기 서명 WS·세션 신호 중계·TURN 자격)·웹(목록·원격 화면·M-10 계측)·Windows 도우미(DXGI 캡처 → openh264 → str0m 영상 트랙, 마우스·`text` 입력). Access(우회 경로 포함) 설정. **iPhone Safari LTE 실기기로** 열기·클릭·한글 입력·`?relay=1` 까지(03 가정 P1·H1·H2, SC-003 측정법).
2. **Windows 무인 접속 spike** — SYSTEM 서비스 → 콘솔 세션 도우미, 잠금 화면·UAC 데스크톱 전환, `sas`(03 가정 P2·W1, SC-004).
3. **macOS 에이전트 최소 화면 + 권한 유지 + 로그인 창** — ScreenCaptureKit → VideoToolbox → str0m, 권한 안내, 자체 서명 두 판 업데이트 후 권한 유지, LoginWindow spike(03 가정 B12·M1).
