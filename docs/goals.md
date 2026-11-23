# 목표 한 장 — WebDesktop
버전: v1.2 · 2026-11-23 · 기준 03 v1.2 · 상세 기획: `autopilot/webdesktop/03-prd.md`

새 요청은 먼저 아래 목표 몇 번에 해당하는지 대조한다. 목표를 바꾸는 말이면 이 파일부터 고친다.

## 목표 1 — 설정 한 번으로 끝 (G1)
- **사용자 원문 (2026-11-23):** "지금의 경우는 설정이 번거롭다고 느껴서 안쓰게 되는거 같거든"
- **조사:**
  - 첫 설정 단계를 세어 봤습니다(공식 가이드 기준). Jump Desktop(Mac 호스트) 약 8단계, Tailscale 과 원격 화면을 합치면 약 9단계, Chrome Remote Desktop 약 5단계 — `autopilot/webdesktop/research/commercial-products.md`.
  - 계속 드는 비용도 있습니다. Tailscale 키는 기본 180일마다 만료됩니다 — https://tailscale.com/kb/1028/key-expiry (2026-11-23).
  - 실무자 불만 1: Tailscale 키가 만료되면 원격 기기를 되살리기 어렵습니다 — https://forum.tailscale.com/t/unable-to-reauth-expired-key/5639
  - 실무자 불만 2: RustDesk 자체 서버 주소가 조용히 공개 서버로 바뀝니다 — https://github.com/rustdesk/rustdesk/issues/15861
  - 이 PC 실측: Tailscale 연결 기기 0대, Jump 호스트 승인 창을 5번 놓쳤습니다 — research/local-inspection.md T4·J7.
- **세운 목표:**
  - 새 PC 는 3단계 안에 추가합니다: 웹앱 「기기 추가」 → 설치 파일 받아 실행(로그인 없는 링크) → 코드 입력.
  - 행동과 별도로 OS 확인 창(예/허용만 누르는 것)은 Windows 2개, Mac 5개 이하입니다. Mac 5개는 관리자 암호, 「그래도 열기」, 화면 기록, 손쉬운 사용, 로컬 네트워크입니다. Mac 수는 실기기로 세어 확정합니다.
  - 허브 주소 입력과 계정 로그인은 0번입니다.
  - 한 번 등록한 기기는 키 만료나 재로그인으로 다시 설정하지 않습니다.
- **원문과 달라진 점:** 「번거롭지 않게」를 행동 수와 재설정 0회로 바꿨습니다. 재설정 0회를 넣은 이유는 첫 설정 못지않게 키 만료·재인증이 이탈 원인이기 때문입니다(위 불만 1).
- **완료 기준과 잼 수단:**

| 완료 기준 | 잼 수단 | 지금 |
|---|---|---|
| SC-001 사용자 행동 ≤ SETUP_MAX_ACTIONS, OS 확인 창 ≤ WIN_OS_PROMPTS / MAC_OS_PROMPTS | 설치 시험 체크리스트(만들 것) | 잼 수단 없음 |
| SC-007 30일 재설정 0회 | 접속 기록 + 에이전트 로그 집계 스크립트(만들 것) | 잼 수단 없음 |

- **정답지 출처:** Chrome Remote Desktop 공식 단계 https://support.google.com/chrome/answer/1649523

## 목표 2 — 어디서나 브라우저로 조작 (G2)
- **사용자 원문 (2026-11-23):**
  - "Window PC <-> Mac book <-> iPhone <-> Android 간 호환이 자유로웠으면 좋겠어"
  - "아이폰 때문에 웹앱에서 접근해서 제어하고 싶어 앱스토어 심사 받는게 쉽지 않잖아"
- **조사:**
  - iPhone 은 OS 가 막아 호스트가 될 수 없습니다 — https://rustdesk.com/blog/rustdesk-remote-control-android-ios
  - iPhone 에서 우리 VPN 앱을 쓰려면 법인 개발자 계정이 필요합니다(지침 5.4) — https://developer.apple.com/forums/thread/747574
  - Safari 26.4 부터 WebTransport 와 Keyboard Lock 을 지원합니다 — https://webkit.org/?p=17862
  - Jump Desktop 도 WebRTC 위에 만들었습니다 — https://JumpDesktop.com/docs/JumpDesktopTeamsSecurityPaper.pdf 와 research/local-inspection.md J1 이 일치합니다.
  - 가장 가까운 Tailscale 중계는 도쿄 39.5ms 입니다(이 PC 실측 T3). Cloudflare TURN 은 애니캐스트입니다(가장 가까운 거점으로 연결).
