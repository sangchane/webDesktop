// 로컬 시험 무대 — wrangler dev 두 개(hub 8787·app 8788) + 가짜 TURN + 가짜 에이전트·뷰어 도우미.
// 허브 시험(hub.test.mjs)과 웹앱 화면 시험(web.test.mjs)이 같이 쓴다.
import assert from 'node:assert/strict';
import { spawn, execSync } from 'node:child_process';
import { generateKeyPairSync, sign } from 'node:crypto';
import { writeFileSync, rmSync, readFileSync, mkdtempSync } from 'node:fs';
import { tmpdir, homedir } from 'node:os';
import { createServer } from 'node:http';
import { join, delimiter } from 'node:path';

export const ROOT = new URL('..', import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, '$1');
export const HUB = 'http://127.0.0.1:8787';
export const APP = 'http://127.0.0.1:8788';
const DEV_VARS = join(ROOT, 'crates/hub/.dev.vars');
const toml = readFileSync(join(ROOT, 'crates/hub/wrangler.toml'), 'utf8');
const TEAM = toml.match(/ACCESS_TEAM_DOMAIN = "(.+)"/)[1];
const AUD = toml.match(/ACCESS_AUD = "(.+)"/)[1];
export const CODE_TTL = 10 * 60_000; // REG_CODE_TTL_MIN
export const OFFLINE = 90_000; // OFFLINE_DETECT_SEC
export const TICKET_TTL = 60_000; // WS_TICKET_TTL_SEC
export const INSTALL_BYTES = Buffer.from('MZ fake agent');

const rsa = generateKeyPairSync('rsa', { modulusLength: 2048 });
const procs = [];
let persist = null; // wrangler 로컬 DB 폴더 — 시험이 끝나면 지운다.

export function jwt(over = {}, key = rsa.privateKey) {
  const b = (o) => Buffer.from(JSON.stringify(o)).toString('base64url');
  const head = b({ alg: 'RS256', kid: 'test', typ: 'JWT' });
  const body = b({ aud: [AUD], iss: `https://${TEAM}`, exp: Math.floor(Date.now() / 1000) + 600, email: 'owner@test', ...over });
  return `${head}.${body}.${sign('sha256', Buffer.from(`${head}.${body}`), key).toString('base64url')}`;
}

// 시계를 옮기면(now) 토큰 만료도 그 시각 기준으로.
// ip: 시험 모드에서 빈도 한도를 묶음마다 따로 세는 주소(x-test-ip).
export async function api(path, { method = 'GET', body, now, ip, token = jwt(now ? { exp: Math.floor(now / 1000) + 600 } : {}) } = {}) {
  const headers = { 'content-type': 'application/json' };
  if (token) headers['cf-access-jwt-assertion'] = token;
  if (now) headers['x-test-now'] = String(now);
  if (ip) headers['x-test-ip'] = ip;
  const r = await fetch(APP + path, { method, headers, body: body && JSON.stringify(body) });
  return { status: r.status, body: await r.json().catch(() => null) };
}

export async function hub(path, { method = 'GET', body, now, ip } = {}) {
  const headers = { 'content-type': 'application/json' };
  if (now) headers['x-test-now'] = String(now);
  if (ip) headers['x-test-ip'] = ip;
  const r = await fetch(HUB + path, { method, headers, body: typeof body === 'string' ? body : body && JSON.stringify(body) });
  return { status: r.status, body: await r.json().catch(() => null) };
}

export function agentKey() {
  const { publicKey, privateKey } = generateKeyPairSync('ed25519');
  const pub = Buffer.from(publicKey.export({ format: 'jwk' }).x, 'base64url').toString('base64');
  return { pub, privateKey };
}

export const enrollBody = (code, pub, name = '집 PC') => ({ code, pubkey: pub, name, os: 'windows', agent_version: '0.1.0' });

/** WebSocket 을 열고 받은 메시지·닫힘을 줄 세운다. skip 에 맞는 메시지는 줄에 넣지 않는다. */
export function open(url, headers = {}, skip = () => false) {
  const ws = new WebSocket(url, { headers });
  const opened = new Promise((res, rej) => {
    ws.onopen = res;
    ws.onerror = rej;
  });
  const inbox = [];
  const waiters = [];
  let closed = null;
  const push = (x) => (waiters.length ? waiters.shift()(x) : inbox.push(x));
  ws.onmessage = (e) => {
    const m = typeof e.data === 'string' && e.data.startsWith('{') ? JSON.parse(e.data) : e.data;
    if (!skip(m)) push(m);
  };
  ws.onclose = (e) => {
    closed = { code: e.code, reason: e.reason };
    push({ closed });
  };
  const next = (ms = 5000) =>
    inbox.length
      ? Promise.resolve(inbox.shift())
      : new Promise((res, rej) => {
          const w = (x) => (clearTimeout(t), res(x));
          // 시간이 지나면 대기자를 빼야 다음 메시지를 삼키지 않는다.
          const t = setTimeout(() => (waiters.splice(waiters.indexOf(w), 1), rej(new Error('대기 시간 초과'))), ms);
          waiters.push(w);
        });
  return { ws, next, opened, closed: () => closed };
}

/** ms 동안 pred 에 맞는 메시지가 오지 않으면 true. 다른 메시지는 버린다. */
export async function quiet(c, pred, ms = 500) {
  try {
    await until(c, pred, ms);
    return false;
  } catch (e) {
    if (e.message !== '대기 시간 초과') throw e;
    return true;
  }
}

