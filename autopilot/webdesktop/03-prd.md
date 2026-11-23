# PRD — WebDesktop
버전: v1.2

## 배경
- 사용자는 Jump Desktop(원격 화면)과 Tailscale(VPN)을 같이 쓰다가 설정이 번거로워 안 쓰게 됐다(원문 2026-11-23, 이 PC 실측: Tailscale 기기 0대, Jump 호스트 승인 창 5회 놓침 — research/local-inspection.md T4·J7).
- 두 제품은 같은 뼈대(신호/조정 서버 + 직접 연결 + 중계)다 → 서버 1벌로 합친다(01-recon 요약 1).
- 혼자 쓴다. iPhone 은 App Store 앱 없이 웹앱으로 제어한다(02 A-1·A-2). 서버는 기존 Cloudflare 유료 계정(DL-3·DL-6).
- 상세: [01-recon](01-recon.md) · [02-register](02-blindspot-register.md) · [decision-log](decision-log.md)

## 제품 목표 (서로 직교)
- **G1 설정 한 번** — 새 PC·Mac 을 추가해 첫 화면을 보기까지 사용자 행동 SETUP_MAX_ACTIONS 이하. 이후 재인증·키 만료로 다시 설정하지 않는다.
- **G2 어디서나 조작** — iPhone·Android·다른 PC 의 브라우저에서, 밖(LTE)에서도, 잠긴 집 PC 까지 열어 조작한다.
- **G3 기기끼리 사설망** — 호스트(Windows·Mac)끼리 같은 망처럼 직접 통신한다(2단계).

## 유저 스토리
- **US-1 (P1)** 나는 밖에서 iPhone Safari 로 집 Windows PC 화면을 열어 조작하고 싶다 — 앱 설치 없이 급한 일을 처리하려고. 독립 시험: iPhone LTE → 잠긴 Windows PC 열기·클릭·한글 입력.
- **US-2 (P1)** 나는 새 PC·Mac 을 설치 파일 하나와 코드 하나로 추가하고 싶다 — 설정 때문에 포기하지 않으려고. 독립 시험: 깨끗한 PC 에서 설치 → 목록에 「온라인」.
- **US-3 (P2)** 나는 다른 PC 브라우저에서 Mac 을 오래 쓰고 싶다(단축키·클립보드 포함) — 자리를 옮겨도 일을 이어가려고.
- **US-4 (P3)** 나는 집 PC 와 MacBook 이 서로 사설 주소로 통신하게 하고 싶다 — 파일 공유·개발 서버 접속을 위해.
P1 스토리(US-1·US-2)만으로 MVP 가 성립한다.

