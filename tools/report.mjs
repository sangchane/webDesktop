// 접속 기록(E-09) 판정 — SPEC C7·C8. 운영 웹앱은 Access 로 막혀 있어, 이미 로그인한 Aside 브라우저에서 읽는다.
// 실행: node tools/report.mjs --last   → 마지막 세션 한 줄 + `path=<direct|relay|없음>`
//       node tools/report.mjs --since 2026-11-24T20:00   → 그 뒤 iPhone 세션으로 SC-002·003·006 채점(C8)
//       node tools/report.mjs --uptime DEV   → 에이전트가 끊김 없이 붙어 있은 시간 + 허브 목록 「켜짐」(C9, 이 PC 에서)
import { execFileSync } from 'node:child_process';
import assert from 'node:assert/strict';
import { homedir } from 'node:os';
import { join } from 'node:path';
import { readdirSync, readFileSync } from 'node:fs';

const APP = 'https://webdesktop-app.sangchane87.workers.dev';
const ASIDE = join(homedir(), '.aside', 'cli', 'aside.exe');
const MARK = 'REPORT_JSON:';

function api(path) {
  const code = `const page = await openTab('${APP}/');
const r = await page.evaluate(async () => {
  const x = await fetch('${path}', { redirect: 'manual' });
  return { status: x.status, body: x.status === 200 ? await x.json() : null };
});
console.log('${MARK}' + JSON.stringify(r));
await page.close();`;
  const out = execFileSync(ASIDE, ['repl', code], { encoding: 'utf8', timeout: 120_000 });
  const line = out.split('\n').find((l) => l.startsWith(MARK));
  if (!line) throw new Error(`Aside 응답에 결과가 없습니다:\n${out}`);
  const r = JSON.parse(line.slice(MARK.length));
  if (r.status !== 200) throw new Error(`${path} 를 못 읽었습니다(HTTP ${r.status}) — Aside 브라우저로 웹앱에 로그인하세요.`);
  return r.body;
}
const accessLog = () => api('/api/access-log');

// 에이전트 로그(줄마다 JSON)에서 마지막 agent.connected 시각 — 그 뒤 끊김(agent.disconnected)이나 재시작(새 connected)이 없으면 그때부터 계속 붙어 있다.
function connectedSince(lines) {
  let since = null;
  for (const l of lines) {
    const e = l.startsWith('{') ? JSON.parse(l) : null;
    if (e?.fields?.event === 'agent.connected') since = e.timestamp;
    if (e?.fields?.event === 'agent.disconnected') since = null;
  }
  return since;
}

const v = (x, unit = '') => (x == null ? '없음' : `${x}${unit}`);

// 03-prd.md 상수 표.
const CONNECT_MAX_SEC = 5;
const LATENCY_P50_MS = 150;
const SC006_TEXT = '안녕하세요 원격 테스트';

const median = (xs) => {
  const a = [...xs].sort((x, y) => x - y);
  return a.length ? (a.length % 2 ? a[(a.length - 1) / 2] : (a[a.length / 2 - 1] + a[a.length / 2]) / 2) : null;
};
assert.equal(median([300, 100, 200]), 200);
assert.equal(median([100, 200]), 150);
assert.equal(median([]), null);

// 채점 — [이름, 통과, 설명].
function grade(items, clip) {
  // 첫 화면이 안 뜬 세션(null)도 1회 — 실패로 센다.
  const ff = items.map((x) => x.click_to_first_frame_ms);
  const fast = ff.filter((x) => x != null && x <= CONNECT_MAX_SEC * 1000).length;
  const lat = items.map((x) => x.latency_p50_ms).filter((x) => x != null);
  const p50 = median(lat);
  return [
    ['SC-002', ff.length >= 10 && fast / ff.length >= 0.9,
      `첫 화면 ${CONNECT_MAX_SEC}초 이하 ${fast}/${ff.length}회 (10회 중 9회 필요) — [${ff.map((x) => v(x)).join(', ')}]ms`],
    ['SC-003', p50 != null && p50 <= LATENCY_P50_MS,
      `입력 지연 p50 ${v(p50, "ms")} (기준 ${LATENCY_P50_MS}ms 이하) — 세션별 [${lat.join(', ')}]ms`],
    ['SC-006', clip === SC006_TEXT, `호스트 클립보드 「${clip}」 (기준 「${SC006_TEXT}」)`],
  ];
}
{
  const ok = (ms, p) => ({ click_to_first_frame_ms: ms, latency_p50_ms: p });
  const g = grade([...Array(9)].map(() => ok(900, 120)).concat(ok(null, null)), SC006_TEXT);
  assert.deepEqual(g.map((x) => x[1]), [true, true, true]);
  assert.equal(grade([...Array(8)].map(() => ok(900, 120)).concat(ok(null, null), ok(6000, 100)), '')[0][1], false);
  assert.deepEqual(grade([ok(900, 200)], '안녕').map((x) => x[1]), [false, false, false]);
}

