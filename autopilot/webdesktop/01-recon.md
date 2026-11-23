# 01 RECON — WebDesktop (가칭)
버전: v0.3 · 2026-11-23 · 강도 full · 개정: 사용자 답(혼자 사용·iPhone 웹앱) + 빌더 체크 #1·#2·#11·#15 + GATE 1차(DL-2·3·6 반영)
근거 파일: `research/local-inspection.md`(이 PC 실측) · `research/remote-desktop-oss.md` · `research/vpn-oss.md` · `research/commercial-products.md`

## 한 장 요약
1. **두 제품은 같은 뼈대다.** Jump Desktop = 신호 서버(neuron.jumpdesktop.com) + WebRTC(ICE·DTLS) 직접 연결 + 실패 시 중계. Tailscale = 조정 서버(공개키 교환소) + WireGuard + STUN 직접 연결 + 실패 시 DERP 중계. → **서버 1대(조정 + 중계) + 기기 에이전트 1개**로 둘을 합칠 수 있다. 단 「같은 중계」는 같은 서버라는 뜻이지 같은 규약이 아니다 — 브라우저 WebRTC 는 TURN 만, 공식 WireGuard 앱은 UDP 고정 주소만 쓴다.
2. **"웹앱만으로"는 안 된다.** 브라우저는 가상 네트워크 카드(TUN)를 못 만들고 화면을 내보내거나 입력을 주입할 수 없다. 브라우저가 할 수 있는 것은 **기기 관리 화면 + 원격 화면 보기(WebRTC, 직접 연결 가능)** 이다. VPN 과 화면 내보내기는 기기마다 작은 네이티브 에이전트가 필요하다.
3. **iPhone 은 "보는 쪽"만 된다.** iOS 는 원격 제어 당하는 쪽(호스트)이 될 수 없다. 우리 VPN 앱을 만들면 App Store 법인 계정이 필요하다(지침 5.4) → 사용자 결정(2026-11-23): **우리 iPhone 앱은 만들지 않는다.** 뷰어는 웹앱. 폰 VPN 은 범위 밖(DL-2).
4. **RustDesk 가 이미 Rust·4개 OS 원격 데스크톱**이다(AGPL). 사람들이 막히는 곳은 기술이 아니라 **설정(서버 주소·키·중계·권한·로그인)** 이다. 우리 차별점은 "설치 1번 + 짧은 코드/QR 1번 → 끝"이어야 한다.
5. **한국 지연:** 이 PC 에서 Tailscale 가장 가까운 중계는 도쿄 39.5ms. 가까운 거점의 중계를 쓰면 직접 연결 실패 시에도 지연을 줄일 수 있다 → Cloudflare TURN(애니캐스트)으로 결정(DL-3). 한국 거점 사용 여부는 실측(02 R20).

## 도메인 업무 흐름 (원격 접속이 실제로 도는 순서)
1. 기기 등록: 기기가 키쌍을 만든다 → 조정 서버에 공개키 + 계정 연결(Tailscale: SSO 로그인, Jump: 계정 로그인 / 연결 코드 `JumpConnectCli --connectcode`) — 출처: tailscale.com/blog/how-tailscale-works, local-inspection J6
2. 상시 대기: 기기가 조정/신호 서버에 계속 붙어 있다(Jump 59초 유지 신호, 끊기면 20초 뒤 재접속 — J3). 이게 "기기 목록·온라인 표시"의 근거.
3. 접속 요청: 보는 쪽이 대상 선택 → 서버가 양쪽 주소 후보(ICE/STUN)를 교환 → 직접 연결 시도(UDP 홀펀칭).
4. 실패 시 중계: Jump Relay(UDP/TCP 443) · Tailscale DERP(HTTPS). 종단 암호화라 중계는 내용을 못 본다.
5. 호스트 승인·권한: macOS 화면 기록 + 손쉬운 사용 + Remote Desktop 항목. (Android 호스트라면 MediaProjection 동의 + 접근성 서비스 — 범위는 Q4.)
6. 세션: 영상(H.264/H.265/AV1) + 입력 + 클립보드 + 오디오. VPN 은 IP 패킷 터널.
7. 운영: 키 만료(Tailscale 180일 기본), 기기 제거, 접근 규칙.

## 이해관계자
- 사용하는 사람 = 돈 내는 사람 = 운영하는 사람 = 사용자 본인(가정 — 00-seed 해석 1). 기기: Windows PC, MacBook, iPhone, Android.
- 외부: Apple(App Store 심사·NetworkExtension 권한), Google Play(VpnService·접근성 서비스 정책), 서버 호스팅 업체(중계 트래픽 비용).

