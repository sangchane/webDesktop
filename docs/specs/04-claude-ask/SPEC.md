# SPEC 04 — Claude Code 권한 요청을 폰에서 허용·거부 (M)

목표: goals.md 목표 4 · SC-012 · FR-023 · DL-24

## FRAME
- 사용자: PC 에서 Claude Code 를 돌려 두고 자리를 비운 본인. 폰에 웹앱이 열려 있다.
- 문제: Claude Code 가 권한을 물으면 PC 앞에 갈 때까지 멈춘다.
- 안 할 것: 웹 푸시(웹앱을 닫은 폰 알림), Codex·Cursor·OpenCode, 「항상 허용」(제안 규칙 저장), 명령 고치기(updatedInput).

## 흐름
1. `~/.claude/settings.json` 의 `hooks.PermissionRequest` 에 `curl -s -m 115 -H "x-webdesktop: 1" --data-binary @- http://127.0.0.1:47913/ask` (timeout ASK_HOOK_TIMEOUT_SEC). `webdesktop-agent --claude-hook` 이 넣는다(다른 훅 보존, `settings.json.webdesktop.bak`).
2. 에이전트 로컬 입구(127.0.0.1:ASK_PORT, 시험은 `WEBDESKTOP_ASK_PORT`): `POST /ask` 만, 머리글 `x-webdesktop: 1` 필수, `Origin` 있으면 403, 본문 ≤ ASK_BODY_MAX. 훅 입력에서 project = cwd 끝 폴더, tool = tool_name, detail = Bash 면 command, 파일 도구면 file_path, 그 밖은 tool_input JSON — ASK_DETAIL_MAX 로 자름.
3. `ask.new {ask_id, device_id, project, tool, detail, truncated, wait_sec}` 에이전트→허브. 허브는 device_id 를 인증된 값으로 덮고 ready 뷰어 전부에게. 산 뷰어(마지막 ping, 없으면 연 시각이 OFFLINE_DETECT_SEC 안)가 없으면 에이전트에게 `ask.done`. 에이전트가 허브와 끊긴 동안 온 요청은 곧바로 결정 없음.
4. 웹앱은 화면(목록·원격) 아래에 카드: 「PC 이름 · 폴더」, 도구, 명령(고정폭, 스크롤), 남은 초, [거부] [허용]. 누르면 `ask.answer {ask_id, device_id, allow}`. 카드가 바뀐 뒤 ASK_ARM_MS 동안 단추 잠금. 명령이 ASK_DETAIL_MAX 로 잘렸으면(truncated) 허용 막음 — 에이전트도 그런 allow 는 결정 없음으로. 보이지 않거나 글 방향을 바꾸는 글자는 「⟨U+202E⟩」로 드러낸다. 보일 칸은 도구 이름으로(Bash=command, 파일 도구=file_path, WebFetch=url, 그 밖=입력 전체).
5. 허브는 answer 를 그 기기 에이전트로. 에이전트는 훅 출력을 돌려주고 `ask.done` 을 허브→모든 뷰어로(다른 폰의 카드도 사라짐).
   - 허용: `{"hookSpecificOutput":{"hookEventName":"PermissionRequest","decision":{"behavior":"allow"}}}`
   - 거부: `…"decision":{"behavior":"deny","message":"폰에서 거부했습니다"}}}`
   - 시간 초과·뷰어 없음·ASK_PENDING_MAX 초과: 빈 본문(결정 없음 → 터미널 창).
6. 뷰어가 새로 붙을 때와 에이전트 인증이 끝났을 때 허브가 `ask.sync` → 에이전트가 기다리는 `ask.new` 를 남은 초로 다시 보낸다. curl 이 먼저 끊기면(Esc) 카드를 거둔다.

## 코드 위치
- `crates/proto` — WsMsg 4개(`ask.new`·`ask.answer`·`ask.done`·`ask.sync`)와 꼴 검사, 상수.
- `crates/hub/src/entry.rs` — 중계만(저장 없음). 로그엔 ask_id·tool 만, detail 금지.
- `crates/agent/src/ask.rs`(새, resident 계층) — 로컬 입구, HTTP 한 요청 읽기, 훅 입력→카드, 결정→훅 출력, 기다리는 요청 장부. 순수 함수 단위 시험. `resident.rs` 는 select 에 배선만. `main.rs` 는 `--claude-hook`.
- `crates/web/src/ask.rs`(새) — 카드 화면. `main.rs` 는 상태·배선만.
- 바닥선: 로컬 입구는 신뢰 경계(꼴·크기·머리글 검사), 로그에 명령 원문 금지, 실패는 「결정 없음」으로(데이터 손실 없음 — Claude 가 터미널에서 다시 묻는다).

## 완료 기준
1. `cargo test` — 훅 입력→카드(Bash·Edit·MCP), 결정→훅 출력이 공식 문서 예시 꼴과 글자 그대로 같음, HTTP 요청 읽기(머리글 없음·Origin·큰 본문 거절), 훅 설치가 다른 훅을 보존·두 번 넣지 않음, proto 꼴 검사.
2. `node --test tools/agent.test.mjs` — 실제 에이전트 + 로컬 허브 + 시험 브라우저: 뷰어 없을 때 curl 이 1초 안에 빈 응답, 카드 허용 → allow 출력, 거부 → deny 출력, 늦게 붙은 뷰어도 카드를 받음(ask.sync).
3. 정답지: 실제 `claude` 의 권한 요청 1회를 이 PC 에서 폰 카드로 허용해 명령이 실행됨을 본다.
