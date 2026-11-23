# 메시 VPN · NAT 통과 조사
버전: v0.1 · 2026-11-23 · 조사: 서브에이전트(sonnet) · 메인 재확인: iroh 라이선스·릴리스(gh api, 2026-11-23 — Apache-2.0, v1.0.0 2026-06-15, v1.5.0 2026-11-19)

## 결론
1. 기반 후보: **iroh**(Rust, Apache-2.0, 1.0 안정, Swift/Kotlin 바인딩, WASM 빌드)를 기기 간 P2P 연결 계층으로. 전체 IP 터널(VPN)이 필요하면 boringtun(BSD-3) 또는 tun-rs + Firezone connlib(Apache-2.0, Rust) 구조를 참고.
2. 브라우저는 메시에 **중계 경유로만** 참여 가능(UDP·TUN 없음). Tailscale(tsconnect: WASM+gVisor+DERP over WebSocket), NetBird(브라우저 클라이언트, 중계 전용), iroh(wasm_browser, 중계 전용) 모두 같은 구조.
3. 라이선스 주의: NetBird 서버 쪽(management/signal/relay) AGPL-3.0 · defguard AGPL · ZeroTier nonfree/ 폴더 비자유 · Mullvad 앱 GPL-3.0 · Tailscale GUI(iOS·mac·Win) 비공개.

## 표 1. VPN 저장소 (gh api 2026-11-23)
| 이름 | 언어 | 라이선스 | 스타 | 최근 릴리스 | OS | 조정 / 중계 / 자체 호스팅 |
|---|---|---|---|---|---|---|
| tailscale/tailscale | Go | BSD-3 | 39,472 | v1.104.7 (11-12) | 전 OS (GUI 일부 비공개) | 조정 서버 비공개 SaaS. 중계 DERP(HTTPS/WebSocket). cmd/derper·cmd/tsconnect 공개 |
| juanfont/headscale | Go | BSD-3 | 45,989 | v0.29.7 (10-30) | Tailscale 공식 클라이언트 사용 | Tailscale 조정 서버 호환 구현, DERP 내장 가능, 호스팅 쉬움 |
| netbirdio/netbird | Go | 클라이언트 BSD-3 / 서버 AGPL-3.0 | 31,217 | v0.83.2 (11-20) | 전 OS + 브라우저 | management+signal+relay, IdP 필요 |
| zerotier/ZeroTierOne | C++ | MPL-2.0 + nonfree | 17,282 | 1.16.2 (05-28) | 전 OS | 자체 L2 프로토콜 |
| slackhq/nebula | Go | MIT | 18,549 | v1.12.0 (10-29) | 전 OS | 인증서 CA, lighthouse+relay, 설정 수작업 많음 |
| firezone/firezone | Elixir+Rust | Apache-2.0 | 9,198 | headless 1.5.16 (11-10) | 전 OS | 허브-스포크 ZTNA(메시 아님). connlib·snownet(ICE, str0m)·smoltcp |
| DefGuard/defguard | Rust | AGPL-3.0+기업 | 2,922 | v2.2.0 (11-06) | WireGuard 클라이언트 | NAT 통과 약함(미확인) |
| tonarino/innernet | Rust | MIT | 5,587 | v2.0.0 (07-02) | Linux/mac | 중계 없음(미확인) |
| cloudflare/boringtun | Rust | BSD-3 | 7,275 | 0.5.2 (2022-07), 푸시 2026-06 | 라이브러리 | WireGuard 사용자 공간만. 릴리스 정체 |

## 표 2. Rust 네트워크 부품
| 부품 | 라이선스 | 스타 | 최신 | 용도·성숙도 |
|---|---|---|---|---|
| n0-computer/iroh | Apache-2.0 | 13,078 | v1.5.0 (11-19) | QUIC+TLS1.3, 공개키로 접속, 홀펀칭+중계(WebSocket), 멀티패스. 1.0 wire 안정. n0 공용 중계는 속도 제한 → 자체 중계 필요 |
| quinn-rs/quinn | Apache-2.0 | 5,357 | 0.11.19 (09-30) | 순수 Rust QUIC |
| smoltcp | 0BSD | 4,692 | v0.14.0 (08-17) | 사용자 공간 TCP/IP. Firezone 사용 |
| netstack-smoltcp | Apache-2.0 | 30~45 | — | 소규모, 유지 위험 |
| tun-rs | Apache-2.0 | 239 | 2.8.18 (11-04) | Win/mac/Linux, Android VpnService fd, iOS NEPacketTunnelProvider fd |
| rust-libp2p | MIT | 5,650 | v0.57.1 (10-15) | DCUtR 홀펀칭 성공률 70%±7.1% (arxiv.org/abs/2510.27500) |
| wireguard-rs | MIT | 535 | 2022 정체 | 사용 불가 |

