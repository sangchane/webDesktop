# 02 블라인드스팟 레지스터 — WebDesktop
버전: v0.4 · 2026-11-23 · 개정: GATE 1차 지적 반영(DL-3·4·6 이후 낡은 줄 정리)
스캔: 공통 12축 + STRIDE 6범주 + 현업 바닥선 17 + 임시 프로파일 「원격 접속」(20항목) + P2 웹 SaaS 6항목 · P4 모바일 5항목(아래 표)

## 사용자 답 (2026-11-23, 원문)
> "나 혼자 사용할거야 아이폰 때문에 웹앱에서 접근해서 제어하고 싶어 앱스토어 심사 받는게 쉽지 않잖아"
- A-1 사용 범위 = **나 혼자** (Clear) → 다중 사용자·팀·판매·개인정보 처리 규제 범위 밖.
- A-2 iPhone = **우리 앱 없음, 웹앱(브라우저)으로 접속해 제어** (Clear) → App Store 심사·조직 계정 불필요.
- 파생 결정(내 판단, decision-log 기록): 접속하는 쪽(뷰어)은 모든 기기에서 **같은 웹앱**(iPhone·Android·다른 PC 브라우저). 네이티브 코드는 **제어 당하는 Win·Mac 에이전트 + 서버**만. 폰 VPN 은 Q1 답으로 범위 밖(DL-2). 나중에 필요하면 공식 WireGuard 앱 + QR 경로가 있다(https://apps.apple.com/us/app/wireguard/id1441195209) — 단 UDP 허브가 필요해 Cloudflare 만으로는 불가.

## 공통 12축
| 축 | 상태 | 처리 | 근거 |
|---|---|---|---|
| 1. 기능 범위 | Asked → 답 Clear | Q1=메시, Q3=원격 화면 먼저, Q4=Android 뷰어만 | VPN 을 무엇에 쓰는지에 따라 메시/출구/서브넷 설계가 갈림. 조사로 못 정함 |
| 2. 데이터 모델 | Assumed | 기기(공개키·이름·OS·마지막 접속) · 세션 기록 · 소유자 인증 수단 · VPN 주소 할당. 1인이라 수십 행 규모 | 01 도메인 흐름 1~7 |
| 3. UX 흐름 | Assumed | 웹앱: 로그인 → 기기 목록(온라인 표시) → 누르면 화면 → 터치=트랙패드 방식. 기기 추가 = 웹앱 「기기 추가」 → 설치 파일 받기·실행 → 화면의 REG_CODE_LEN 자리 코드 입력(03 SC-001) | Jump 연결 코드(J6), Chrome RD 최단 흐름 |
| 4. 비기능 | Assumed | 지연·접속·재연결 목표는 03 상수 표(LATENCY_P50_MS 등 — 설계 결정, 스켈레톤 실측 뒤 조정). 기기 추가 SETUP_MAX_ACTIONS. 종단 암호화 필수 | 03 상수 표 DL-7·DL-8 |
| 5. 외부 의존 | Assumed | Cloudflare(Worker·DO·TURN·Access·정적 자산, DL-3·DL-6) — HTTPS 는 workers.dev 가 제공. Cloudflare 장애 시 대체 경로 없음(R15 2차, 07 장애 표) | 01 스택, DL-3 |
| 6. 엣지·실패 | Assumed | 끊김 시 자동 재연결(03 FR-010 순서), 호스트 절전, 두 기기 동시 접속 시 나중 것이 이어받음 | 원격 접속 프로파일 R6·R8 |
| 7. 제약 | Asked → 답 Clear | Q2=Cloudflare, 이후 사용자 지시로 기존 Workers Paid 계정(DL-6) | 1인 예산·운영 의지가 답 |
| 8. 용어 | Assumed | 용어 고정: 「호스트」=제어 당하는 PC/Mac, 「뷰어」=웹앱으로 보는 기기, 「허브」=Cloudflare Worker+DO(조정·신호), 「중계」=Cloudflare TURN, 「에이전트」=호스트에 까는 프로그램 | — |
| 9. 완료 신호 | Assumed | 03 SC 로 정함: iPhone Safari 에서 LTE 로 집 Windows PC·MacBook 을 열고 조작 | 원문 |
| 10. 비용·라이선스 | Assumed | AGPL/GPL 코드 복사 금지. 서버 = 기존 Workers Paid 계정(추가 비용 0 목표, SC-009). TURN 월 TURN_FREE_GB 포함. Apple·Google 개발자 비용 0 | 01 라이선스 |
| 11. 사용 맥락 | Clear | 나 혼자. 밖에서 iPhone 으로 집 PC·Mac 을 잠깐 조작(짧은 세션 추정) + 다른 PC 에서 긴 세션. 4개 OS | 원문 + A-1, A-2 |
| 12. 현업 바닥선 | Assumed | 아래 표 | engineering-baseline.md |

## 임시 프로파일 「원격 접속」
| # | 항목 | 상태 | 처리 |
|---|---|---|---|
| R1 | 무인 접속(아무도 없는 PC, 로그인 화면·UAC 창 포함) | Assumed | 예. Windows 는 서비스(SYSTEM) + 사용자 세션 도우미 2개 프로세스, macOS 는 LaunchDaemon+LaunchAgent. **위험: 구현 난도 상** — 사전 부검 #2 |
| R2 | 호스트 깨우기(절전·꺼짐) | Assumed — 미룸: 2차. 같은 망 다른 기기 에이전트가 Wake-on-LAN 대신 보냄(Jump MacAddresses J4) |
| R3 | 여러 모니터 | Assumed | 모니터 선택만(동시 표시는 범위 밖) |
| R4 | 해상도·고해상도(Retina) | Assumed | 원본 해상도 송출 + 뷰어 쪽 확대/축소. 해상도 바꾸기는 범위 밖 |
| R5 | iPhone 터치 → 마우스 | Assumed | 트랙패드 방식(한 손가락 이동, 탭=클릭, 두 손가락 탭=오른쪽, 두 손가락 끌기=스크롤, 집기=확대) — Jump 제스처 프로필(J4 GestureProfileCode) 참고 |
| R6 | 키보드(한글 입력·단축키) | Assumed | 뷰어에서 완성된 글자를 보냄(유니코드 입력) + 특수키 막대(Ctrl·Alt·Cmd·Esc·Tab). **위험: iOS 한글 조합 입력** → 검증 필요 — 확인 방법: 스켈레톤에서 iPhone Safari 한글 입력 시험 |
| R7 | 클립보드 | Assumed | 텍스트만 양방향 |
| R8 | 연결 끊김·망 전환(와이파이↔LTE) | Assumed | 03 FR-010 순서: ICE 재시작 → RECONNECT_MAX_SEC 안 복귀 못 하면 새 세션 자동 1회 → 그래도 안 되면 「다시 연결」 버튼 |
| R9 | 이동통신 데이터 사용량 | Assumed | 뷰어에서 화질 단계(자동/절약). 절약 = SAVER_BITRATE_KBPS 이하 |
| R10 | 오디오 | Assumed — 미룸: 2차 |
| R11 | 파일 전송 | Assumed — 미룸: 2차 |
| R12 | 호스트 화면 가리기(원격 중 모니터에 보이는 것) | Assumed — 범위 밖(혼자 사용, 집) |
| R13 | 코덱·하드웨어 인코딩 | Assumed | H.264 먼저(모든 브라우저 디코딩). HW 인코더 실패 시 openh264. AV1/H.265 는 2차 |
| R14 | macOS 권한(화면 기록·손쉬운 사용·월간 재승인) | Assumed | 에이전트를 .app 번들로, 첫 실행 권한 안내 화면, 권한 빠지면 웹앱 기기 목록에 「권한 필요」 표시. 월간 재승인 회피 여부 **검증 필요** — 확인 방법: macOS 26 에서 스켈레톤 설치 후 30일 관찰 |
| R15 | 허브(Cloudflare) 장애 시 | Assumed — 미룸: 2차 이후. 1차는 허브 필수, 대체 경로 없음(07 장애 표) |
| R16 | 에이전트 업데이트 | Assumed | 서버가 새 판을 알려 에이전트가 서명 확인 후 스스로 교체 |
| R17 | 코드 서명(Windows SmartScreen, macOS Gatekeeper) | Assumed | 혼자 쓰므로 서명 없이 설치 + 경고 1회 허용. macOS 는 공증 없이 「그래도 열기」. **위험: macOS 권한 목록 등록이 서명에 묶일 수 있음** → 검증 필요 — 확인 방법: R14·B12 와 같은 업데이트 후 권한 유지 시험 |
| R18 | iPhone 웹앱 백그라운드 | Clear | iOS Safari 는 백그라운드에서 연결이 끊김 → 앱 전환 후 돌아오면 재접속. 웹앱의 한계로 수용 |
| R19 | 뷰어 기기 분실 | Assumed | Cloudflare Zero Trust 대시보드에서 그 사용자 세션 폐기(revoke) — 로그인은 Access 이메일 PIN(DL-4). 07 런북 RB-5 |
| R20 | 중계 위치 | Assumed | Cloudflare TURN 은 애니캐스트(가까운 거점). 한국 거점 사용 여부 검증 필요 — 확인 방법: 스켈레톤 세션에서 TURN 경로 RTT 측정(도쿄 39.5ms T3 와 비교) |

## STRIDE (경계: 인터넷 ↔ 서버, 서버 ↔ 호스트 에이전트, 뷰어 ↔ 호스트)
| 범주 | 유효? | 처리 |
|---|---|---|
| S 위장 | 유효 — 남이 내 계정으로 로그인하면 내 PC 전체를 조작 | Mitigate: Cloudflare Access 이메일 PIN, 허용 이메일 1개(DL-4). 기기 등록은 일회용 코드(REG_CODE_TTL_MIN). 에이전트는 기기 키로 허브 인증. 상세 04 위협모델 |
| T 변조 | 유효 — 서버가 털리면 화면 스트림 중간자 | Accept + Mitigate: 웹앱 코드를 서버가 내려주므로 **웹 뷰어에서는 서버를 믿을 수밖에 없다**(빌더 체크 B8). 허브 = 사용자 Cloudflare 계정의 Worker(배포 권한 = 계정 권한, 계정 2단계 인증은 사용자 몫). 뷰어↔호스트는 WebRTC DTLS 종단 암호화(중계·망 구간 보호) |
| R 부인 | 약함(1인) | Mitigate: 접속 기록 ACCESS_LOG_DAYS 보관 — 침입 발견용 |
| I 노출 | 유효 | Mitigate: TURN 중계는 DTLS 암호문만 전달. 서버 DB 에 화면·입력 저장 안 함. (폰 VPN 허브 예외는 DL-2 로 범위 밖이 되어 사라짐) |
| D 서비스 거부 | 유효 — 공개 서버 | Mitigate: 로그인·코드 입력 빈도 제한, TURN 은 인증된 세션만 |
| E 권한 상승 | 유효 — 에이전트가 SYSTEM 권한으로 돎 | Mitigate: 에이전트는 서버가 보낸 명령 중 정해진 몇 개만 받음(임의 명령 실행 기능 없음). 원격 셸은 범위 밖 |

## 현업 바닥선 1~17
| # | 항목 | 상태 | 처리 |
|---|---|---|---|
| 1 | 코드 구조 | Assumed | Cargo 작업 공간: `hub`(입구→서비스→저장소) · `agent` · `proto`(공유 계약) · `web`(PWA). 04 「코드 구조」 |
| 2 | 의존성 조립 | Assumed | 실행 파일마다 `main` 한 곳에서 조립 |
| 3 | 오류 처리 | Assumed | 기본값(두 갈래) |
| 4 | 입력 꼴 검사 | Assumed | 서버 입구와 에이전트 수신부에서 메시지 꼴·크기 상한 |
| 5 | 로그 | Assumed | `tracing` JSON 한 줄. 키·코드·화면 내용 금지 |
| 6 | DB 구조 | Assumed | Durable Object 내장 SQLite + 번호 마이그레이션, 색인 대조표 05 |
| 7 | DB 연결 | Clear(해당없음 — DO 내장 SQLite 라 원격 연결·풀 없음, 05 저장소 설계) |
| 8 | 설정 | Assumed | 허브: Worker 비밀을 한 곳에서 검사, 빠지면 모든 요청 500 + error 로그(Worker 는 기동 거부 개념 없음 — 07). 에이전트: 설정 파일 검사 실패 시 기동 거부 |
| 9 | 종료 보존 | Assumed | 종료 신호에 세션 닫기 알림 후 종료 |
| 10 | 주석 | Assumed | 기본값 |
| 11 | 빈도 제한 | Assumed | STRIDE D 와 같음 |
| 12 | 상태 확인 | Assumed | `/healthz`, 연결 경로(직접/중계) 비율·세션 지연 기록 |
| 13 | 백업·복원 | Assumed | DO SQLite 시점 복구(PITR) + 설치 때 복원 연습 1회(07). 잃어도 기기 재등록이면 복구됨 |
| 14 | 만료 정리 | Assumed | 등록 코드·세션 토큰 만료분 주기 삭제 |
| 15 | 공유 계약 | Assumed | `proto` 크레이트 하나 → 웹은 같은 정의에서 TS 타입 생성 |
| 16 | 품질 도구 | Assumed | `cargo fmt/clippy/test` + 웹 typecheck 를 한 명령 |
| 17 | 의존 패키지 | Assumed | `Cargo.lock` 고정, `cargo deny`(라이선스: AGPL/GPL 차단) |

## P2 웹 SaaS · P4 모바일 (해당 항목)
| 항목 | 상태 | 처리 |
|---|---|---|
| P2 테넌시 | Clear(해당없음 — 1인, A-1) | — |
| P2 인증 | Clear | Access 이메일 PIN(DL-4) |
| P2 결제·구독 | Clear(해당없음 — 판매 안 함) | — |
| P2 백업·DR | Assumed | 바닥선 13, 07 장애 표 |
| P2 개인정보 | Clear(해당없음 — 1인, 저장 개인 정보는 이메일(Access 쪽)과 기기 이름뿐) | 04 프라이버시 |
| P2 이메일/알림 | Clear | PIN 메일은 Access 가 보냄. 우리 메일 없음 |
| P4 오프라인 | Clear(해당없음 — 원격 화면은 온라인 전제) | — |
| P4 스토어 심사 | Clear(해당없음 — 폰 앱 없음, A-2) | — |
| P4 강제 업데이트 | Assumed | 에이전트: proto major 불일치 → `upgrade_required` + 자동 업데이트(FR-017). 웹앱은 허브가 매번 최신 제공 |
| P4 미디어 용량 | Assumed | 화질 단계(FR-013) |
| P4 기기 분실 | Assumed | R19 |

## 빌더 체크 (규칙 15) — 새 맥락 검토관(dev:reviewer) 1회, 15건
| # | 추측해야 하는 곳 | 무더기 | 처리 |
|---|---|---|---|
| B1 | 원격 화면 중계: 01 은 DERP 식, 02 는 TURN — 모순 | 이미 답 있음(02 가 맞음) | 01 v0.2 스택 표를 TURN 으로 고침 |
| B2 | 「두 기능이 같은 중계」의 뜻 | 사소함 | Assumed(내 판단): 같은 서버 1대, 규약은 따로(TURN / WireGuard UDP / 호스트 VPN 경로). 01 v0.2 요약 1 |
| B3 | 폰 VPN = 공식 WireGuard 앱이면 서버가 허브 → 서버가 내부 패킷을 봄 | 사소함 | DL-2 로 해소(폰 VPN 범위 밖) — 더 이상 가정 아님 |
| B4 | 호스트끼리 VPN NAT 통과 방식(자체 vs iroh) | 사소함(기술 선택) | A4 에서 결정 — 후보 비교 표를 A4 에 |
| B5 | VPN 용도 미정 | **사용자 몫** | → Q1 |
| B6 | 원격 화면이 VPN 위로 가나 | 사소함 | Assumed(내 판단): 원격 화면은 VPN 과 독립된 WebRTC 경로. 에이전트는 VPN 인터페이스 주소를 ICE 후보에서 뺀다(터널 안 터널 방지) |
| B7 | 신호 규약 | 사소함 | Assumed: 브라우저·에이전트 ↔ 서버 WebSocket(TLS) + JSON, `proto` 크레이트에서 `ts-rs` 로 TS 타입 생성. A5 에 계약 |
| B8 | 뷰어 신뢰 경로, 서버가 JS 를 바꿀 수 있음 | 이미 답 있음(틀린 주장) | STRIDE T 를 「웹 뷰어는 서버를 믿는다」로 고침 |
| B9 | 첫 소유자 등록·패스키 분실 복구 | 사소함 | DL-4 이후 해소: 로그인은 Cloudflare Access(소유자 이메일). 첫 소유자 = Access 정책에 적은 이메일, 복구 = 이메일 계정 복구 |
| B10 | 기기 등록 코드 방향 | 사소함 | Assumed: 웹앱이 코드(REG_CODE_LEN·REG_CODE_TTL_MIN·REG_CODE_MAX_TRIES, 활성 코드는 하나뿐) 표시 → 에이전트 설치 창에 입력. 에이전트가 만든 기기 공개키를 코드와 함께 서버에 등록 |
| B11 | Windows 잠금 화면·UAC 캡처(WGC 불가) | 이미 답 있음(사전 부검 P2) | 01 v0.2 캡처 행을 DXGI Desktop Duplication + SYSTEM 도우미로 고침. 스켈레톤 spike 유지 |
| B12 | macOS 서명 없는 앱 자동 업데이트 시 권한(TCC) 초기화 | 사소함 + 검증 필요 | Assumed: 사용자 키체인의 자체 서명 인증서로 매 판 같은 서명 → 권한 유지 기대. **검증 필요** — 확인 방법: 서명 바꾼 판 2개로 업데이트 후 화면 기록 권한 유지 확인. 안 되면 Apple Developer ID(연 99달러, App Store 심사 아님)를 그때 사용자에게 묻는다 |
| B13 | 영상: H.264 레벨·5K 축소·비트레이트 연동 | 사소함 | Assumed: 긴 변 최대 2560px(넘으면 축소), H.264 High 프로파일 레벨 5.1 이하, str0m 대역폭 추정값으로 인코더 비트레이트 조정 — A4 |
| B14 | 데스크톱 브라우저가 Win/Cmd·Alt+Tab 을 먼저 가져감 | 사소함 | Assumed: 전체 화면 + Keyboard Lock API(Chromium, Safari 26.4 — https://9to5mac.com/2026/03/24/apple-details-safari-26-4-with-44-new-features-191-bug-fixes-more/), 안 되는 키는 특수키 막대. 입력은 DataChannel 순서 보장·신뢰 모드 |
| B15 | 01 에 모바일 앱·Android 호스트가 남아 02 와 모순 | 이미 답 있음 | 01 v0.2 에서 고침. Android 호스트 범위는 **사용자 몫** → Q4 |

## 가정 사전 부검 (규칙 16) — Impact 상위 3
| 가정 | 「6개월 뒤 실패했다면 이것 때문?」 | 뒤집기(확실히 망하려면) | 결과 |
|---|---|---|---|
| P1. iPhone Safari 웹앱(WebRTC)만으로 조작감이 충분하다 | 가능 — 한글 조합 입력이 깨짐, 단축키 못 보냄, 앱 전환마다 끊김, 화면 확대·스크롤이 불편하면 결국 안 씀(지금과 같은 결과) | Safari 가상 키보드에 그대로 맡기고 iPhone 실기기 시험 없이 PC 브라우저로만 확인 | **03 가정 `위험`** — 확인 방법: 워킹 스켈레톤 첫 판을 iPhone 실기기(Safari, LTE)로 시험. 한글·특수키 막대·재접속 시간 측정. 실패 시 대안: 홈 화면 웹앱(PWA) 모드·자체 입력창 |
| P2. 무인 접속(잠긴 Windows·UAC 창·macOS 로그인 창)을 에이전트로 해낼 수 있다 | 가능 — 집 PC 가 잠기면 밖에서 못 들어가 제품 가치가 0. Windows 보안 데스크톱은 일반 캡처(WGC)로 안 잡히고 SYSTEM 프로세스가 필요 | 사용자 세션 안 일반 프로그램으로만 만들기 | **03 가정 `위험`** — 확인 방법: 스켈레톤 2단계에서 Windows 잠금 화면 캡처·입력(서비스+세션 도우미) spike. 안 되면 대안: Windows 자동 로그인 + 잠금 안 함 설정 안내(보안 약화를 화면에 경고) |
| P3. 폰 VPN 은 공식 WireGuard 앱 + QR 로 충분하다 | 약함 — 공식 앱은 고정 상대 주소만 알아 NAT 통과를 못 함 → 폰은 항상 서버를 거침(허브). 폰 VPN 사용량이 크면 서버 전송량·지연 문제 | 폰으로 집 NAS 영상을 VPN 으로 대량 스트리밍 | **버팀**(1인·가벼운 사용) — 단 Q1 답이 「출구 노드(집 인터넷 우회)」면 서버 전송량을 A7 에서 계산. 서버 전송 한도는 검증 필요 — 확인 방법: 선택한 서버 요금표 |
## 질문 배치 (사용자 몫 4건) — 2026-11-23 제시
- Q1. VPN 을 무엇에 쓰나(의도, 여러 개 선택) — B5, 축1
- Q2. 서버를 어디에 두나 — 축7, R20
- Q3. 첫 완성본 범위 — 축1·9
- Q4. Android 폰도 제어 당하게 하나 — B15
### 답 (2026-11-23, 원문)
- Q1 VPN 용도: 「기기끼리 직접 통신, 근데 왜 jump to desktop 사용할때 tailscale을 필수처럼 쓰는건데?」 → **메시(호스트끼리 직접 통신)**. 폰 VPN·출구 노드·집 서비스 허브는 범위 밖. 뒷부분은 질문 — 최종 보고에서 답함(Jump 는 Tailscale 없이도 자체 클라우드·중계로 됨).
- Q2 서버: 「클라우드 플레어만 있어도 가능해? 아니면 오라클 같은게 필요해?」 → 질문형 답. 조사 결과 **Cloudflare 만으로 가능**(폰 VPN 이 빠져 UDP 허브가 필요 없음) → 추천안으로 채택(DL-3). 근거: TURN 월 1,000GB 무료 https://www.cloudflare.com/products/turn-sfu/ · STUN 무료 https://developers.cloudflare.com/calls/turn/faq · Durable Objects 무료 플랜(SQLite) https://developers.cloudflare.com/changelog/2025-04-07-durable-objects-free-tier · Access 무료 50명 https://www.cloudflare.com/plans/sase-zero-trust.md
- Q3 첫 범위: **원격 화면 먼저 (추천)**.
- Q4 Android: **보는 쪽만 (추천)**.


