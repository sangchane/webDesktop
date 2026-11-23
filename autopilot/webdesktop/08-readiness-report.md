# 착수 준비 보고서 — WebDesktop
버전: v1.0 · 기준 03 v1.2 · 2026-11-23

## 판정: CONCERNS (착수 가능)
- GATE 1차(새 맥락 검토관 dev:reviewer): **FAIL** — CRITICAL 2(SC-002·005 잼 수단 없음, FR-018 계약 공백), HIGH 9. 단계 05·06 재작성 + 01~04·07 패치 1회.
- 재검토 1회(상한): **CONCERNS** — 1차 23건 중 닫힘 18·부분 5·안 닫힘 0. 패치가 새로 만든 HIGH 2(N1 설치 파일에 로그인 필요, N2 계측 값 저장 칸 없음)·MEDIUM 3.
- 재검토 뒤 국소 수정(DL-9): N1·N2·N3·N4·N5·N6 과 부분 5건을 05 필드·03 상수·04 문단 수준으로 고침. 재검토 상한 때문에 **이 수정은 검토관이 다시 보지 않았다** → 아래 선행 조건 1.
- `check_package.py` 최종: CRITICAL 0 · HIGH 0 · FR 22(P0 10·P1 8·P2 4) 중 P0·P1 18 전부 05 매핑 · SC 10 전부 06 시나리오 · register 67행 전부 마킹 · 근거 미확인 상수 0 · 검증 필요 23건(전부 확인 방법 있음).

## 남은 지적 (MEDIUM 이하 — 구현 중 처리)
| # | 내용 | 처리 시점 |
|---|---|---|
| R-1 | DL-9 국소 수정이 새 불일치를 만들었을 수 있음(재검토 없음) | 첫 작업 SPEC 쓸 때 05·06 을 한 번 대조 |
| R-2 | C9: 04~06 에 상수 표 밖 숫자 26줄(대부분 근거 인용·예시, 일부 SQL 상한) | SPEC 에서 값을 코드 상수로 옮길 때 03 표와 대조 |
| R-3 | 「검증 필요」 개수가 검토관 grep(22) 과 스크립트(23)에서 1건 차이 | 영향 없음(전부 확인 방법 있음) |

## 착수 조건 (사용자 손이 필요한 것)
1. **Cloudflare 대시보드 설정 2개** — wrangler 로그인 권한에는 Access(Zero Trust)·Realtime TURN 키 범위가 보이지 않는다(2026-11-23 `wrangler whoami`).
   - ① Zero Trust → Access 앱 1개(소유자 이메일만) + 우회 앱 2개(`/agent/*`, `/healthz`). 첫 작업 1 끝에 제가 단계별 안내를 드린다.
   - ② Realtime → TURN 키 1개 만들기 → 키 ID·토큰을 `wrangler secret put` 으로 넣기(값은 사용자가 직접 입력, 제가 보지 않음).
   - 대안: 이 두 권한을 가진 Cloudflare API 토큰을 사용자가 만들어 환경 변수로 주면 제가 설정한다.
2. **Cloudflare 계정 2단계 인증 확인** — 허브를 믿는 구조라 계정 탈취 = 전체 탈취(04 위협 ④-1).
3. **실기기 시험 협조** — iPhone(LTE)·집 Windows PC·MacBook 에서 웹앱을 열어 주는 것. 판정은 제 계측 스크립트가 한다(06 「시험 환경」).
4. 개발 도구: Rust(stable)·Node 22·`worker-build`·`just`·Windows SDK, Mac 빌드는 MacBook 에서(크로스 빌드 안 함).

## 첫 작업 3개 = 워킹 스켈레톤 (07 착수 자산과 같음)
1. **허브 + 웹 + Windows 에이전트 최소 화면** — iPhone Safari LTE 로 열기·클릭·한글·`?relay=1`. 같이 확인: 가정 P1·H1·H2, SC-003 측정법(`requestVideoFrameCallback`), 설치 링크 E-12, 오프라인 판정(자동 응답 시각 API).
2. **Windows 무인 접속 spike** — SYSTEM 서비스 → 콘솔 도우미, 잠금·UAC 데스크톱 전환, `sas`. 같이 확인: 가정 P2·W1.
3. **macOS 최소 화면 + 권한 유지 + 로그인 창** — 같이 확인: 가정 B12·M1, MAC_OS_PROMPTS 실측.

## 검증 필요 23건 → 어느 작업에서 닫나
| 작업 | 항목 |
|---|---|
| 1 | H1(DO 24시간 연결) · H2(Access 경로 우회) · TURN 자격 발급 API 경로 · 자동 응답 시각 API · `requestVideoFrameCallback` rtpTimestamp(Safari) · R6(iPhone 한글) · R20(TURN 한국 거점 RTT) · 한국 3사 직접 연결률(계측 누적) |
| 2 | W1(SendSAS 정책) |
| 3 | B12·R17(자체 서명 업데이트 후 권한 유지) · R14(월간 재승인 30일 관찰) · M1(로그인 창) · MAC_OS_PROMPTS 실측 |
| 2단계 | T1(에이전트 TURN 클라이언트) |
| 운영 | TURN 사용량 API · PITR 복원 연습(Rust API) |
| 범위 밖 기록 | Apple 5.4 · Google Play 접근성 · 부가통신사업 신고 · P3 폰 WireGuard(판매·폰 VPN 을 하게 될 때만) |

## 구현 핸드오프 (dev:build SPEC 입력)
/dev:build 로 다음을 실행:
<inputs>autopilot/webdesktop/03-prd.md (요구사항·상수 표), 05-api-contract.md, 08-readiness-report.md (착수 조건·첫 작업 3개)</inputs>
<references>04-architecture.md, 06-test-design.md, 07-ops-design.md, docs/goals.md — 필요할 때만 읽는다</references>
<first_task>SPEC.md 작성 — 첫 작업 1(허브 + 웹 + Windows 에이전트 최소 화면)을 낯선 구현자 실행 가능 수준(≥7/10)으로. R-1 대조 포함</first_task>
<then>superpowers 설치 시 `superpowers:writing-plans` → 첫 작업 3개(워킹 스켈레톤)부터. brainstorming 은 생략 — 이 프롬프트를 붙여 넣은 것이 설계 승인이다.</then>
UI: BUILD·REVIEW 에서 dev:ui dial = 밀도 중간 · 모션 낮음(원격 화면 위 0) · 폰 우선
<model_hints>
opus: 허브 인증(Access JWT·기기 서명·등록 코드 트랜잭션), Windows SYSTEM 서비스·데스크톱 전환, str0m 세션 루프·재연결 3단계, 2단계 VPN(ICE+boringtun)
sonnet: proto 메시지·TS 생성, 웹 화면 3개·입력 변환기, 계측 집계 스크립트, DO 마이그레이션·색인, 설치 체크리스트
haiku: 오류 문구 표, 로그 사건 이름 상수, 포맷·리네임
</model_hints>