## 요구사항 풀
| ID | 요구사항 | 우선순위 | 출처 |
|---|---|---|---|
| FR-001 | 웹앱 접속은 Cloudflare Access 이메일 일회용 PIN 로그인, 허용 이메일은 소유자 1개. 로그인 유지 ACCESS_SESSION_DAYS | P0 | US-1, DL-4, STRIDE S |
| FR-002 | 웹앱 「기기 추가」가 설치 파일 받기 단추와 등록 코드(REG_CODE_LEN 자리, REG_CODE_TTL_MIN 분, 1회용)를 보여 준다. 활성 코드는 하나뿐(새로 만들면 이전 코드 폐기). 틀린 코드 입력은 활성 코드의 시도 수로 세고 REG_CODE_MAX_TRIES 회가 되면 활성 코드 폐기. 설치 파일에는 허브 주소가 빌드 때 들어 있다(1인·허브 1개). 에이전트 설치 창에 코드를 넣으면 에이전트가 만든 기기 공개키가 등록된다 | P0 | US-2, B10 |
| FR-003 | 기기 목록: 이름·OS·온라인 여부·마지막 접속 시각·권한 상태(macOS)·연결 가능 여부(= 온라인이고 권한 정상). OFFLINE_WARN_HOURS 넘게 꺼진 기기는 「오래 꺼짐」 | P0 | US-1, R14 |
| FR-004 | 목록에서 기기를 누르면 원격 화면 세션을 연다. 직접 연결 우선, 실패 시 Cloudflare TURN. 화면에 경로(직접/중계) 표시 | P0 | US-1, DL-5 |
| FR-005 | 호스트는 화면을 H.264(레벨 H264_MAX_LEVEL 이하)로 보내고 긴 변이 VIDEO_MAX_EDGE 를 넘으면 줄인다. 망 대역폭 추정에 따라 비트레이트를 바꾼다 | P0 | B13 |
| FR-006 | 데스크톱 브라우저 입력: 마우스(이동·클릭·휠)·키보드. 전체 화면에서 Keyboard Lock 으로 Win/Cmd·Alt+Tab 전달, 안 되는 키는 특수키 막대 | P0 | US-3, B14 |
| FR-007 | 터치 입력(iPhone·Android·iPad): 조작 방식 두 가지 — 트랙패드(한 손가락 이동, 탭=클릭, 탭 후 끌기=끌기, 두 손가락 탭=오른쪽, 두 손가락 끌기=스크롤)·직접 터치(탭=그 자리 클릭, 길게=오른쪽, 길게 누른 뒤 끌기=끌기, 한 손가락 끌기=스크롤). 두 방식 모두 집기=화면 확대(트랙패드는 커서를 따라감). 특수키 막대(Ctrl·Alt·Win·Esc·Tab·Del) + 자체 가상 키보드(F1~F12·방향·Home/End·한글 두벌식 조합) + 시스템 자판으로 완성 글자 입력 | P0 | US-1, R5·R6, research/touch-control.md |
| FR-008 | Windows 무인 접속: 잠금 화면·로그인 화면·UAC 창을 캡처하고 입력할 수 있다(Ctrl+Alt+Del 포함) | P0 | US-1, R1, B11 |
| FR-009 | macOS 호스트: 첫 실행 때 화면 기록·손쉬운 사용·로컬 네트워크 권한 안내 화면, 권한이 빠지면 목록에 「권한 필요」(어느 권한인지). Jump 의 무인 접속용 「Remote Desktop」 항목은 서드파티가 얻을 수 있는지 미확인이라 넣지 않음(02 R14 — 월간 재승인이 생기면 그때 검토) | P0 | R14 |
| FR-010 | 망 끊김·와이파이↔LTE 전환 시 자동 재연결. 순서: ① ICE 재시작 ② RECONNECT_MAX_SEC 안에 화면이 안 돌아오면 새 세션을 자동으로 1회 ③ 그것도 실패하면 「다시 연결」 버튼. 목표: ①에서 RECONNECT_MAX_SEC 안 복귀 | P0 | R8 |
| FR-011 | 클립보드 텍스트 양방향 | P1 | US-3, R7 |
| FR-012 | 모니터가 여럿이면 하나를 골라 본다 | P1 | R3 |
| FR-013 | 화질 단계: 자동 / 절약(SAVER_BITRATE_KBPS 이하) | P1 | R9 |
| FR-014 | macOS 로그인 창(재부팅 후, FileVault 잠금 해제 뒤) 접속 | P1 | R1, B12 |
| FR-015 | 접속 기록(시각·뷰어 기기 정보·호스트·경로·길이)을 ACCESS_LOG_DAYS 일 보관, 웹앱에서 본다 | P1 | STRIDE R |
| FR-016 | 기기 제거: 키 폐기, 이후 그 기기의 연결·세션 즉시 거절 | P1 | STRIDE S |
| FR-017 | 에이전트 자동 업데이트: 개발 PC 의 배포 명령이 새 판(파일은 R2, 정보는 허브)을 올리고, 허브가 알리면 에이전트가 1회용 토큰(UPDATE_TOKEN_TTL_MIN)으로 받아 서명(UPDATE_SIGN_ALG)을 확인한 뒤 교체. 새 판이 허브에 못 붙으면 이전 판으로 복귀 | P1 | R16 |
| FR-018 | VPN 메시: 호스트끼리 사설 주소(VPN_SUBNET)로 IP 통신. 직접 연결 우선, 실패 시 TURN(에이전트 TURN 클라이언트는 이 단계에서 넣음). 목록에 사설 주소와 상대별 경로(직접/중계)·RTT 표시 | P1 | US-4, DL-2 |
| FR-019 | 오디오 전송 | P2 | R10 |
| FR-020 | 파일 전송 | P2 | R11 |
| FR-021 | 원격 깨우기(같은 망 다른 에이전트가 WoL 대신 보냄) | P2 | R2 |
| FR-022 | VPN 기기 이름으로 접속(사설 DNS) | P2 | US-4 |

