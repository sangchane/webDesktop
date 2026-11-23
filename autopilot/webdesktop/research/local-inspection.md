# 이 PC 에 설치된 Jump Desktop · Tailscale 관찰 기록
버전: v0.1 · 2026-11-23 · 관찰자: Claude (메인)

## 관문 판정 (역공학 범위)
- Jump Desktop Connect `eula.rtf` 「DESCRIPTION OF OTHER RIGHTS AND LIMITATIONS」: "(a) copy, modify, create a derivative work of, reverse engineer, decompile or otherwise attempt to extract the source code …" — **금지**.
  → 디컴파일·코드 풀이 하지 않음. 파일 목록 · 포함 오픈소스 부품의 글자 · 사용자 본인 로그·설정 · 공개 CLI 도움말까지만 봤다.
  → 우리 제품은 Jump Desktop 과 호환할 필요가 없으므로(새 프로토콜) 저작권법 제101조의4(호환 목적 역분석)도 근거가 되지 않는다.
- Tailscale 클라이언트: BSD-3 소스 공개(github.com/tailscale/tailscale). 역공학 불필요 — 소스를 읽는다. REA 는 포함 모듈 확인만 썼다.

## Jump Desktop (뷰어 9.0.16/9.0.20 · 호스트 Connect 10.15.53, Windows)
| # | 관찰 | 근거 | 설계에 주는 뜻 |
|---|---|---|---|
| J1 | [관찰] 뷰어·호스트가 같은 `jrtc.dll`(약 9MB)을 쓴다. 안에 Google libwebrtc 소스 경로(`p2p/base/basic_ice_controller.cc`, `pc/dtls_srtp_transport.cc`, `net/dcsctp/…`, `p2p/dtls/dtls_stun_piggyback_controller.cc`)와 opus·libvpx·H.264·H.265·AV1 패킷화 코드 | `strings jrtc.dll` | 전송은 **WebRTC(ICE+DTLS-SRTP+SCTP 데이터 채널)**. 브라우저 뷰어와 같은 기술이다 |
| J2 | [관찰] 호스트에 `aom.dll`(AV1 인코더), 뷰어에 `dav1d.dll`(AV1 디코더), `libGLESv2.dll` | 파일 목록 | AV1 지원. 하드웨어 인코딩이 없을 때 소프트웨어 AV1 |
| J3 | [관찰] 신호(시그널링) 서버 `neuron.jumpdesktop.com:443` 에 상시 접속. 접속 직후 "Got ice servers". 59초마다 `>>`, 2분마다 `<<` 유지 신호. 끊기면 20초 뒤 재접속 | `Viewer_2026_10_02.log`, `Viewer_2026_05_06.log` | 모든 기기가 신호 서버에 상시 접속 → 기기 목록·온라인 상태·접속 요청이 여기서 온다. 중계(TURN) 주소는 서버가 내려준다 |
| J4 | [관찰] 저장된 접속 파일(.jump, JSON) 항목: `NeuronId`, `SupportsRtcV1/V2`, `TcpPort 35384`, `MacAddresses`(원격 전원 켜기용), `UseVirtualMonitors`, `UsbDeviceState`, `UseStylusRemoting`, `ColorQuality 420_8bit`, `HardwareAccelerationPreference`, `MaxFrameRate`, RDP·VNC·ARD 항목, `OsTypeCode 2 / OsVersion 26.5`(상대는 macOS 26) | `Client/Servers/*.jump` (비밀 값 가림) | 기능 범위의 실제 목록. 직접 TCP 포트를 따로 두고, 신호 서버 ID 로도 찾는다 |
| J5 | [관찰] 호스트 드라이버 4개: 가상 모니터(`JumpVirtualMonitorDriver`, UMDF, Class=Display → IddCx 계열), 가상 마우스(`jumpvms.sys`, HIDClass, KMDF), 가상 오디오(`jumpaudio.sys`), USB 넘기기(`JumpUsbUde.sys`, UDE). `sas.dll`(Ctrl+Alt+Del) | `drivers/*/*.inf` | 고급 기능(가상 모니터·USB·로그인 화면 조작)은 **서명된 커널/UMDF 드라이버**가 필요하다 → 1차 범위에서 빼는 후보 |
| J6 | [관찰] `JumpConnectCli --cloudlessurl`(클라우드 없이 여는 주소), `--connectcode <code>`(연결 코드로 등록) | CLI 도움말 | 쉬운 설정 = **짧은 코드 하나로 기기 등록**. 클라우드 없이 직접 여는 길도 있다 |
| J7 | [관찰] 호스트 로그에 "Interactive auth request cancelled" 5회(2026-06-29) | `Connect/Logs/Agent_2026_06_29.log` | 접속 때 호스트 쪽 승인 창이 뜨고, 사용자가 놓친 흔적. 승인 흐름의 마찰 근거 |
| J8 | [관찰] UI 는 Qt5, 오류 보고 sentry | 파일 목록 | — |

## Tailscale (1.96.3, Windows)
| # | 관찰 | 근거 | 설계에 주는 뜻 |
|---|---|---|---|
| T1 | [관찰] `tailscaled.exe` Go 1.26.1, 모듈 43개: `tailscale/wireguard-go`, `golang.zx2c4.com/wintun`, `gvisor.dev/gvisor`(사용자 공간 TCP/IP), `huin/goupnp`, `kortschak/wol`, `google/go-tpm`, `coder/websocket`, `tailscale/wf`(Windows 방화벽), `tailscale/web-client-prebuilt`(로컬 웹 화면) | REA `inspect_go_binary` 증거 `ev_b92a8bd8…` | 같은 일을 Rust 로: WireGuard=boringtun, 가상 NIC=wintun(공통), 네트워크 스택=smoltcp 등 — 조사 결과와 대조 |
| T2 | [관찰] `tailscale netcheck`: UDP 가능, `MappingVariesByDestIP: false`(직접 연결 쉬운 NAT), 포트 열기 UPnP 가능, 공유기 UPnP 응답 3곳(이중 공유기) | 2026-11-23 10:18 실행 | 이 집 망은 직접 연결이 잘 된다. 이중 공유기는 흔하다 → 설계가 견뎌야 함 |
| T3 | [관찰] 가장 가까운 Tailscale 중계(DERP)는 **도쿄 39.5ms**, 홍콩 58.2ms. 한국 중계 없음 | 같은 실행 | 직접 연결 실패 시 원격 화면이 도쿄를 돌아온다. **한국에 자체 중계를 두면** 중계 지연을 줄일 수 있다(측정 필요) |
| T4 | [관찰] `tailscale status`: BackendState `NoState`, 연결된 기기 0 | 같은 날 | 사용자가 지금 Tailscale 을 쓰지 않고 있음(원문 「안 쓰게 된다」와 일치) |