## 규제·표준
| 항목 | 내용 | 해당 |
|---|---|---|
| Apple 심사 지침 5.4 | VPN 앱은 NEVPNManager 사용 + **조직 등록 개발자만** 배포 | 해당 — iPhone VPN 을 App Store 로 낼 때. 개인 사용은 Xcode/TestFlight 내부 테스트 길 검토(검증 필요 — 확인 방법: Apple Developer 문서 Network Extension entitlement) — https://developer.apple.com/forums/thread/747574 , https://developer.apple.com/forums/thread/844103 |
| Google Play 접근성 API 정책 | 원격 제어 목적 접근성 서비스 사용 시 신고·심사 | 해당 — Android 호스트를 Play 로 낼 때(검증 필요 — 확인 방법: Play 정책 센터 AccessibilityService 항목) |
| WebRTC 보안 | DTLS-SRTP 필수(RFC 8827) | 해당 — 브라우저 뷰어 |
| WireGuard / Noise | 키 교환 표준 | VPN 을 WireGuard 로 할 때 |
| 저작권법 제101조의4 · 두 제품 약관 | 역공학 금지. 호환 목적 외 역분석 근거 없음 | 해당 — 디컴파일하지 않고 공개 자료·오픈소스로 설계 (commercial-products.md) |
| 오픈소스 라이선스 | RustDesk AGPL-3.0, Sunshine GPL-3.0 → 코드 복사 금지 | 해당 |
| 전기통신사업법(부가통신사업 신고) | 남에게 유료 서비스로 팔 때 | 1인 사용이면 해당 없음(가정). 판매 시 검증 필요 — 확인 방법: 과학기술정보통신부 부가통신사업 신고 기준 확인 |
| 개인정보보호법 | 남의 기기·화면을 다룰 때 | 1인 사용이면 해당 없음(가정) |

## 유사 솔루션
| 제품 | 원격 화면 | VPN | 브라우저 | 자체 호스팅 | 라이선스 | 우리와의 관계 |
|---|---|---|---|---|---|---|
| Jump Desktop | ○ WebRTC, AV1, 가상 모니터·USB | ✕ (Tailscale 위에서 수동 설정) | 뷰어 ○ | Enterprise 만 | 상용 | 기능·UX 기준점 |
| Tailscale (+Headscale) | ✕ | ○ WireGuard 메시 | SSH 콘솔만(WASM, 중계 전용) | Headscale | BSD-3(클라이언트) | VPN 구조 기준점 |
| RustDesk | ○ 자체 프로토콜 | ✕ | Pro 큰 플랜만 | ○ hbbs/hbbr | AGPL-3.0 | 가장 가까운 경쟁. 설정 불만 사례 |
| NetBird | 브라우저 RDP(IronRDP WASM) | ○ WireGuard | ○ 중계 전용 | ○ (서버 AGPL) | BSD-3/AGPL | "VPN+브라우저 원격" 결합의 선례 |
| Chrome Remote Desktop | ○ | ✕ | ○ | ✕ | 무료 | 설정 최단(약 5단계) 기준점 |

## 기능 지도
- **기본(없으면 안 씀):** 기기 자동 발견·목록·온라인 표시 / NAT 뒤에서도 연결(직접 + 중계) / 종단 암호화 / 화면 보기 + 마우스·키보드 / 클립보드 / 다시 접속 / Win·mac 호스트 / iPhone·Android·브라우저 뷰어 — 근거: 위 5개 제품 공통
- **차별점 후보:** ① 설치 1번 + 코드/QR 1번(현재 Jump 약 8단계, Tailscale+원격 약 9단계 → 목표 3단계 이하) ② VPN 과 원격 화면을 한 앱·한 계정으로 ③ 내 기기 키는 만료 없음(Tailscale 180일 만료 불만) ④ macOS 권한 안내 마법사 ⑤ 가까운 거점 중계(Cloudflare TURN)
- **일부러 안 만들 것(1차):** USB 넘기기·가상 모니터·가상 오디오(서명된 드라이버 필요 — local-inspection J5) / iOS 호스트(OS 불가) / 팀·SSO·감사 로그(1인 사용) / RDP·VNC 호환(새 프로토콜에 집중) / 게임용 240fps

