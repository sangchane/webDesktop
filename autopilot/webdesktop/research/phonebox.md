# phonebox 기능 분석 (2026-11-25)

대상: https://github.com/ersinkoc/phonebox (MIT, 별 6,031, 커밋 ec32140 2026-11-25). 읽기만 함, 실행 안 함.
같은 이름 다른 저장소: pfurpass/phonebox(브라우저 noVNC 로 안드로이드 에뮬레이터 조작, 별 27) — 이번 분석 대상 아님.

## 무엇인가
안드로이드 폰(WebView 앱) + PC 에이전트(.NET 10). 폰을 PC 옆 「상태판·조작판」으로 쓴다. 화면 전송은 없다.
연결: USB(adb reverse + 고정 인증서 TLS) 또는 Wi-Fi(8766 HTTPS, QR 짝짓기). 클라우드·계정 없음.
위젯 133개: PC 상태, AI 사용량·세션, 미디어, 단축키, 자동화 등. 대부분 우리 목표(G1~G3) 밖.

## 우리 제품과 닿는 기능

| phonebox 기능 | 근거 파일 | 우리 쪽 | 판단 |
|---|---|---|---|
| 받아쓰기: 폰 음성 인식 → PC 에 타자 | agent/PhoneBox.Agent/Dictation.cs | 시스템 자판 글자 입력 길(FR-007)이 있음. iPhone 자판 마이크로 되는지 미확인 | 먼저 확인. 안 되면 Web Speech API 단추 |
| 트랙패드 진동(클릭 1번·오른쪽 2번·끌기 시작 강하게) | docs/GUIDE.md 「Trackpad」 | 트랙패드 방식 있음, 진동 없음 | 작음. Android 만(iOS Safari 는 vibrate 없음) |
| Mac 다중 손가락 제스처(3·4손가락 쓸기 → Mission Control·공간 이동) | TrackpadHandler.cs `GestureName` | 없음 | Mac 호스트(첫 작업 3) 때 |
| 원격 깨우기(WoL) | WakeOnLan.cs, GUIDE 「Wake-on-LAN」 | FR-021(P2) 설계 있음 | 설계 보강에 씀(아래) |
| 클립보드 | Clipboard.cs | FR-011(P1) 아직 없음 | 원래 할 일 |
| 사용자 단축키·매크로 단추(if/else 포함) | Shortcuts.cs, MacroFlow.cs | 특수키 막대만 | 후보. 매크로 조건문은 과함 |
| 잠김 감지 | SessionLockMonitor.cs | SC-004 잠긴 화면 | 목록에 「잠김」 표시 후보 |
| 폰 마이크 → PC 마이크 | PhoneMic*.cs (가상 오디오 장치) | FR-019 오디오(P2, 방향 반대) | 뺌 |
| AI 권한 묻기 카드(Claude Code·Codex hook → 폰에서 허용/거부) | ClaudeHook.cs, GUIDE 「AI agent permission prompts」 | 없음 | 목표 밖. 넣으려면 goals.md 변경 |

## 옮길 만한 안전 장치
- 받아쓰기: 제어 문자를 공백으로 바꾼다. Enter 는 옵션일 때만. 최대 1000자. 앞 창이 관리자 권한이면(UIPI) 조용히 실패하지 말고 이유를 돌려준다. 받아쓴 글은 로그에 안 남긴다.
- 트랙패드: 이동 한 번 최대 500, 스크롤 최대 1000 으로 자른다. 마우스 조작은 기본 꺼짐(위젯 설정에서 켬).
- WoL: 매직 패킷은 개인 LAN 어댑터 전부에서 3번(100ms 간격) 보낸다. 결과는 「보냄」이지 「켜짐」이 아니다. 폰은 짝지은 망에 있을 때만 보낸다(카페 망 금지). 깨운 뒤 에이전트 응답을 최대 3분 기다린다. 유선 권장, Windows 「빠른 시작」 끄기 안내.
- 짝짓기 코드: 128비트, 5분, 1회용, 틀림 10번이면 폐기, 새 코드가 옛 코드를 폐기. 우리 FR-002 와 같은 모양.

## 라이선스
MIT 라 복사 금지 대상은 아니다. 다만 C#·JS 라 Rust 로 다시 쓴다.
