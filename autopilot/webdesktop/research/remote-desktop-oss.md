# 원격 데스크톱 조사
버전: v0.1 · 2026-11-23 · 조사: 서브에이전트(sonnet) · 메인 재확인(gh api 2026-11-23): rustdesk AGPL-3.0 · Sunshine GPL-3.0 · str0m MIT · webrtc-rs Apache-2.0 · IronRDP Apache-2.0(MIT 겸용) · Safari 26.4 WebTransport 지원(2026-03, https://webkit.org/?p=17862 , https://9to5mac.com/2026/03/24/apple-details-safari-26-4-with-44-new-features-191-bug-fixes-more/)

## 결론
1. 부품: 호스트 에이전트 Rust 직접. 캡처 `windows-capture`(WGC) · `screencapturekit`, 입력 `enigo`, 전송 WebRTC(`str0m` 또는 `webrtc-rs`). 브라우저 뷰어는 WebRTC 기본, WebCodecs+WebTransport 는 2차.
2. iPhone Safari 때문에 WebRTC 가 가장 안전. WebCodecs iOS 26.0+, WebTransport iOS 26.4+.
3. RustDesk(AGPL-3.0)·Sunshine/Moonlight(GPL-3.0)는 코드 복사 금지, 구조만 참고. 재사용 가능: IronRDP·webrtc-rs·str0m·windows-rs·Guacamole. noVNC·Selkies 는 MPL-2.0(파일 단위).
4. **RustDesk 가 이미 거의 같은 제품**(Rust, 4 OS, 자체 서버). 불만의 핵심은 서버·키·중계 설정 → 우리 차별점은 코덱이 아니라 **자동 설정(페어링)**.

## 표 1. 저장소 (gh api 2026-11-23)
| 저장소 | 언어 / 라이선스 | 스타 / 최근 릴리스 | 호스트·뷰어 | 브라우저 뷰어 | 전송·코덱·NAT |
|---|---|---|---|---|---|
| RustDesk | Rust+Flutter / AGPL-3.0 | 130.8k / 1.5.3 (11-09) | 호스트 Win·mac·Linux·Android, iOS 뷰어만 | 웹 v2 프리뷰(Pro 서버), OSS 여부 미확인 | 자체 프로토콜 UDP/TCP, VP8/VP9/AV1/H264/H265, HW 인코딩. 홀펀칭 실패 시 중계(hbbs 랑데부 · hbbr 중계) |
| RustDesk server | Rust / AGPL-3.0 | 10.8k / 1.1.16 (07-20) | — | — | 포트 21115~21119 |
| Sunshine | C++ / GPL-3.0 | 43.8k / v2026.914 | Win·Linux·mac(추정) | moonlight-web-stream(GPL) | GameStream 호환 RTP/UDP, H264/HEVC/AV1, 중계 없음 |
| Moonlight-qt | C++ / GPL-3.0 | 19.7k / v6.2.0 (10-04) | 뷰어 | — | 위와 같음 |
| Apache Guacamole | Java+C / Apache-2.0 | 4.05k / 1.6.0 | RDP·VNC·SSH 서버 | 있음(canvas) | guacd 가 RDP/VNC → WebSocket. 저지연 아님 |
| noVNC | JS / MPL-2.0 | 14.2k / v1.7.0 (04-28) | VNC 서버 | 있음 | RFB over WebSocket |
| MeshCentral | Node.js / Apache-2.0 | 7.7k / 1.3.1 (10-23) | Win·mac·Linux 에이전트 | 있음 | 서버 중계 |
| IronRDP | Rust / MIT OR Apache-2.0 | 3.3k / viewer 0.1.0 (07-10) | 클라이언트+서버 크레이트 | WASM `ironrdp-web` | RDP, RemoteFX, H264(EGFX) 완성도 미확인 |
| Selkies | Py+Rust+TS / MPL-2.0 | 2.6k / 2.2.1 (11-04) | Linux 호스트 | 있음(Safari 포함) | WebSocket 기본, WebRTC 옵션 |

## 표 2. 브라우저 기술
| 기술 | Chrome·Edge | iOS Safari | 출처 |
|---|---|---|---|
| WebRTC | 지원 | 지원 | (추정, URL 재확인 안 함) |
| WebCodecs | 지원 | 16.4~18.7 부분, 26.0+ | https://caniuse.com/webcodecs |
| WebTransport | 지원 | 26.4+ (OS 판이 하한, 제3자) | https://caniuse.com/webtransport |
- 지연 비교 실측은 못 찾음. 인접: Fraunhofer WebTransport 원격 렌더링 약 100ms https://publica.fraunhofer.de/handle/publica/490090 · webrtcHacks 카메라 데모 약 300ms https://webrtchacks.com/webcodecs-webtransport-and-webrtc/

## 표 3. Rust 부품 (crates.io 2026-11-23)
| 용도 | 크레이트 | 상태 |
|---|---|---|
| WebRTC | `webrtc` 0.21.1 (11-07), Apache-2.0 | 활발. GitHub 태그와 버전 체계 어긋남. RustDesk 는 SCTP 문제로 포크 |
| WebRTC | `str0m` 0.26.0 (11-22), MIT | Sans-IO, 활발, 0.x. Firezone 이 사용 |
| WebRTC | `libwebrtc` 0.4.2 (LiveKit), Apache-2.0 | 구글 원본 바인딩, 빌드 무거움, HW 코덱 이점 |
| Win 캡처 | `windows-capture` 2.0.1, MIT | WGC 기반 |
| mac 캡처 | `screencapturekit` 12.0.0 (11-21), Apache-2.0 | 활발 |
| 통합 캡처 | `xcap` 0.9.10 (11-09) | 활발 (`scap` 정체) |
| 인코딩 | `openh264` 0.9.10 · NVENC 크레이트 미성숙 · VideoToolbox `cidre` · Media Foundation 은 windows-rs 직접 | HW 인코딩은 직접 만들어야 함 |
| 입력 | `enigo` 0.6.1, MIT | 유지되나 릴리스 간격 김 |
| 클립보드 | `arboard` 3.7.0 | 활발 |

## 모바일·macOS 제약
- [확인] iOS 는 컨트롤러만, 호스트 불가(RustDesk) — https://rustdesk.com/blog/rustdesk-remote-control-android-ios
- [추정] iOS 화면 내보내기는 ReplayKit 방송 확장뿐, 입력 주입 공개 API 없음 — https://developer.apple.com/documentation/replaykit/rpbroadcastsamplehandler
- [확인] Android 호스트 = MediaProjection(캡처, 동의창 우회 불가) + AccessibilityService(입력). 잠금 화면 입력 불가, 재부팅 후 현장 잠금 해제, 제조사 배터리 관리가 서비스 종료 — 같은 글
- [확인] macOS 15 화면 기록 권한 월간 재확인 — https://www.macrumors.com/2024/08/15/macos-sequoia-screen-recording-app-permissions/
- [확인·제3자] "Remote Desktop Access" 권한 유형은 월간 만료 없음 — https://documentation.n-able.com/takecontrol/userguide/Content/system-requirements/macOS-security-settings.htm . 서드파티가 얻을 수 있는지 미확인
- 미확인: macOS 26 정책. 26.1 에서 일반 실행 파일이 권한 목록에 안 뜨는 문제 → 에이전트는 .app 번들로 — https://developer.apple.com/forums/thread/807898
- [추정] 접근성 권한과 화면 기록 권한은 따로 승인

## 사용자 불만
- RustDesk #15861 (2026-08-14): "a self-hosted `rendezvous_server` is accepted, used for the initial NAT test, then silently replaced by the fastest public server" https://github.com/rustdesk/rustdesk/issues/15861
- RustDesk #15247 (2026-06-11): "Login required for public server rule is ambiguous, not well defined, incomplete, confusing and misinforming" https://github.com/rustdesk/rustdesk/issues/15247
- NixOS 포럼: 중계 서버를 몰라 오류(검색 요약) https://discourse.nixos.org/t/how-to-configure-selfhost-rustdesk-server/65040
- Jump Desktop 포럼(검색 요약): Fluid 가 안 뜨고 VNC 만, Mac→PC Fluid 연결 1~2분, Tahoe 26.1 문제, Tailscale 경로 사용 여부를 알 수 없음
  - https://support.jumpdesktop.com/hc/en-us/community/posts/24075436492941/comments/24083389113997
  - https://support.jumpdesktop.com/hc/en-us/community/posts/41097847743245/comments/41165591342477
  - https://support.jumpdesktop.com/hc/en-us/community/posts/43160868665741-Tailscale

## 불확실
- WebCodecs 가 WebRTC 보다 빠르다는 근거 없음. iOS 26.4 미만은 WebTransport 없음 → WebRTC 필수.
- iOS 호스트는 범위에서 빼는 것이 현실적.
- 미확인: Sunshine macOS, RustDesk 웹 OSS 여부, macOS 26 권한, Firefox 현황.