## 엣지케이스
| 흐름 | Given | When | Then |
|---|---|---|---|
| 기기 추가 | 등록 코드가 만료됨 | 에이전트가 그 코드로 등록 | 거절 「코드 만료」, 웹앱에서 새 코드 |
| 기기 추가 | 누가 코드를 대입 | 틀린 코드가 REG_CODE_MAX_TRIES 회 들어옴 | 활성 코드 폐기(맞는 코드도 이후 거절), 허브 warn 로그, 웹앱에서 새 코드 |
| 기기 추가 | 같은 PC 에 재설치 | 새 키로 등록 | 새 기기로 등록, 옛 항목은 「오프라인」으로 남고 사용자가 제거 |
| 화면 열기 | 호스트 오프라인 | 뷰어가 누름 | 「꺼져 있음 · 마지막 접속 시각」, 세션 만들지 않음 |
| 화면 열기 | 직접 연결 실패(대칭 NAT) | ICE 진행 | TURN 경로로 연결, 「중계」 표시 |
| 화면 열기 | 다른 뷰어가 이미 보고 있음 | 새 뷰어가 엶 | 나중 뷰어가 이어받고 앞 뷰어에 「다른 기기에서 열림」 |
| 화면 열기 | macOS 화면 기록 권한 빠짐 | 뷰어가 엶 | 「Mac 에서 권한 다시 허용 필요」, 검은 화면 대신 안내 |
| 조작 | iPhone 한글 조합 중 | 글자 완성 | 완성 글자만 호스트에 입력, 조합 중 자모가 두 번 들어가지 않음 |
| 조작 | Windows 가 UAC 창으로 바뀜 | 화면 전환 | 화면이 끊기지 않고 UAC 창이 보이고 클릭됨 |
| 재연결 | 와이파이 → LTE 전환 | 연결 끊김 | ICE 재시작으로 RECONNECT_MAX_SEC 안 복귀. 못 하면 새 세션 자동 1회, 그래도 실패하면 「다시 연결」 버튼 |
| 재연결 | iPhone 에서 다른 앱으로 갔다 옴 | Safari 복귀 | 자동 재연결(R18) |
| VPN | 두 호스트가 모두 대칭 NAT | 메시 연결 | TURN 경유, 「중계」 표시 |
| 허브 | Cloudflare 장애 | 뷰어 접속 | 웹앱이 열리지 않음 — 범위 밖 대체 경로(R15, 2차) |

## 성공 기준
| ID | 기준 (pass/fail) | 측정 |
|---|---|---|
| SC-001 | 새 PC 를 추가해 첫 화면을 보기까지 **사용자 행동**이 SETUP_MAX_ACTIONS 이하 — 행동 = 사용자가 무엇을 고르거나 입력하는 단계: ① 웹앱 「기기 추가」 ② 설치 파일 받아 실행 ③ 코드 입력. 허브 주소 입력·새 PC 에서의 계정 로그인 같은 단계는 0개여야 한다(설치 파일은 로그인 없이 받는 1회용 링크 — 05 E-12). **OS 확인 창**(OS 가 띄우는 승인·관리자 암호·시스템 설정 토글 단계)은 따로 세어 Windows WIN_OS_PROMPTS 개(SmartScreen·UAC), macOS MAC_OS_PROMPTS 개(관리자 암호 + Gatekeeper 「그래도 열기」 + 화면 기록·손쉬운 사용·로컬 네트워크) 이하. macOS 수는 실기기로 세어 확정 — 검증 필요, 확인 방법: 첫 작업 3 설치 체크리스트 | 설치 시험 체크리스트로 두 수 세기 |
| SC-002 | iPhone Safari(LTE)에서 기기를 누른 뒤 첫 화면이 뜨기까지 CONNECT_MAX_SEC 이하(10회 중 9회) | 웹앱 세션 계측(누름 → 첫 프레임) |
| SC-003 | 입력 → 화면 반영 지연(뷰어가 입력을 보낸 시각부터 그 입력이 반영된 첫 프레임이 뷰어 화면에 그려진 시각까지, 뷰어 시계 하나로) p50 이 LATENCY_P50_MS 이하(iPhone LTE → 집 PC, 1분) | 06 SC-003 측정법(RTP 타임스탬프 짝짓기) |
| SC-004 | 잠긴 Windows PC 를 iPhone 에서 열어 잠금 해제·UAC 승인까지 할 수 있다 | 수동 시험 1회 + 캡처 장면 기록 |
| SC-005 | 와이파이 → LTE 전환 뒤 RECONNECT_MAX_SEC 안에 화면이 돌아온다(5회 중 5회) | 세션 계측 |
| SC-006 | iPhone 에서 친 한글 문장 「안녕하세요 원격 테스트」가 호스트 메모장에 그대로 들어간다 | 자동 비교(호스트 클립보드로 회수) |
| SC-007 | 키 만료·재로그인 때문에 에이전트를 다시 설정하는 일이 0번(30일 운영) | 접속 기록·에이전트 로그 |
| SC-008 | TURN 중계 구간에 화면·입력 평문이 없다 — 캡처에서 영상은 SRTP(헤더만 평문, 페이로드 암호), 데이터 채널은 DTLS 레코드뿐이고 디코딩 가능한 H.264 NAL 이 없다 | 패킷 캡처 1회(뷰어 `iceTransportPolicy: relay` 강제) |
| SC-009 | 월 추가 비용이 0원 — Workers Paid 포함량 안, TURN 사용량 TURN_FREE_GB 이하 | Cloudflare 사용량 화면 월 1회 |
| SC-010 | (2단계) 호스트 A 에서 호스트 B 의 사설 주소로 ping·TCP 연결 성공, 경로 표시 | 에이전트 시험 명령 |

