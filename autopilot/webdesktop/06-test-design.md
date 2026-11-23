# 테스트 설계 — WebDesktop
버전: v1.2 · 기준 03 v1.2 · 개정: GATE 1차 + 재검토(설치 링크·계측 저장·오프라인 판정·1단계 에이전트 TURN 없음)(뷰어 계측·지연 측정법·SC-008 판정식·중계 강제 방법·코드 시도 수·VPN TURN)

근거 (진입 사전조사)
- 정량: 위험 상위 = iPhone 웹 조작감(P1)·Windows 보안 데스크톱(P2)·macOS 권한 유지(B12)·에이전트 TURN(T1) — 02 사전 부검, 03 가정.
- 정성: "Fluid 연결이 1~2분 걸림"(Jump 커뮤니티, research/remote-desktop-oss.md) → 접속 시간은 뷰어가 스스로 잰다.
- 사용자 영향: 실기기에서만 드러나는 실패가 많다 → 수동 시험도 계측 값·장면 기록과 판정 스크립트로 남긴다.

## 시험 환경
- 단위·통합: `cargo test`, 허브는 로컬 `wrangler dev`(로컬 DO·SQLite)에서 가짜 에이전트·가짜 뷰어 WebSocket 으로. 별도 시험 허브는 두지 않는다(1인).
- E2E: 운영 허브 + 실기기(iPhone, 집 Windows PC, MacBook). 판정은 `tools/` 집계 스크립트가 E-09 접속 기록(M-06·M-10)을 읽어 통과/실패를 낸다.
- 중계 강제: 뷰어 `RTCPeerConnection({iceTransportPolicy:"relay"})` 시험 스위치(웹앱 `?relay=1`).

## SC-003 측정법 (뷰어 시계 하나)
1. 뷰어가 입력을 보낼 때 `seq` 와 `performance.now()` 를 기록(D-01~03).
2. 도우미가 그 입력을 주입한 뒤 처음 인코딩한 프레임의 RTP 타임스탬프를 `input.applied{seq, rtp_ts}`(D-08)로 보낸다.
3. 뷰어는 `video.requestVideoFrameCallback` 의 `metadata.rtpTimestamp` 가 `rtp_ts` 와 같은 프레임이 그려진 `expectedDisplayTime`(또는 콜백 시각)을 잡는다.
4. 지연 = 3의 시각 − 1의 시각. 같은 기기 시계라 시계 차이 문제가 없다. 1분 동안 모아 p50 을 M-10 `latency_p50_ms` 로 보내고, 허브가 `session.latency_p50_ms` 에 저장한다.
- `requestVideoFrameCallback`·`rtpTimestamp` 의 iPhone Safari 지원은 검증 필요 — 확인 방법: 첫 작업 1에서 Safari 콘솔로 메타데이터 출력. 안 되면 대안: 도우미가 프레임 구석에 seq 를 픽셀 표지로 그리고 뷰어가 캔버스로 읽음.

