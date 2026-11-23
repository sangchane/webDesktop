# decision-log — WebDesktop

## 2026-11-23
- [A0] 강도 full — 원격 제어·네트워크 접근(보안 피해 큼), 4개 OS, 외부 시스템 2개 이상. 해석 5개는 00-seed 「해석한 것」.
- [A1] 역공학 범위 결정: Jump Desktop 은 EULA 가 역공학·디컴파일 금지 → 파일 목록·포함 오픈소스 글자·본인 로그/설정·공개 CLI 도움말까지만. Tailscale 은 BSD-3 소스 공개 → REA `inspect_go_binary` 로 모듈 목록만. 대안(디컴파일)을 버린 이유: 약관 위반 + 저작권법 제101조의4 호환 목적 아님 + 소스·공개 자료로 충분.
- [A1] 스킬 사용: `anthropic-skills:deep-research` 대신 조사 서브에이전트 3개(sonnet, 병렬 — model-routing A1 조사 등급). `rea` 1회(Tailscale 모듈 목록). ecc 꺼짐 → 대체 경로(WebSearch/WebFetch + gh api).
- [A1] 메인 재확인: iroh(Apache-2.0, 1.0=2026-06-15, 1.5.0=2026-11-19), rustdesk AGPL-3.0, Sunshine GPL-3.0, str0m MIT, webrtc-rs Apache-2.0, IronRDP Apache-2.0, Safari 26.4 WebTransport, Apple 지침 5.4(조직 계정).
- [A1] 교차 확인: Jump 백서 "built on WebRTC" ↔ jrtc.dll 안 libwebrtc 소스 경로(J1) 일치.
- 비용(A1까지): 검색 메인 2회 + 서브에이전트 3개(도구 호출 126회, 약 26만 토큰), 소요 약 30분.
- [A2] DL-1 사용자 답: 혼자 사용 · iPhone 은 웹앱(App Store 앱 없음). → 네이티브 코드는 Win·Mac 호스트 에이전트만. 버린 대안: iOS NetworkExtension 앱(조직 계정·심사).
- [A2] DL-2 VPN = 호스트끼리 메시(2단계). 폰 VPN·출구 노드 범위 밖(사용자 Q1).
- [A2] DL-3 서버 = Cloudflare 만(Workers+Durable Objects 신호, Realtime TURN 중계, 정적 웹앱, Access 로그인). 버린 대안: 서울 VM 월 5달러(관리 부담·비용), 오라클 무료(자리 없음·한도 축소 보고), 집 PC(이중 공유기·꺼지면 전체 불가). 바꿀 때 비용: 신호 Worker 를 axum 서버로 옮기기 + TURN 을 coturn 으로 — 며칠.
- [A2] DL-4 로그인 = Cloudflare Access 이메일 일회용 PIN, 정책은 소유자 이메일 1개. 버린 대안: 직접 만든 패스키(코드·복구 설계 필요, 빌더 체크 B9 를 플랫폼에 넘김). 호스트 에이전트는 Access 를 거치지 않고 기기 키(Ed25519) 서명으로 인증.
- [A2] DL-5 화면 전송·호스트 간 VPN 모두 ICE(str0m) + Cloudflare TURN 한 길로 — 연결 기술 1벌. VPN 패킷 운반 방식은 A4.
- [A2] 빌더 체크: dev:reviewer(opus 상속) 1회, 15건 → 이미 답 4 · 사소함 10 · 사용자 몫 1(+Android).
- [A2] DL-6 사용자 지시(2026-11-23 「search-real-estate에 클라우드 플레어 월 5$ 요금제 활성화 해둔 접속 정보 있으니까 그거 사용해서 하자」「유료 플랜인 상태야」): 기존 Cloudflare 계정(Workers Paid, 2026-11-16부터 — search-real-estate AGENTS.md:54) 사용. 인증은 wrangler OAuth 로그인(이 PC 사용자 설정 `%APPDATA%\xdg.config\.wrangler\config\default.toml`, 값은 문서에 안 씀). workers.dev 하위 도메인은 그 계정 것. 영향: Durable Objects 상시 WebSocket 시간 과금 걱정이 무료 한도 → 유료 포함량으로 바뀜. 주의: 같은 계정을 listing 앱과 나눠 씀 → Worker 이름·D1·DO 이름에 `webdesktop-` 접두로 섞이지 않게.
- [A3] DL-7 상수 값(03 상수 표) 중 출처가 없는 것은 내 설계 결정: 설정 행동 3(설치·기기 추가·코드 — 비교 제품 5~9보다 적게), 접속 5초·재연결 5초(실측 전 목표, 스켈레톤에서 재조정), 등록 코드 8자리·10분·5회(대입 공격 막기 + 손으로 옮겨 적기), Access 30일·기록 90일, 긴 변 2560px·H.264 5.1(5K 는 레벨 한계 — B13), 절약 1Mbps, 유지 신호 30초(Jump 59초보다 짧게 — 온라인 표시 빠르게), TURN 자격 60분, VPN 10.233.0.0/16(Tailscale 대역과 안 겹침), 빈도 60회/분, WS 메시지 64KB.
- [A3] 목표 수립: docs/goals.md(규칙 11). 목표별 조사는 A1 조사 3건 + 이번 Cloudflare 확인으로 대신함(새 검색 없음).
- [A4~A7] 스킬 사용: 단계별 1순위 ecc 스킬은 꺼져 있음 → 대체(stage-templates·engineering-baseline). 진입 사전조사는 기존 조사·이번 확인으로 대신함.
- [GATE 1차] dev:reviewer(opus 상속) 판정 FAIL — CRITICAL 2(SC-002·005 잼 수단 없음, FR-018 계약 공백), HIGH 9, MEDIUM 7, LOW 5. 타당성 필터: 23건 중 23건 반영(#17 YAGNI 는 일부 — R2 덤프·시험 허브·배너 2종 제거, PITR 로 대체).
- [GATE 1차] DL-8 패치 결정: ① 첫 화면·재연결 시간은 뷰어가 재서 M-10 으로(에이전트는 뷰어 시각을 모름) ② SC-003 은 D-08 `input.applied` 의 RTP 타임스탬프를 `requestVideoFrameCallback` 과 짝지어 뷰어 시계 하나로 ③ 에이전트 TURN 자격 M-11, VPN 경로 M-12, ICE ufrag/pwd 를 M-08 에 ④ str0m 은 TURN 할당 안 함(README 확인) → TURN 클라이언트 필요(위험 T1) ⑤ 활성 등록 코드 하나 + 시도 수는 그 코드에 ⑥ 업데이트 = R2 + latest json + 1회용 토큰, 관리 API 없음 ⑦ 백업 = DO PITR, 시험 허브 없음(로컬 wrangler dev) ⑧ Mac OS 확인 창 4(Gatekeeper·화면 기록·손쉬운 사용·로컬 네트워크), Windows 2(SmartScreen·UAC), 행동과 따로 셈 ⑨ LATENCY_P50_MS 근거를 ITU G.114(음성)에서 설계 결정으로 ⑩ Windows 기기 키 = DPAPI + 파일 ACL SYSTEM 전용 ⑪ 유지 신호는 DO 자동 응답 ⑫ 재연결 3단계 순서 통일 ⑬ 새 상수 OFFLINE_WARN_HOURS·SLO_SUCCESS_PCT·TURN_WARN_PCT·AGENT_LOG_FILES·UPDATE_TOKEN_TTL_MIN·PITR_DAYS.
- [GATE 1차] 메인 확인: workers-rs v0.9.0(2026-11-11) DO 휴면·SQL·자동 응답 API 존재(`worker/src/durable.rs:370,405,695,1066`), str0m README TURN 절, Firezone snownet boringtun 사용.
- [GATE 재검토] 같은 검토관 재검토 1회 — 판정 CONCERNS. 1차 23건: 닫힘 18·부분 5·안 닫힘 0. 새 지적 N1·N2(HIGH), N3~N5(MEDIUM), N6(LOW).
- [GATE 재검토] DL-9 국소 수정(재검토 없이, 단계 재실행 아님): N1 설치 파일은 E-12 `/agent/install/{os}?t=` 로그인 없는 링크(INSTALL_LINK_TTL_MIN, 활성 코드와 같이 폐기) / N2 session 표에 M-10 값 칸(click_to_first_frame_ms·latency_p50_ms·reconnect_ms_json·restarts·new_session_used), 경로 출처는 M-10 하나 / N3 오프라인 = 닫힘 이벤트 또는 OFFLINE_DETECT_SEC, 자동 응답 시각 API 없으면 `agent.alive` / N4 MAC_OS_PROMPTS 5(관리자 암호 추가), 실기기로 확정 / N5 `update_token` 표 / N6 04 시퀀스 viewer_id, goals 순서, 1단계 에이전트 TURN 없음(뷰어 TURN 만), 1차 #2 `vpn_link` 표·E-02 vpn_links, #3 01·02 낡은 줄, #4 FR-009 로컬 네트워크·Remote Desktop 항목 제외 이유, #20 TURN_PRICE_USD_PER_GB.
- 비용(GATE 까지): 서브에이전트 6개(조사 3·빌더 체크 1·GATE 1+재검토 1), 메인 검색 약 12회. 강도 full.
- [BUILD T2] DL-10 Worker 를 둘로 나눔 — `webdesktop-app`(정적 웹앱 + /api/*, Worker Access 로 보호) + `webdesktop-hub`(공개 /agent/*·/ws/*·/healthz + DO Hub). 근거: Workers Access 문서(2026-11-02 수정) 「WebSocket 미지원(403)」, 경로 예외 없음 / 계정 zone 0개(API 확인 2026-11-23) → 호스트 단위 Access 불가. 뷰어 WS 는 E-13 1회용 입장권(60초). 버린 대안: 도메인 구입(연 약 10달러, 사용자 결제 필요), 직접 만든 로그인(코드·복구 부담).
- [BUILD T2] 07 의 `just` 대신 `scripts/verify.ps1`(새 도구 설치 피함).