## UI 방향
- 화면 3개: 기기 목록 · 원격 화면 · 기기 추가(코드). 그 밖은 설정 1개(접속 기록·기기 제거).
- dev:ui dial: 밀도 중간, 모션 낮음(원격 화면 위에서는 0), 폰 우선(목록은 한 손 엄지 영역). DESIGN.md 는 구현 착수 때 고른다.
- 원격 화면 위 도구는 반투명 막대 1개(특수키·키보드·화질·연결 끊기), 길게 누르면 숨김.

## 화면 스케치 — 기기 목록 (폰)
```
┌──────────────────────────┐
│ 내 기기                 ＋│  ← ＋ = 기기 추가(코드 화면)
├──────────────────────────┤
│ ● 집 PC      Windows      │  ● 온라인 / ○ 꺼짐
│   직접 연결 가능           │
│ ● MacBook    macOS        │
│   ⚠ 권한 필요 (화면 기록)  │  ← 누르면 Mac 에서 할 일 안내
│ ○ 회사 PC    Windows      │
│   마지막 접속 3일 전        │
└──────────────────────────┘
빈 상태: 「아직 기기가 없습니다 — ＋ 를 눌러 첫 PC 를 추가하세요」
로딩: 목록 자리 회색 막대 3줄 / 오류: 「허브에 연결할 수 없습니다 · 다시 시도」
```

## 범위 밖
- 우리 iPhone·Android 앱, 폰 VPN, 출구 노드, 집 서비스 허브(사용자 답 Q1·A-2)
- Android 호스트(Q4), iOS 호스트(OS 불가)
- 여러 사용자·팀·SSO·판매(A-1)
- USB 넘기기·가상 모니터·가상 오디오 드라이버(서명 드라이버 필요 — local-inspection J5)
- RDP·VNC 호환, Jump Desktop·Tailscale 과의 호환
- 원격 셸·임의 명령 실행(STRIDE E)
- 허브 없이 같은 망 직접 접속(R15 — 2차 이후)