## NAT 통과
- [확인] 쉬운 NAT/어려운 NAT. 한쪽만 어려우면 256포트 탐색 시 64%, 1,024회 98%, 2,048회 99.9%. 양쪽 다 어려우면 사실상 불가 — https://tailscale.com/blog/how-nat-traversal-works
- [확인] DERP = HTTPS 위 중계 + 직접 경로로 올리는 보조 통로. 저자 추정 "90% 넘게 직접 연결"(측정치 아님) — 같은 글
- [확인] iroh: "roughly 9 out of 10 network configurations allow a direct connection", 실패 예로 회사 방화벽·이동통신망 — https://docs.iroh.computer/concepts/holepunching . "데이터 95%가 직접 경로"(연결률 아님) — https://iroh.computer/blog/v1
- 미확인: 한국 이동통신 3사 CGNAT·대칭 NAT 여부. 직접 실측해야 함. 참고: 네덜란드 5G 논문 https://arxiv.org/abs/2311.04658 , APNIC https://blog.apnic.net/2022/05/03/how-nat-traversal-works-concerning-cgnats/

## 브라우저 참여
- [확인] Tailscale SSH 콘솔: 클라이언트+WireGuard+gVisor+SSH 를 WASM 으로, 임시 인증키, DERP over WebSocket — https://tailscale.com/blog/ssh-console . WASM 약 32MB(제3자, 미검증)
- [확인] NetBird 브라우저 클라이언트: WASM 피어, SSH·RDP(IronRDP WASM), 중계 전용, 소리 없음 — https://docs.netbird.io/manage/peers/browser-client
- [확인] iroh 브라우저: UDP 불가 → 모든 연결 중계 — https://docs.iroh.computer/languages/wasm-browser
- 미확인: 브라우저 WebRTC DataChannel 로 홀펀칭해 메시에 참여한 사례

## 모바일
- [확인] iOS Packet Tunnel 메모리: iOS 10~14.8 15MiB, iOS 15~18 50MiB, iOS 26 값 없음, "하드코딩 말라" — https://developer.apple.com/forums/thread/73148
- [확인] Tailscale 은 iOS 15MB 때문에 Go GC 를 조였고 13MB 가 위태로웠다 — https://tailscale.com/blog/go-linker . Rust 는 GC 가 없어 유리(추정)
- [확인] Firezone: Rust connlib + Swift/Kotlin 껍데기 구조. tun-rs 가 OS fd 를 Rust 로 넘기는 패턴 문서화
- 미확인: Android VpnService 배터리·백그라운드 제한 수치

## 사용자 불만 (원문)
- Tailscale 키 만료: "Now I see that one of the machines key has expired. I can not find a way looking at the Admin console to reauth the key." (2023-06-16) https://forum.tailscale.com/t/unable-to-reauth-expired-key/5639
- Tailscale 원격 Mac 재인증하려면 그 연결을 끊어야 함(요약) https://forum.tailscale.com/t/tailscale-key-expiry/46
- NetBird 배터리 "Hope, that NetBird will get better in barrery-usage." (2025-09-15) https://forum.netbird.io/t/hope-the-netbird-android-app-will-get-better-with-battery-useage/239
- NetBird 로그인 실패 "Error: Unauthenticated, signin works only using setup keys" (2024-06-12) https://github.com/netbirdio/netbird/issues/2123
- ZeroTier Android 배터리 8~10시간 → 4시간(검색 요약, 원문 미확인) https://discuss.zerotier.com/t/android-app-battery-use-wifi-vs-cell/12535

## 반례·불확실
- iroh 는 VPN 이 아니라 연결 라이브러리 — TUN·주소·접근 규칙·조정 서버·키 정책은 직접 만들어야 함.
- 이동통신망에서 직접 연결률은 실측 전 모름 → 중계 운영비와 체감 속도를 좌우.
- 2차 요약에 기댄 항목(ZeroTier 인용, NetBird 모바일 문서, Mullvad JNI)은 원문 재확인 필요.