/** pred 에 맞는 메시지가 올 때까지 다른 메시지는 버린다. */
export async function until(c, pred, ms = 5000) {
  const end = Date.now() + ms;
  for (;;) {
    const m = await c.next(Math.max(1, end - Date.now()));
    if (m && m.closed) throw new Error(`닫힘 ${JSON.stringify(m.closed)}`);
    if (pred(m)) return m;
  }
}

export async function connectAgent(deviceId, key, { proto = 1, sigOver, ip } = {}) {
  // 뷰어가 붙을 때마다 오는 ask.sync(FR-023)는 세션 시험의 「다음 메시지」를 밀어낸다 — 가짜 에이전트는 기다리는 요청이 없다.
  const a = open(HUB.replace('http', 'ws') + '/agent/ws', ip ? { 'x-test-ip': ip } : {}, (m) => m?.t === 'ask.sync');
  const hello = await a.next();
  assert.equal(hello.t, 'hello');
  const sig = sigOver ?? sign(null, Buffer.from(hello.nonce + deviceId), key.privateKey).toString('base64');
  a.ws.send(JSON.stringify({ t: 'auth', device_id: deviceId, sig, proto }));
  return a;
}

async function waitUp(url, ms) {
  const end = Date.now() + ms;
  while (Date.now() < end) {
    const dead = procs.find((x) => x.p.exitCode !== null);
    if (dead) throw new Error(`wrangler 가 끝남:
${dead.log().slice(-3000)}`);
    try {
      const r = await fetch(url);
      if (r.status < 500) return;
    } catch {}
    await new Promise((r) => setTimeout(r, 1000));
  }
  throw new Error(`${url} 가 뜨지 않음`);
}

function startWrangler(dir, port, persist) {
  // 빌드 명령(worker-build)은 크레이트 폴더에서 돌아야 한다.
  const p = spawn('npx', ['wrangler', 'dev', '--ip', '127.0.0.1', '--port', String(port), '--persist-to', persist], {
    cwd: join(ROOT, dir),
    shell: true,
    // wrangler 의 빌드 명령이 cargo·worker-build 를 찾도록.
    env: { ...process.env, PATH: `${join(homedir(), '.cargo', 'bin')}${delimiter}${process.env.PATH}` },
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  let log = '';
  p.stdout.on('data', (d) => (log += d));
  p.stderr.on('data', (d) => (log += d));
  procs.push({ p, log: () => log });
}

// 가짜 Cloudflare TURN — generate-ice-servers 꼴(201, 포트 53 주소 포함). turn.fail 이면 500.
export const turn = { calls: [], fail: false };
const turnServer = createServer((req, res) => {
  let body = '';
  req.on('data', (d) => (body += d));
  req.on('end', () => {
    turn.calls.push({ url: req.url, auth: req.headers.authorization, body: JSON.parse(body || '{}') });
    if (turn.fail) return res.writeHead(500).end('{}');
    res.writeHead(201, { 'content-type': 'application/json' }).end(
      JSON.stringify({
        iceServers: [
          { urls: ['stun:stun.cloudflare.com:3478', 'stun:stun.cloudflare.com:53'] },
          { urls: ['turn:turn.cloudflare.com:3478?transport=udp', 'turn:turn.cloudflare.com:53?transport=udp'], username: 'u1', credential: 'c1' },
        ],
      }),
    );
  });
});

export async function startStack() {
  // 빈 포트 — 같은 PC 의 다른 wrangler dev(8790 등)와 겹치지 않게.
  await new Promise((r, j) => turnServer.once('error', j).listen(0, '127.0.0.1', r));
  const jwk = { ...rsa.publicKey.export({ format: 'jwk' }), kid: 'test', alg: 'RS256', use: 'sig' };
  writeFileSync(
    DEV_VARS,
    `ACCESS_TEST_JWKS='${JSON.stringify({ keys: [jwk] })}'\nHUB_PUBLIC_URL=${HUB}\n` +
      `TURN_KEY_ID=test-key\nTURN_KEY_TOKEN=test-token\nTURN_TEST_URL=http://127.0.0.1:${turnServer.address().port}\n`,
  );
  persist = mkdtempSync(join(tmpdir(), 'webdesktop-hub-'));
  // E-12 설치 파일 자리 — 로컬 R2 에 작은 가짜 실행 파일(macOS 는 비워 not_found 를 본다).
  writeFileSync(join(persist, 'agent.exe'), INSTALL_BYTES);
  execSync(`npx wrangler r2 object put webdesktop-releases/windows/webdesktop-agent.exe --local --persist-to "${persist}" --file "${join(persist, 'agent.exe')}"`, {
    cwd: join(ROOT, 'crates/hub'),
    stdio: 'ignore',
  });
  startWrangler('crates/hub', 8787, persist);
  await waitUp(HUB + '/healthz', 600_000);
  startWrangler('crates/app', 8788, persist);
  await waitUp(APP + '/api/devices', 600_000);
}

export function stopStack() {
  turnServer.close();
  rmSync(DEV_VARS, { force: true });
  for (const { p, log } of procs) {
    if (process.env.HUB_TEST_LOG) console.log(log());
    try {
      execSync(`taskkill /pid ${p.pid} /T /F`, { stdio: 'ignore' });
    } catch {}
  }
  // workerd 가 막 꺼져 파일을 잡고 있을 수 있다 — 몇 번 다시 시도.
  try {
    if (persist) rmSync(persist, { recursive: true, force: true, maxRetries: 5, retryDelay: 500 });
  } catch (e) {
    console.warn(`시험 DB 폴더를 지우지 못함(시험 결과와 무관): ${persist} — ${e.code}`);
  }
}