## 가정 목록
- 02 register 의 Assumed 전부(축 2~6·8~10·12, R1~R20, B2~B14). 핵심:
- **위험 P1** iPhone Safari 웹앱 조작감(한글·특수키·복귀) — 확인 방법: 첫 스켈레톤을 iPhone 실기기 LTE 로 시험.
- **위험 P2** Windows 보안 데스크톱 캡처·입력(DXGI + SYSTEM 도우미) — 확인 방법: 스켈레톤 2단계 spike.
- **위험 B12** macOS 자체 서명 앱의 업데이트 후 권한 유지 — 확인 방법: 서명 같은 두 판으로 업데이트 후 화면 기록 권한 확인. 실패하면 Apple Developer ID(연 99달러)를 사용자에게 묻는다.
- **위험 H1** workers-rs 의 Durable Objects WebSocket 휴면 — API 는 있음(확인 2026-11-23: github cloudflare/workers-rs v0.9.0 `worker/src/durable.rs` `accept_web_socket`·`websocket_message`·`set_websocket_auto_response`·`sql`). 남은 확인 — 검증 필요 — 확인 방법: 24시간 연결 시험에서 연결 유지·과금 시간 확인. 안 되면 허브만 TypeScript(사용자 확인 뒤).
- **위험 H2** Cloudflare Access 로 workers.dev 주소를 보호하고, `/agent/*`·`/healthz` 는 경로별 우회(Bypass)로 열 수 있다 — 검증 필요 — 확인 방법: 대시보드에서 workers.dev 에 Access 앱 1개 + 우회 앱 2개를 걸고 각 경로를 curl 로 확인. 안 되면 도메인 1개 구입(가격은 그때 확인).
- **위험 T1** str0m 은 TURN 할당을 하지 않는다(README 「NIC enumeration and TURN」 — 확인 2026-11-23) → 에이전트용 TURN 클라이언트가 필요. 1단계는 뷰어 브라우저가 TURN 을 쓰므로 영향 작음, 2단계 VPN 은 필수 — 검증 필요 — 확인 방법: crates.io `turn`(webrtc-rs) 클라이언트로 Cloudflare TURN 할당 시험, 안 되면 RFC 8656 할당만 직접 구현.
- **위험 M1** macOS 로그인 창(FR-014)에서 ScreenCaptureKit·TCC 가 동작한다 — 검증 필요 — 확인 방법: 첫 작업 3 에서 LoginWindow 용 LaunchAgent spike.
- **위험 W1** Windows `SendSAS` 는 SoftwareSASGeneration 정책이 필요할 수 있다 — 검증 필요 — 확인 방법: 첫 작업 2 spike 에서 정책 유무별 시험.
- 폰 VPN 없음 → 공식 WireGuard 앱 경로(02 B3)는 쓰지 않는다.

