# Jump Desktop · Tailscale 공개 자료 · 경쟁 제품 · 약관
버전: v0.1 · 2026-11-23 · 조사: 서브에이전트(sonnet). 메인 교차 확인: Jump 백서 "built on WebRTC" ↔ local-inspection.md J1(jrtc.dll 안 libwebrtc) 일치.

## Jump Desktop
- 전송: 백서 "Jump Desktop is built on Web Real-Time Communication (WebRTC)". AWS 기반 클라우드가 중개, 직접 연결 실패 시 전 세계 Relay 또는 기업 자체 Relay. 종단 암호화(Relay 는 못 읽음) — https://JumpDesktop.com/docs/JumpDesktopTeamsSecurityPaper.pdf
- 포트: 직접 UDP 35384-35484, 중계 UDP/TCP 80·443, 클라우드 neuron.jumpdesktop.com:443. "Direct Connections Only"(VPN 전용) — https://changelog.jumpdesktop.com/vpn-only-connections-for-fluid-1sSFig
- Cloudless Fluid: 클라우드 없이 IP 직접, Enterprise 전용, TCP 35384 / UDP 35384-35390 — https://changelog.jumpdesktop.com/cloudless-fluid-connections-4c3jLG
- 코덱: 10.7.33(2026-03-16) AV1 추가, GPU 되면 자동. NVIDIA/AMD/Intel HW — https://changelog.jumpdesktop.com/jump-desktop-10-beta-2-474w6Y
- "up to 480 fps", 가상 디스플레이 120/144/240Hz, 4K 6개. 지연 ms 공식 수치 없음 — https://www.jumpdesktop.com/
- Fluid 2.0: 4:4:4, 10-bit·5.1/7.1(Enterprise), FEC, **브라우저 클라이언트 무료** — https://changelog.jumpdesktop.com/jump-desktop-with-fluid-2.0-1yMoBG
- OS: 호스트 Win·Mac·Linux(Alpha). 뷰어 Win·Mac·iPhone/iPad·Android(Beta)·웹·Linux(Alpha)
- 가격: 개인 Win/Android/웹 무료, Mac/iOS 1회 구매. Teams Pro $7.99/사용자/월, Enterprise $15.99 — https://jumpdesktop.com/pricing-plans.html
- macOS 호스트 권한: 화면 기록 + 손쉬운 사용 + (무인용) "Remote Desktop" 항목. 안 하면 화면 기록 매월 재승인. 26.2 에서 이 단계가 안 끝난다는 보고 — https://support.jumpdesktop.com/hc/en-us/articles/360015527692 , https://support.jumpdesktop.com/hc/en-us/community/posts/47163865787149 (검색 요약)

## Tailscale
- 구조: 조정 서버 = "shared drop box for public keys". 개인키는 기기를 떠나지 않음. STUN/ICE, 막히면 DERP — https://tailscale.com/blog/how-tailscale-works
- 오픈소스: 클라이언트 BSD-3, 조정 서버 비공개, GUI(mac·iOS·Win) 비공개 — https://github.com/tailscale/tailscale
- 가격(2026): Personal $0 사용자 6명·기기 무제한·비상업. Standard $8, Premium $18 — https://tailscale.com/pricing
- 키 만료 기본 **180일**, 기기별 끄기 가능, 만료 시 끊김 — https://tailscale.com/kb/1028/key-expiry
- MagicDNS 기본, 주소 100.64.0.0/10 — https://tailscale.com/kb/1081/magicdns
- macOS 판 3종(Standalone·App Store·오픈소스 CLI)이 섞이면 확장이 안 뜸 — https://tailscale.com/kb/1065/macos-variants
- iOS: VPN 구성 설치 + 알림 허용, 키 만료 때 재인증 — https://tailscale.com/kb/1020/install-ios

## 첫 설정 단계 수 (공식 가이드로 센 추정)
| 제품 | 시나리오 | 단계 |
|---|---|---|
| Jump Desktop (Mac 호스트) | 계정 → Connect 설치 → 로그인 → 화면 기록 → 손쉬운 사용 → Remote Desktop 항목 → 뷰어 로그인 → 컴퓨터 선택+Mac 비밀번호 | 약 8 (OS 권한 3) |
| Tailscale 단독 | 가입 → 용도 → 기기A 설치 → 인증 → 기기B 설치 → 인증 | 6 (+OS 승인 1~2) |
| Tailscale + 원격 화면 | 위 + 호스트 RDP/화면공유 켜기 → 주소 확인 → 클라이언트에 입력 | 약 9 |
| 운영 중 | 180일마다 재인증 | 반복 |
| Chrome Remote Desktop | 호스트 설치 → PIN → 접속 | 약 5 (최단) — https://support.google.com/chrome/answer/1649523 |

- 두 제품을 함께 쓰는 공식 가이드 없음. Jump 는 Tailnet 경로가 닿으면 그쪽을 쓴다는 지원팀 답변(검증 방법 없음) — https://support.jumpdesktop.com/hc/en-us/community/posts/43160868665741-Tailscale . 함께 쓰려면 "Direct Only + UDP 허용" 수동 설정.

## 약관·법
- Tailscale ToS §2.3: "modify, disassemble, decompile, reverse engineer …" 금지(조각 인용) — https://tailscale.com/terms . 단 클라이언트 코드는 BSD-3 로 공개 → 소스를 읽으면 됨. 이번에 REA 로 본 것은 실행 파일에 박힌 모듈 목록(메타데이터)뿐.
- Jump EULA: "reverse engineer, decompile or otherwise attempt to extract the source code …" — https://jumpdesktop.com/terms.html
- 저작권법 제101조의4: 정당한 이용자가 **호환에 필요한 부분에 한해** 역분석 가능. 얻은 정보로 실질적으로 유사한 프로그램 개발 금지 — https://ko.wikisource.org/wiki/저작권법_(대한민국) (국가법령정보센터 원문 재확인 필요). → 우리 제품 설계 목적의 디컴파일은 근거 없음. 공개 자료 + 오픈소스로 설계한다.

## 경쟁 제품
| 제품 | 무료 | 브라우저 접속 | VPN | 비고 |
|---|---|---|---|---|
| Chrome Remote Desktop | 무료(미검증) | 있음 | 없음 | 설정 최단 |
| Parsec | Personal 무료 60fps | 문서 근거 없음 | 없음 | https://parsec.app/pricing |
| AnyDesk | 무료 여부 충돌 | 미확인 | 미확인 | |
| TeamViewer | 개인 무료 | 무료 티어엔 없음 | 없음 | |
| RustDesk | 오픈소스, SaaS 아님, Server Pro $11.88/월~ | Pro 큰 플랜만 | 없음 | https://rustdesk.com/pricing |
| Splashtop Personal | 같은 LAN 만 무료 | 미확인 | 없음 | |

## 불확실
- Jump 지원 문서 다수가 403 → 검색 요약 의존. Fluid 가 순수 WebRTC 인지 일부만인지 불명(로컬 관찰로는 ICE·DTLS-SRTP·SCTP 모두 있음).
- 가격·무료 여부는 제3자와 충돌 → 쓰기 전에 공식 재확인.