- **세운 목표:**
  - 모든 뷰어는 같은 웹앱 하나를 씁니다: iPhone·Android·다른 PC 브라우저.
  - 호스트는 Windows·Mac 두 가지입니다.
  - 잠긴 Windows 까지 열 수 있어야 합니다.
  - LTE 에서도 쓸 만해야 합니다: 첫 화면 5초 안, 입력 지연 p50 150ms 이하.
- **원문과 달라진 점:**
  - 「4개 OS 상호 호환」을 「4개 OS 에서 보기 + 2개 OS 가 호스트」로 좁혔습니다. iOS 는 OS 제약으로 호스트가 될 수 없고, Android 호스트는 사용자가 뺐습니다(Q4).
  - 「잠긴 PC 열기」를 더했습니다. 무인 접속이 안 되면 밖에서 쓸 수 없기 때문입니다(사전 부검 P2).
- **완료 기준과 잼 수단:**

| 완료 기준 | 잼 수단 | 지금 |
|---|---|---|
| SC-002 첫 화면 ≤ CONNECT_MAX_SEC | 뷰어 계측 M-10 → `tools/report.py` | 잼 수단 없음 |
| SC-003 지연 p50 ≤ LATENCY_P50_MS | RTP 타임스탬프 짝짓기(06 측정법) | 잼 수단 없음 |
| SC-004 잠긴 Windows 해제 | 수동 시험 + 장면 기록 | 잼 수단 없음 |
| SC-005 망 전환 재연결 | 뷰어 계측 M-10 → `tools/report.py` | 잼 수단 없음 |
| SC-006 iPhone 한글 입력 | 호스트 클립보드 회수 비교 | 잼 수단 없음 |

- **정답지 출처:** 원격 데스크톱 지연의 공개 기준 값은 찾지 못했습니다(Jump Desktop 도 ms 를 공개하지 않음). 그래서 150ms 는 설계 결정(DL-8)입니다. 첫 시제품 실측 뒤 조정합니다.

## 목표 3 — 기기끼리 사설망 (G3, 2단계)
- **사용자 원문 (2026-11-23):**
  - "원격데스크탑 및 VPN까지 제공되는 프로그램"
  - Q1 답: "기기끼리 직접 통신"
- **조사:**
  - 쉬운 NAT 는 대부분 직접 연결되고, 양쪽 다 어려운 NAT 는 중계가 필요합니다 — https://tailscale.com/blog/how-nat-traversal-works
  - iroh 공식 문서: 망 구성 약 10개 중 9개가 직접 연결됩니다 — https://docs.iroh.computer/concepts/holepunching
  - 이 집 망은 직접 연결이 쉬운 형태이고 UPnP(공유기 자동 포트 열기)가 됩니다(T2).
- **세운 목표:** Windows 호스트와 Mac 호스트가 사설 주소(VPN_SUBNET)로 통신합니다. 직접 연결이 안 되면 Cloudflare TURN 을 거칩니다.
- **원문과 달라진 점:** 폰은 VPN 에서 뺐습니다(Q1). 폰 VPN 을 넣으려면 공식 WireGuard 앱과 UDP 허브가 필요하고, Cloudflare 만으로는 둘 수 없습니다.
- **완료 기준과 잼 수단:**

| 완료 기준 | 잼 수단 | 지금 |
|---|---|---|
| SC-010 사설 주소 ping·TCP | 에이전트 시험 명령 | 잼 수단 없음 |

## 공통 — 비용
- SC-009: 월 추가 비용 0원입니다(기존 Workers Paid 포함량 안, TURN 월 1,000GB 이하).
- 잼 수단: Cloudflare 사용량 화면. 지금: 잼 수단 없음(배포 전).