## 상수 표 (단일 출처)
| 이름 | 값 | 단위 | 근거 |
|---|---|---|---|
| SETUP_MAX_ACTIONS | 3 | 사용자 행동 | 설계 결정 DL-7 (비교: Chrome RD 약 5, Jump 약 8, Tailscale+원격 약 9 — research/commercial-products.md) |
| WIN_OS_PROMPTS | 2 | OS 확인 창 | 설계 결정 DL-8 (서명 없는 설치 SmartScreen 1 + 서비스 설치 UAC 1 — 02 R17) |
| MAC_OS_PROMPTS | 5 | OS 확인 창 | 출처 https://support.jumpdesktop.com/hc/en-us/articles/360015527692 (화면 기록·손쉬운 사용) + https://developer.apple.com/documentation/technotes/tn3179-understanding-local-network-privacy (로컬 네트워크) + 02 R17(Gatekeeper 1) + 설계 결정 DL-9(LaunchDaemon 설치 관리자 암호 1) |
| CONNECT_MAX_SEC | 5 | 초 | 설계 결정 DL-7 |
| LATENCY_P50_MS | 150 | ms | 설계 결정 DL-8 (원격 데스크톱 공개 지연 수치를 찾지 못함 — research/remote-desktop-oss.md 표 2. 스켈레톤 실측 뒤 조정) |
| RECONNECT_MAX_SEC | 5 | 초 | 설계 결정 DL-7 |
| REG_CODE_LEN | 8 | 자리(숫자·대문자, 헷갈리는 글자 뺌) | 설계 결정 DL-7 |
| REG_CODE_TTL_MIN | 10 | 분 | 설계 결정 DL-7 |
| REG_CODE_MAX_TRIES | 5 | 회 | 설계 결정 DL-7 |
| ACCESS_SESSION_DAYS | 7 | 일 | 실측: Worker Access 설정 화면의 최대 선택값 7 days(2026-11-23) — DL-11 |
| ACCESS_LOG_DAYS | 90 | 일 | 설계 결정 DL-7 |
| VIDEO_MAX_EDGE | 2560 | px | 설계 결정 DL-7 (B13) |
| H264_MAX_LEVEL | 5.1 | H.264 레벨 | 설계 결정 DL-7 (B13) |
| SAVER_BITRATE_KBPS | 1000 | kbps | 설계 결정 DL-7 (R9) |
| AGENT_HEARTBEAT_SEC | 30 | 초 | 설계 결정 DL-7 (Jump 59초 관찰 — local-inspection J3) |
| TURN_CRED_TTL_MIN | 60 | 분 | 설계 결정 DL-7 |
| WS_TICKET_TTL_SEC | 60 | 초(뷰어 입장권, 1회용) | 설계 결정 DL-10 (05 E-13) |
| VIEWER_UA_MAX | 256 | 글자(접속 기록의 뷰어 User-Agent) | 설계 결정 DL-15 |
| TURN_FREE_GB | 1000 | GB/월 | 출처 https://www.cloudflare.com/products/turn-sfu/ |
| VPN_SUBNET | 10.233.0.0/16 | IPv4 대역 | 설계 결정 DL-7 (Tailscale 100.64.0.0/10 과 겹치지 않게) |
| UPDATE_SIGN_ALG | Ed25519 | 서명 방식 | 설계 결정 DL-7 |
| RATE_LIMIT_PER_MIN | 60 | 요청/분/IP(허브 입구) | 설계 결정 DL-7 |
| WS_MSG_MAX_BYTES | 65536 | 바이트 | 설계 결정 DL-7 |
| OFFLINE_WARN_HOURS | 24 | 시간 | 설계 결정 DL-8 |
| SLO_SUCCESS_PCT | 95 | % (주간 세션 성공률) | 설계 결정 DL-8 |
| TURN_WARN_PCT | 80 | % (TURN_FREE_GB 대비) | 설계 결정 DL-8 |
| AGENT_LOG_FILES | 7 | 파일(하루 1개 회전) | 설계 결정 DL-8 |
| UPDATE_TOKEN_TTL_MIN | 10 | 분 | 설계 결정 DL-8 |
| TURN_PRICE_USD_PER_GB | 0.05 | 달러/GB (포함량 초과분) | 출처 https://developers.cloudflare.com/realtime/turn/ |
| INSTALL_LINK_TTL_MIN | 10 | 분 (설치 파일 1회용 링크 — REG_CODE_TTL_MIN 과 같은 수명) | 설계 결정 DL-9 |
| OFFLINE_DETECT_SEC | 90 | 초 (앱 메시지·자동 응답이 이만큼 없으면 오프라인) | 설계 결정 DL-9 |
| PITR_DAYS | 30 | 일 | 출처 https://developers.cloudflare.com/durable-objects/api/storage-api/ (SQLite DO 시점 복구) |
| TAP_MAX_MS | 250 | ms (터치를 탭=클릭으로 보는 최대 시간) | 설계 결정 DL-16 (iPhone 실기기 C8 뒤 조정) |
| TAP_SLOP_PX | 10 | px (탭으로 보는 최대 손가락 이동) | 설계 결정 DL-16 |
| TRACKPAD_GAIN | 1.5 | 배 (손가락이 영상 폭만큼 움직이면 커서는 1.5배) | 설계 결정 DL-16 |
| LONG_PRESS_MS | 500 | ms (직접 터치에서 길게 누르기 = 오른쪽 클릭·끌기 시작) | 출처 iOS UILongPressGestureRecognizer 기본값 0.5초 (research/touch-control.md) |
| DOUBLE_TAP_MS | 300 | ms (두 번째 탭을 첫 탭 자리에 붙이는·탭 후 끌기로 보는 간격) | 설계 결정 DL-17 |
| ZOOM_MAX | 6 | 배 (집기 확대 상한 — 폰 세로에서 1920 폭 PC 를 1:1 보다 조금 더) | 설계 결정 DL-17 |
| PAN_EDGE_PCT | 15 | % (확대 중 커서가 화면 가장자리 이 폭 안에 들면 화면이 따라감) | 설계 결정 DL-17 |
| TABLET_MIN_PX | 600 | CSS px (화면 짧은 변이 이 이상이면 태블릿 — 기본 조작 방식 = 직접 터치) | 출처 Android sw600dp (research/touch-control.md) |
| VIEWER_WS_RETRY_SEC | 2 | 초 (뷰어 허브 WS 가 끊긴 뒤 다시 붙기 전 대기, 웹앱 `hub.rs` RETRY_MS) | 설계 결정 DL-16 |
| VIEWER_STATS_EVERY_SEC | 60 | 초 (M-10 계측을 다시 보내는 주기, `session.rs` REPORT_EVERY_MS) | 06 SC-003 측정법 「1분」 |
| INPUT_BUFFER_KEEP | 64 | 글자 (숨은 입력칸을 비우는 길이, `remote.rs` TEXT_KEEP) | 설계 결정 DL-16 |
| LATENCY_MATCH_KEEP | 256 | 칸 (지연 짝짓기에 기억하는 프레임·입력 수, 60fps 로 약 4초, `measure.rs` KEEP) | 설계 결정 DL-16 |

## 미결정
0건