## 수용 기준 → 시나리오 변환표
| SC/FR ID | 시나리오 (Given/When/Then) | 레이어 | 데이터/목킹 |
|---|---|---|---|
| SC-001, FR-002 | Given 깨끗한 Windows VM, When 웹앱 「기기 추가」 → 설치 파일 받아 실행 → 코드 입력, Then 목록에 「온라인」, 사용자 행동 ≤ SETUP_MAX_ACTIONS, OS 확인 창 ≤ WIN_OS_PROMPTS, 새 PC 에서 허브 주소·로그인 입력 0회(설치 링크 E-12 로 받음) | E2E(체크리스트) | 운영 허브 |
| SC-001 (Mac) | Given 새 macOS 계정, When 같은 흐름 + OS 확인 창, Then 행동 ≤ SETUP_MAX_ACTIONS, OS 확인 창 ≤ MAC_OS_PROMPTS, 권한 ok | E2E(체크리스트) | 실기기 MacBook |
| FR-002 만료 | Given 만료된 활성 코드, When `/agent/enroll`, Then 400 `code_expired`, 코드 행 삭제 | 통합(허브) | 가짜 시계 |
| FR-002 대입 | Given 활성 코드, When 틀린 코드 REG_CODE_MAX_TRIES 회, Then 활성 코드 폐기 → 맞는 코드도 `code_invalid`, warn 로그 `enroll.rejected` | 통합(허브) | — |
| FR-002 재사용 | Given 사용된 코드, When 다시 등록, Then `code_invalid` | 통합(허브) | — |
| FR-002 새 코드 | Given 활성 코드 A, When 새 코드 B 생성, Then A 로 등록 시 `code_invalid`, A 의 설치 링크도 `code_invalid` | 통합(허브) | — |
| FR-002 설치 링크 | Given 설치 링크, When Access 로그인 없이 `/agent/install/windows?t=` , Then 파일. INSTALL_LINK_TTL_MIN 지나면 `code_expired` | 통합(허브) + E2E(curl) | 가짜 시계 |
| FR-001 | Given Access JWT 없음, When `/api/devices`, Then 401 `unauthorized`. Given 서명·aud 틀린 JWT, Then 401 | 통합(허브) | 시험용 JWKS |
| FR-001 우회 경로 | Given workers.dev 에 Access, When `/agent/enroll`·`/healthz` 를 로그인 없이 호출, Then Access 로그인 화면이 아니라 허브 응답 | E2E(curl) | 운영 허브(03 H2) |
| FR-003, FR-009 | Given 에이전트 연결, When `agent.report{perm:{screen:false}}`, Then 뷰어 WS 에 `device.status{connectable:false}`, 목록 「권한 필요」 | 통합(허브) | 가짜 에이전트 WS |
| FR-003 오래 꺼짐 | Given last_seen_at 이 OFFLINE_WARN_HOURS 전, When `/api/devices`, Then `offline_long:true` | 통합(허브) | 가짜 시계 |
| FR-003 조용한 끊김 | Given 에이전트 WS 가 닫힘 이벤트 없이 멈춤(half-open), When OFFLINE_DETECT_SEC 뒤 `/api/devices`, Then `online:false`, 그 WS 닫힘 | 통합(허브) | 가짜 시계 + 응답 없는 가짜 에이전트 |
| SC-002, FR-004 | Given iPhone Safari LTE·집 PC 온라인, When 기기를 10회 누름, Then M-10 `click_to_first_frame_ms` ≤ CONNECT_MAX_SEC 가 9회 이상 | E2E(실기기, 스크립트 판정) | E-09 집계 |
| FR-004 중계 | Given 웹앱 `?relay=1`, When 세션, Then M-10 path=relay 가 접속 기록에 저장, 화면에 「중계」 | E2E | 운영 허브 + Cloudflare TURN |
| FR-004 오프라인 | Given 기기 오프라인, When `POST /api/sessions`, Then 409 `device_offline` | 통합(허브) | — |
| FR-004 이어받기 | Given 뷰어 A 세션 중, When 다른 `viewer_id` 의 B 가 엶, Then A 에 `session.end{taken_over}`, B 연결. 같은 `viewer_id` 재요청은 같은 session_id | 통합(허브) | 가짜 뷰어 2개 |
| FR-018 에이전트 TURN (2단계) | Given 에이전트가 `turn.request{vpn}`, Then `turn.creds` 의 만료 = TURN_CRED_TTL_MIN, 만료 전 재요청에 새 자격 | 통합(허브) | TURN API 가짜 |
| SC-003 | Given iPhone LTE 세션, When 1분간 초당 2회 클릭, Then 위 측정법 p50 ≤ LATENCY_P50_MS | E2E(실기기, 스크립트 판정) | D-08 |
| FR-005 | Given 5K 모니터, When 세션, Then 송출 긴 변 ≤ VIDEO_MAX_EDGE, H.264 레벨 ≤ H264_MAX_LEVEL | 단위(인코더 설정) | 해상도 입력값 |
| FR-005 대역폭 | Given 대역폭 추정이 1Mbps 로 떨어짐, Then 인코더 목표 비트레이트 ≤ 추정값 | 단위 | 가짜 BWE |
| FR-006 | Given 데스크톱 Chrome 전체 화면, When Alt+Tab·Win 키, Then 호스트로 전달(Keyboard Lock) | E2E(수동) | — |
| FR-006 좌표 | Given 비율 좌표 (0.5,0.5), When mouse_move, Then 선택 모니터 중앙 픽셀 | 단위(좌표 변환) | 모니터 배치 표 |
| SC-006, FR-007 | Given iPhone 한글 키보드, When 「안녕하세요 원격 테스트」 입력, Then 호스트 메모장 내용이 같다 | E2E(실기기, 자동 비교) | 호스트 클립보드로 회수 |
| FR-007 제스처 | Given 합성 터치 열(두 손가락 탭), When 변환, Then `mouse_button{right}` 1회 | 단위(웹 입력 변환기) | 합성 터치 이벤트 |
| FR-007 조합 중복 | Given 조합 중 자모 이벤트, When compositionend, Then `text` 1회만 | 단위(웹) | 합성 composition 이벤트 |
| FR-007 특수키 | Given 특수키 막대 Ctrl 고정 + C, Then `key` HID Ctrl down·C down·C up·Ctrl up | 단위(웹) | — |
| SC-004, FR-008 | Given 잠긴 Windows, When iPhone 에서 엶 → `sas` → 비밀번호 → UAC 승인, Then 데스크톱 도달 | E2E(수동 + 장면 기록) | 실기기 |
| FR-008 데스크톱 전환 | Given 세션 중, When UAC 창(Winlogon 데스크톱), Then `host.state{desktop:winlogon}`, 프레임 계속 | 통합(Windows 에이전트) | `consent.exe` 유발 |
| SC-005, FR-010 | Given 세션 중, When 와이파이 끄기(LTE 전환) 5회, Then 접속 기록의 `reconnect_ms[]` 5개 모두 ≤ RECONNECT_MAX_SEC, `new_session_used=false` | E2E(실기기, 스크립트 판정) | E-09 집계 |
| FR-010 2·3단계 | Given ICE 재시작이 응답 없음(가짜 에이전트), When RECONNECT_MAX_SEC 경과, Then 새 세션 1회 시도 → 실패 시 「다시 연결」 버튼, 세션 기록 `failed` | 통합(웹 + 허브) | 가짜 에이전트 |
| SC-007 | Given 30일 운영, Then 에이전트 재등록·재로그인 0회(`enroll.ok` 는 기기 수만큼, `agent.auth_failed` 0) | 운영 관찰(스크립트) | 허브 로그·접속 기록 |
| SC-008 | Given `?relay=1` 세션, When 뷰어 쪽 패킷 캡처, Then UDP 페이로드가 STUN/TURN 헤더 + SRTP(헤더 평문·페이로드 암호) + DTLS 레코드뿐, H.264 NAL 시작 코드로 디코딩 가능한 프레임 없음 | 수동 1회(Wireshark) | — |
| SC-009 | Given 한 달 사용, Then Cloudflare 사용량: Workers Paid 포함량 안, TURN ≤ TURN_FREE_GB | 운영 관찰 | 대시보드 |
| FR-011 | Given 호스트 클립보드 「abc」, When 뷰어 붙여넣기 요청, Then 뷰어에 「abc」, 반대 방향도 | 통합(에이전트) | — |
| FR-012 | Given 모니터 2개, When `monitor.select 1`, Then 둘째 모니터 프레임 | 통합(에이전트) | — |
| FR-013 | Given 절약 모드, Then 인코더 목표 ≤ SAVER_BITRATE_KBPS | 단위 | — |
| FR-014 | Given Mac 재부팅 후 로그인 창, When 세션, Then `host.state{loginwindow}`, 로그인 창이 보이고 입력됨 | E2E(수동, 03 M1 spike 결과에 따름) | 실기기 |
| FR-015 | Given 세션 종료, When `/api/access-log`, Then 그 세션 행(경로·첫 프레임·재연결·종료 사유), ACCESS_LOG_DAYS 지난 행은 정리 후 없음 | 통합(허브) | 가짜 시계 |
| FR-016 | Given 기기 제거, When 그 기기가 WS 재연결, Then `device_revoked` 로 닫힘, 열린 세션 `device_revoked` 종료 | 통합(허브) | — |
| FR-017 서명 | Given 서명 틀린 판, Then 설치 안 함·error 로그 `update.rejected` | 통합(에이전트) | 시험 서명 키 |
| FR-017 토큰 | Given 사용한 토큰(`update_token` 행 삭제됨), When `/agent/update/file` 재요청, Then `code_invalid`. UPDATE_TOKEN_TTL_MIN 지나면 `code_expired`. DO 휴면 뒤에도 같은 결과(표에 저장) | 통합(허브) | 가짜 시계 |
| FR-017 복귀 | Given 새 판이 허브 연결 실패, Then 이전 판 복귀·`update.rolled_back` | 통합(에이전트) | 깨진 판 |
| SC-010, FR-018 | Given 두 호스트 VPN 참가, When A 가 B 의 사설 주소로 ping·TCP, Then 성공, M-12 가 `vpn_link` 에 저장되어 E-02 `vpn_links` 로 목록에 표시 | E2E(2단계) | 실기기 2대 |
| FR-018 중계 | Given 한쪽 호스트 UDP 를 TURN 서버 외 차단, When VPN, Then path=relay 로 성공, 자격 만료 전 `turn.request` 로 갱신되어 1시간 넘게 유지 | E2E(2단계) | 방화벽 규칙 |