## 흔한 실패
1. **설정이 조용히 틀어짐** — RustDesk 자체 서버가 공개 서버로 몰래 바뀜(#15861), 중계 서버 개념을 몰라 막힘(NixOS 포럼). → 상태를 화면에 보이게.
2. **키 만료·재인증** — 원격 Mac 키가 만료되면 원격으로 되살릴 수 없음(Tailscale 포럼 5639, 46).
3. **macOS 권한 마찰** — 화면 기록 월간 재승인, 26.2 에서 Remote Desktop 항목 단계 실패(Jump 커뮤니티), 일반 실행 파일이 권한 목록에 안 뜸(Apple 포럼 807898).
4. **모바일 배터리·백그라운드** — NetBird·ZeroTier Android 배터리 불만, 제조사 배터리 관리가 서비스를 죽임(RustDesk 글).
5. **이동통신망 직접 연결 실패 → 중계 비용·지연** — iroh 문서가 이동통신망을 실패 예로 명시. 한국 3사 실측 없음.
6. **iOS 메모리 한도** — Tailscale 이 15MB 한도 때문에 고생(go-linker 글). 현재 50MiB 이지만 바뀔 수 있음.

## 전제 확인
- 맞는 문제인가: 사용자 원문 「설정이 번거로워 안 쓰게 된다」 + 이 PC 실측(Tailscale 기기 0대, Jump 호스트 승인 창 5회 놓침 — T4, J7) → 문제는 실재.
- 아무것도 안 하면: Jump Desktop over Tailscale 을 "Direct Only + UDP 허용"으로 수동 설정하면 지금도 동작은 한다(commercial-products.md). 대안 비용 = 설정 9단계 + 180일 재인증.
- 「만들지 말고 쓰자」 대안: Headscale + Tailscale 공식 앱 + RustDesk 를 묶으면 코드 없이 비슷한 것이 된다. 단 설정 단계는 오히려 늘고 Rust 새 구현이라는 요청과 다르다 → 요청대로 새로 만들되, VPN 부분은 검증된 부품(WireGuard/iroh)을 쓴다.
- 사용자에게 닿는 길: Win/mac 은 설치 파일 직접 배포. Android 는 APK 직접 설치 가능. iPhone·Android 는 웹앱만(사용자 답 A-2·Q4) — 설치할 것 없음.

## 스택 후보
| 층 | 후보 A (추천 출발점) | 후보 B | 근거·트레이드오프 |
|---|---|---|---|
| 원격 화면 전송 | WebRTC — `str0m`(MIT, Sans-IO) | `webrtc-rs`(Apache-2.0) | 브라우저 뷰어와 같은 기술이라 **브라우저도 직접 연결 가능**. Jump 도 WebRTC. str0m 은 Firezone 이 실사용 |
| VPN 터널 | WireGuard — `boringtun`(BSD-3) + `tun-rs` | `iroh`(Apache-2.0, QUIC, 1.0 안정) | WireGuard 는 검증된 표준, 단 boringtun 릴리스 정체. iroh 는 홀펀칭·중계·모바일 바인딩을 한 번에 주지만 TUN·주소·규칙은 직접 |
| 연결(NAT 통과)·중계 — 원격 화면 | 서버에 STUN + **TURN**(UDP/TCP/TLS 443) — 브라우저 RTCPeerConnection 이 쓸 수 있는 중계는 TURN 뿐 | — | Cloudflare 애니캐스트 — 한국 거점 여부 실측(02 R20) |
| 연결·중계 — VPN | 호스트끼리만(DL-2): str0m ICE + Cloudflare TURN 위에 boringtun — 04 에서 결정(Firezone snownet 과 같은 조합, `rust/libs/connlib/snownet/src/node.rs` 확인 2026-11-23) | iroh | 원격 화면과 같은 연결 기술·같은 중계 |
| 화면 캡처 | Windows: **DXGI Desktop Duplication**(SYSTEM 도우미면 잠금 화면·UAC 보안 데스크톱까지) · macOS: `screencapturekit` | `windows-capture`(WGC — 보안 데스크톱 못 잡음) · `xcap` | 무인 접속 때문에 Windows 는 DXGI 가 기본 |
| 인코딩 | OS 하드웨어 인코더 직접(Media Foundation · VideoToolbox) + `openh264` 대체 | NVENC 크레이트 | Rust HW 인코더 크레이트가 미성숙 → 직접 호출 필요(가장 큰 구현 위험) |
| 입력·클립보드 | `enigo` · `arboard` | — | |
| 허브 | Cloudflare Worker(Rust `worker` 크레이트) + Durable Object SQLite (DL-3·DL-6) | 서울 VM `axum`+SQLite(버림 — 04 검토한 대안) | 기존 유료 계정, 관리 없음 |
| 웹 화면 | PWA (브라우저 WebRTC + WebCodecs 선택) | — | iOS Safari WebRTC 필수, WebTransport 는 iOS 26.4+ 만 |
| 모바일 | **만들지 않음**(사용자 결정). 뷰어=웹앱, 폰 VPN 범위 밖(DL-2) | — | App Store 심사 회피 |

## 참조 제품 규칙 상세
"Jump Desktop + Tailscale 합침" 요청 — 두 제품의 공개 문서·이 PC 관찰로 기능 범위 확보: **부분**. Jump 프로토콜 세부(Fluid 메시지 형식)는 약관상 보지 않음 → 우리 프로토콜은 새로 설계(호환 불필요).

## 데이터 공급 실측
해당 없음(외부 데이터 의존 없음). 대신 **망 실측**이 필요: 한국 3사 LTE/5G·가정 공유기에서 직접 연결 성공률 — 검증 필요, 확인 방법: 워킹 스켈레톤에서 STUN 결과·연결 경로를 기록(08 첫 작업 후보).

## 현재 시스템 감사
해당 없음(빈 폴더).

## 이전 프로젝트에서 안 된 것
해당 없음(같은 사용자의 비슷한 이전 프로젝트 기록 없음).
