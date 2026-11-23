# 폰·태블릿으로 PC 조작 — 조작감 조사 (2026-11-23)

질문: 390px 폰·1024px 태블릿에서 1920×1080 PC 를 조작할 때 무엇이 불편하고, 다른 제품은 어떻게 푸나.

## 문제의 크기 (계산)
| 뷰어(세로, CSS px) | 1920 폭 PC 를 다 보이게 할 때 배율 | 9pt 글자(12 PC px)의 화면 크기 | 16px 닫기 단추 |
|---|---|---|---|
| iPhone 390 | 0.20 | 2.4px — 못 읽음 | 3px — 못 누름 |
| iPhone 가로 844 | 0.44 | 5.3px | 7px |
| iPad 1024 | 0.53 | 6.4px | 8.5px |
- 손가락 표적 하한은 44pt(Apple HIG). 그래서 폰은 **확대 없이는 쓸 수 없다**. 1:1 로 보려면 폰 세로 4.9배, 태블릿 1.9배.
- 손가락이 3px 흔들리면 폰 배율 0.2 에서 PC 15px 이다. Windows 더블클릭 허용 범위는 4px(SM_CXDOUBLECLK) → **두 번 탭이 더블클릭이 안 된다**. 두 번째 탭을 첫 탭 자리에 붙여야 한다.

## 다른 제품
| 제품 | 조작 방식 | 확대·이동 | 키보드 |
|---|---|---|---|
| 마이크로소프트 원격 데스크톱(iOS) | 「직접 터치」(누른 곳 클릭, 길게 = 오른쪽) / 「마우스 포인터」(손가락 = 커서 이동, 두 손가락 탭 = 오른쪽, 두 번 탭 후 끌기 = 끌기, 두 손가락 끌기 = 휠) | 마우스 포인터 모드에서 집기 확대, **커서를 가장자리로 밀면 화면이 따라감**. 직접 터치는 집기 불가 → 확대 막대 | 시스템 자판 + Cmd→Ctrl 단축키 바꿈 — https://learn.microsoft.com/en-us/azure/virtual-desktop/users/client-features-ios-ipados , https://techcommunity.microsoft.com/t5/security-compliance-and-identity/gesture-list-for-the-touch-and-mouse-pointer-input-modes-for-the/ba-p/248596 |
| Chrome 원격 데스크톱 | 「마우스」(누른 곳) / 「트랙패드」(밀어서 커서) — 폰은 트랙패드가 낫다는 평 | 「화면에 맞게 크기 바꾸기」 = **호스트 해상도를 폰에 맞춤** | 시스템 자판 + 단축키 보내기 메뉴 — https://www.techsolutions.support.com/how-to/how-to-use-chrome-remote-desktop-12717 |
| TeamViewer(iOS) | 터치 모드 / 마우스 모드(기본 = 마우스) | — | — https://www.teamviewer.com/ams/global/support/knowledge-base/teamviewer-classic/mobile/ios/interaction-methods-on-ios |
| RustDesk | 터치 모드는 기기별 설정(1.4.3), 상대 마우스(1.4.5) | 1.5.0 「캔버스 잠금」 — 두 손가락이 확대·이동을 하다 보니 잘못 움직여 잠금을 더함 | **한 번만 걸리는 Ctrl·Alt 줄**(1.5.0), 시스템 자판 위를 막대가 가린다는 불만(#7183), IME 불만(#8071) — https://ubuntuhandbook.org/index.php/2026/09/rustdesk-1-5-0-is-out-with-screen-zoom-and-pan-lock-for-mobile/ , https://github.com/rustdesk/rustdesk/discussions/7183 |
| Jump Desktop | 브라우저판: 탭 = 클릭, 두 번 탭 = 더블클릭, 길게 = 오른쪽. 직접 터치에서 집기 확대 미지원(지원 답변) | — | — (공식 문서 2쪽은 403 으로 못 읽음 — 검색 요약만) |

## 공통 결론 → 우리 설계
1. **두 가지 조작 방식**이 업계 표준이다: 트랙패드(정밀, 폰) / 직접 터치(빠름, 태블릿). 사용자가 바꾸고 기억한다.
2. **집기 확대는 필수**(폰). 트랙패드 방식에서는 **커서가 가장자리에 닿으면 화면이 따라간다**(MS). 손가락 두 개가 「스크롤」과 「확대」를 같이 하면 잘못 움직인다(RustDesk 잠금) → 손짓 처음에 둘 중 하나로 정하고 끝까지 유지한다.
3. **해상도를 맞추는 근본 해법**은 호스트 해상도를 바꾸는 것(Chrome RD)이다. 에이전트 일이라 지금은 못 한다 → 에이전트 작업(T9 뒤)으로 미룬다.
4. 시스템 자판의 불편: Esc·F키·방향키·Ctrl 이 없고, 화면을 가리고, 한글 조합이 브라우저마다 다르게 온다(우리 DL-16 ②). → **자체 가상 키보드**: 단축키용 한 번만 걸리는 수정키(RustDesk 1.5.0 과 같은 방식, 우리 막대가 이미 이렇게 함), F1~F12·방향·Home/End, 한글 두벌식을 웹앱이 직접 조합(호스트 IME 상태와 무관 — 글자를 유니코드로 보냄, DL-16 과 같은 길).

## 쓴 값의 출처
- 길게 누르기 0.5초: iOS `UILongPressGestureRecognizer.minimumPressDuration` 기본값 0.5초(Apple 문서).
- 태블릿 경계 600px: Android 「sw600dp」 태블릿 자원 기준(Android 개발자 문서 「Support different screen sizes」).
- 손가락 표적 44pt: Apple Human Interface Guidelines 「Layout」.
- iOS Safari 는 `navigator.vibrate` 가 없다 → 눌림 표시는 화면으로만.