## 계약 테스트
- `proto`: 메시지마다 직렬화 왕복 + 생성된 TS 타입과 Rust JSON 견본 일치(공유 계약).
- 허브: 모든 오류 응답이 Problem JSON(05 오류 이름 목록 안의 type), 스택 없음.
- 크기 상한: WS_MSG_MAX_BYTES 초과 → `too_large` 로 닫힘.
- 빈도: RATE_LIMIT_PER_MIN 초과 → 429 `rate_limited`.
- 판 불일치: proto major 다른 hello → `upgrade_required`.
- 유지 신호: `ping` 에 `pong` 자동 응답(DO 깨우지 않음 — 로컬에서는 응답만 확인).

## E2E 후보 (핵심 여정)
1. 새 PC 추가 → iPhone LTE 로 열기 → 한글 입력(SC-001·002·006).
2. 잠긴 Windows 열기 → UAC 승인(SC-004).
3. 와이파이↔LTE 전환 재연결(SC-005).

## 리스크 기반 커버리지 목표
- 높음(정상·경계·실패 전부): 허브 인증(Access JWT·기기 서명·우회 경로), 등록 코드, 기기 제거, Windows 데스크톱 전환, 웹 입력 변환기(한글·특수키), 에이전트 TURN 자격 갱신.
- 중간(정상·실패 1개): 세션 신호 중계·이어받기, 업데이트 서명·토큰, 재연결 3단계.
- 낮음(정상 1개): 클립보드·모니터 선택·화질 단계.