{
  const c = (event, timestamp) => JSON.stringify({ timestamp, fields: { event } });
  assert.equal(connectedSince([c('agent.connected', 'a'), c('session.start', 'b')]), 'a');
  assert.equal(connectedSince([c('agent.connected', 'a'), c('agent.disconnected', 'b')]), null);
  assert.equal(connectedSince([c('agent.connected', 'a'), c('agent.disconnected', 'b'), c('agent.connected', 'c'), '']), 'c');
}

const args = process.argv.slice(2);
const up = args.indexOf('--uptime');
if (up >= 0) {
  const name = args[up + 1];
  if (!name) {
    console.log('사용: node tools/report.mjs --uptime <기기 이름>');
    process.exit(2);
  }
  // resident.rs Home::from_env 와 같은 곳. 로그는 날짜순 파일 이름(agent.YYYY-MM-DD.log).
  const dir = join(process.env.WEBDESKTOP_HOME ?? join(process.env.ProgramData ?? 'C:\\ProgramData', 'WebDesktop'), 'logs');
  const lines = readdirSync(dir).filter((f) => f.startsWith('agent.')).sort()
    .flatMap((f) => readFileSync(join(dir, f), 'utf8').split('\n'));
  const since = connectedSince(lines);
  const dev = api('/api/devices').find((d) => d.name === name);
  const hours = since ? (Date.now() - new Date(since)) / 3_600_000 : 0;
  console.log(`허브 목록 「${name}」 ${dev ? (dev.online ? '켜짐' : '꺼짐') : '없음'} · 에이전트 연결 유지 ${hours.toFixed(1)}시간 (${since ?? '연결 기록 없음'}부터)`);
  const ok = dev?.online && hours >= 24;
  console.log(ok ? 'UPTIME OK' : 'UPTIME 아직 — 24시간 필요');
  process.exit(ok ? 0 : 1);
}
const at = args.indexOf('--since');
if (at >= 0) {
  const since = new Date(args[at + 1] ?? '');
  if (isNaN(since)) {
    console.log('사용: node tools/report.mjs --since 2026-11-24T20:00 (이 PC 시각)');
    process.exit(2);
  }
  // shortcut: 첫 쪽(ACCESS_LOG_PAGE=50)만 본다 — 시험 10회에는 충분, 하루치를 볼 때 next 를 따라가게.
  const items = accessLog().items.filter((x) => new Date(x.started_at) >= since && /iPhone/.test(x.viewer_ua ?? ''));
  const clip = execFileSync('powershell', ['-NoProfile', '-Command',
    '[Console]::OutputEncoding=[Text.Encoding]::UTF8; Get-Clipboard -Raw'], { encoding: 'utf8' }).trim();
  console.log(`${since.toISOString()} 뒤 iPhone 세션 ${items.length}개`);
  for (const x of items) console.log(`- ${x.started_at} · 첫 화면 ${v(x.click_to_first_frame_ms, 'ms')} · p50 ${v(x.latency_p50_ms, 'ms')} · path=${v(x.path)}`);
  const g = grade(items, clip);
  for (const [id, pass, why] of g) console.log(`${pass ? '맞음' : '틀림'} ${id} ${why}`);
  const n = g.filter((x) => x[1]).length;
  console.log(`정답지 ${n}/${g.length}`);
  process.exit(n === g.length ? 0 : 1);
}
if (!args.includes('--last')) {
  console.log('사용: node tools/report.mjs --last | --since <시각> | --uptime <기기 이름>');
  process.exit(2);
}
const last = accessLog().items[0];
if (!last) {
  console.log('세션 없음');
  process.exit(1);
}
console.log(`세션 ${last.session_id} · 시작 ${last.started_at} · 끝 ${v(last.ended_at)} (${v(last.end_reason)})`);
console.log(`뷰어 ${v(last.viewer_ua)}`);
console.log(`첫 화면 ${v(last.click_to_first_frame_ms, 'ms')} · 입력 지연 p50 ${v(last.latency_p50_ms, 'ms')} · 재연결 ${last.reconnect_count}회`);
console.log(`path=${v(last.path)}`);
process.exit(last.path ? 0 : 1);
