// 접속 기록(E-09) 판정 — SPEC C7·C8. 운영 웹앱은 Access 로 막혀 있어, 이미 로그인한 Aside 브라우저에서 읽는다.
// 실행: node tools/report.mjs --last   → 마지막 세션 한 줄 + `path=<direct|relay|없음>`
// shortcut: --since(C8)·--uptime(C9)은 T11·T12 에서 더한다.
import { execFileSync } from 'node:child_process';
import { homedir } from 'node:os';
import { join } from 'node:path';

const APP = 'https://webdesktop-app.sangchane87.workers.dev';
const ASIDE = join(homedir(), '.aside', 'cli', 'aside.exe');
const MARK = 'REPORT_JSON:';

function accessLog() {
  const code = `const page = await openTab('${APP}/');
const r = await page.evaluate(async () => {
  const x = await fetch('/api/access-log', { redirect: 'manual' });
  return { status: x.status, body: x.status === 200 ? await x.json() : null };
});
console.log('${MARK}' + JSON.stringify(r));
await page.close();`;
  const out = execFileSync(ASIDE, ['repl', code], { encoding: 'utf8', timeout: 120_000 });
  const line = out.split('\n').find((l) => l.startsWith(MARK));
  if (!line) throw new Error(`Aside 응답에 결과가 없습니다:\n${out}`);
  const r = JSON.parse(line.slice(MARK.length));
  if (r.status !== 200) throw new Error(`접속 기록을 못 읽었습니다(HTTP ${r.status}) — Aside 브라우저로 웹앱에 로그인하세요.`);
  return r.body;
}

const args = process.argv.slice(2);
if (!args.includes('--last')) {
  console.log('사용: node tools/report.mjs --last');
  process.exit(2);
}
const last = accessLog().items[0];
if (!last) {
  console.log('세션 없음');
  process.exit(1);
}
const v = (x, unit = '') => (x == null ? '없음' : `${x}${unit}`);
console.log(`세션 ${last.session_id} · 시작 ${last.started_at} · 끝 ${v(last.ended_at)} (${v(last.end_reason)})`);
console.log(`뷰어 ${v(last.viewer_ua)}`);
console.log(`첫 화면 ${v(last.click_to_first_frame_ms, 'ms')} · 입력 지연 p50 ${v(last.latency_p50_ms, 'ms')} · 재연결 ${last.reconnect_count}회`);
console.log(`path=${v(last.path)}`);
process.exit(last.path ? 0 : 1);
