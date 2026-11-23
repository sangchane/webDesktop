// 허브 통합 시험 (SPEC C4, 06 표) — 로컬 wrangler dev 두 개(hub·app)에 가짜 에이전트·가짜 뷰어로 붙는다.
// 실행: node --test tools/hub.test.mjs  (처음엔 두 Worker 를 빌드하느라 몇 분 걸린다)
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import { spawn, execSync } from 'node:child_process';
import { generateKeyPairSync, sign } from 'node:crypto';
import { writeFileSync, rmSync, readFileSync, mkdtempSync } from 'node:fs';
import { tmpdir, homedir } from 'node:os';
import { join, delimiter } from 'node:path';

const ROOT = new URL('..', import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, '$1');
const HUB = 'http://127.0.0.1:8787';
const APP = 'http://127.0.0.1:8788';
const DEV_VARS = join(ROOT, 'crates/hub/.dev.vars');
const toml = readFileSync(join(ROOT, 'crates/hub/wrangler.toml'), 'utf8');
const TEAM = toml.match(/ACCESS_TEAM_DOMAIN = "(.+)"/)[1];
const AUD = toml.match(/ACCESS_AUD = "(.+)"/)[1];
const CODE_TTL = 10 * 60_000; // REG_CODE_TTL_MIN
const OFFLINE = 90_000; // OFFLINE_DETECT_SEC

const rsa = generateKeyPairSync('rsa', { modulusLength: 2048 });
const procs = [];

function jwt(over = {}, key = rsa.privateKey) {
  const b = (o) => Buffer.from(JSON.stringify(o)).toString('base64url');
  const head = b({ alg: 'RS256', kid: 'test', typ: 'JWT' });
  const body = b({ aud: [AUD], iss: `https://${TEAM}`, exp: Math.floor(Date.now() / 1000) + 600, email: 'owner@test', ...over });
  return `${head}.${body}.${sign('sha256', Buffer.from(`${head}.${body}`), key).toString('base64url')}`;
}

// 시계를 옮기면(now) 토큰 만료도 그 시각 기준으로.
async function api(path, { method = 'GET', now, token = jwt(now ? { exp: Math.floor(now / 1000) + 600 } : {}) } = {}) {
  const headers = {};
  if (token) headers['cf-access-jwt-assertion'] = token;
  if (now) headers['x-test-now'] = String(now);
  const r = await fetch(APP + path, { method, headers });
  return { status: r.status, body: await r.json().catch(() => null) };
}

async function hub(path, { method = 'GET', body, now } = {}) {
  const headers = { 'content-type': 'application/json' };
  if (now) headers['x-test-now'] = String(now);
  const r = await fetch(HUB + path, { method, headers, body: typeof body === 'string' ? body : body && JSON.stringify(body) });
  return { status: r.status, body: await r.json().catch(() => null) };
}

function agentKey() {
  const { publicKey, privateKey } = generateKeyPairSync('ed25519');
  const pub = Buffer.from(publicKey.export({ format: 'jwk' }).x, 'base64url').toString('base64');
  return { pub, privateKey };
}

const enrollBody = (code, pub, name = '집 PC') => ({ code, pubkey: pub, name, os: 'windows', agent_version: '0.1.0' });

/** WebSocket 을 열고 받은 메시지·닫힘을 줄 세운다. */
function open(url) {
  const ws = new WebSocket(url);
  const inbox = [];
  const waiters = [];
  let closed = null;
  const push = (x) => (waiters.length ? waiters.shift()(x) : inbox.push(x));
  ws.onmessage = (e) => push(typeof e.data === 'string' && e.data.startsWith('{') ? JSON.parse(e.data) : e.data);
  ws.onclose = (e) => {
    closed = { code: e.code, reason: e.reason };
    push({ closed });
  };
  const next = (ms = 5000) =>
    inbox.length
      ? Promise.resolve(inbox.shift())
      : new Promise((res, rej) => {
          const t = setTimeout(() => rej(new Error('대기 시간 초과')), ms);
          waiters.push((x) => (clearTimeout(t), res(x)));
        });
  return { ws, next, closed: () => closed };
}

async function connectAgent(deviceId, key, { proto = 1, sigOver } = {}) {
  const a = open(HUB.replace('http', 'ws') + '/agent/ws');
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

before(async () => {
  const jwk = { ...rsa.publicKey.export({ format: 'jwk' }), kid: 'test', alg: 'RS256', use: 'sig' };
  writeFileSync(DEV_VARS, `ACCESS_TEST_JWKS='${JSON.stringify({ keys: [jwk] })}'\nHUB_PUBLIC_URL=${HUB}\n`);
  const persist = mkdtempSync(join(tmpdir(), 'webdesktop-hub-'));
  startWrangler('crates/hub', 8787, persist);
  await waitUp(HUB + '/healthz', 600_000);
  startWrangler('crates/app', 8788, persist);
  await waitUp(APP + '/api/devices', 600_000);
});

after(() => {
  rmSync(DEV_VARS, { force: true });
  for (const { p, log } of procs) {
    if (process.env.HUB_TEST_LOG) console.log(log());
    try {
      execSync(`taskkill /pid ${p.pid} /T /F`, { stdio: 'ignore' });
    } catch {}
  }
});

// ---- FR-001 Access ----
test('Access: JWT 없음·aud 틀림·서명 틀림·만료는 401, 맞으면 목록', async () => {
  assert.equal((await api('/api/devices', { token: null })).body.type, 'unauthorized');
  assert.equal((await api('/api/devices', { token: jwt({ aud: ['other'] }) })).status, 401);
  const other = generateKeyPairSync('rsa', { modulusLength: 2048 }).privateKey;
  assert.equal((await api('/api/devices', { token: jwt({}, other) })).status, 401);
  assert.equal((await api('/api/devices', { token: jwt({ exp: 1 }) })).status, 401);
  assert.equal((await api('/api/devices', { token: jwt({ nbf: Math.floor(Date.now() / 1000) + 3600 }) })).status, 401);
  const ok = await api('/api/devices');
  assert.equal(ok.status, 200);
  assert.deepEqual(ok.body, []);
});

test('공개 허브는 /api/* 를 받지 않는다', async () => {
  assert.equal((await hub('/api/devices')).status, 404);
});

// ---- FR-002 등록 ----
test('등록 코드: 꼴·설치 링크·설치 링크 만료', async () => {
  const t0 = Date.now();
  const { status, body } = await api('/api/enroll-codes', { method: 'POST', now: t0 });
  assert.equal(status, 201);
  assert.match(body.code, /^[2-9A-HJ-NP-Z]{8}$/);
  const link = new URL(body.install_links.windows);
  assert.equal(link.origin, HUB);
  // 토큰이 맞으면 토큰 오류가 아니다(설치 파일은 T8 뒤라 지금은 not_found).
  assert.equal((await hub(link.pathname + link.search, { now: t0 })).body.type, 'not_found');
  assert.equal((await hub(link.pathname + '?t=wrong', { now: t0 })).body.type, 'code_invalid');
  assert.equal((await hub(link.pathname + link.search, { now: t0 + CODE_TTL + 1 })).body.type, 'code_expired');
});

test('등록: 만료 코드는 code_expired, 그 뒤 같은 코드는 code_invalid', async () => {
  const t0 = Date.now();
  const { body } = await api('/api/enroll-codes', { method: 'POST', now: t0 });
  const k = agentKey();
  const r = await hub('/agent/enroll', { method: 'POST', body: enrollBody(body.code, k.pub), now: t0 + CODE_TTL + 1 });
  assert.equal(r.body.type, 'code_expired');
  assert.equal((await hub('/agent/enroll', { method: 'POST', body: enrollBody(body.code, k.pub) })).body.type, 'code_invalid');
});

test('등록: 틀린 코드 5회면 활성 코드 폐기 — 맞는 코드도 code_invalid', async () => {
  const { body } = await api('/api/enroll-codes', { method: 'POST' });
  const k = agentKey();
  for (let i = 0; i < 5; i++) {
    assert.equal((await hub('/agent/enroll', { method: 'POST', body: enrollBody('WRONG234', k.pub) })).body.type, 'code_invalid');
  }
  assert.equal((await hub('/agent/enroll', { method: 'POST', body: enrollBody(body.code, k.pub) })).body.type, 'code_invalid');
});

test('등록: 새 코드 B 를 만들면 A 와 A 의 설치 링크는 code_invalid', async () => {
  const a = (await api('/api/enroll-codes', { method: 'POST' })).body;
  await api('/api/enroll-codes', { method: 'POST' });
  const k = agentKey();
  assert.equal((await hub('/agent/enroll', { method: 'POST', body: enrollBody(a.code, k.pub) })).body.type, 'code_invalid');
  const link = new URL(a.install_links.windows);
  assert.equal((await hub(link.pathname + link.search)).body.type, 'code_invalid');
});

test('꼴 검사: 깨진 JSON·긴 이름은 bad_request, 큰 본문은 too_large', async () => {
  assert.equal((await hub('/agent/enroll', { method: 'POST', body: '{nope' })).body.type, 'bad_request');
  const k = agentKey();
  assert.equal((await hub('/agent/enroll', { method: 'POST', body: enrollBody('ABCD2345', k.pub, 'x'.repeat(65)) })).body.type, 'bad_request');
  assert.equal((await hub('/agent/enroll', { method: 'POST', body: 'x'.repeat(70_000) })).body.type, 'too_large');
});

// ---- 등록 성공 → 서명 WS → 목록 ----
const dev = { key: agentKey(), id: null };

test('등록 성공 → 재사용은 code_invalid', async () => {
  const { body } = await api('/api/enroll-codes', { method: 'POST' });
  const r = await hub('/agent/enroll', { method: 'POST', body: enrollBody(body.code, dev.key.pub) });
  assert.equal(r.status, 201);
  assert.match(r.body.device_id, /^[0-9a-f]{8}-[0-9a-f]{4}-7[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/);
  dev.id = r.body.device_id;
  assert.equal((await hub('/agent/enroll', { method: 'POST', body: enrollBody(body.code, agentKey().pub) })).body.type, 'code_invalid');
  const list = (await api('/api/devices')).body;
  assert.equal(list.length, 1);
  assert.equal(list[0].online, false);
});

test('서명 WS: 틀린 서명·모르는 기기는 bad_signature, 판이 다르면 upgrade_required', async () => {
  const bad = await connectAgent(dev.id, dev.key, { sigOver: Buffer.alloc(64).toString('base64') });
  assert.equal((await bad.next()).type, 'bad_signature');
  const other = await connectAgent(dev.id, agentKey());
  assert.equal((await other.next()).type, 'bad_signature');
  const unknown = await connectAgent('01234567-89ab-7fff-bfff-ffffffffffff', dev.key);
  assert.equal((await unknown.next()).type, 'bad_signature');
  const old = await connectAgent(dev.id, dev.key, { proto: 2 });
  assert.equal((await old.next()).type, 'upgrade_required');
  assert.equal((await old.next()).closed.code, 4002);
  assert.equal((await api('/api/devices')).body[0].online, false);
});

test('서명 WS: auth 전에 다른 메시지는 unauthorized', async () => {
  const a = open(HUB.replace('http', 'ws') + '/agent/ws');
  await a.next();
  a.ws.send(JSON.stringify({ t: 'viewer.hello', viewer_id: 'v1', proto: 1 }));
  assert.equal((await a.next()).type, 'unauthorized');
});

test('서명 WS: auth 전에 꼴이 틀린 글을 보내면 바로 닫힘', async () => {
  const a = open(HUB.replace('http', 'ws') + '/agent/ws');
  await a.next();
  a.ws.send('{garbage');
  assert.equal((await a.next()).type, 'bad_request');
  assert.equal((await a.next()).closed.code, 4002);
});

test('서명 WS 성공 → 온라인, ping 에 pong, 다시 붙으면 앞 연결 닫힘', async () => {
  const a = await connectAgent(dev.id, dev.key);
  a.ws.send('ping');
  assert.equal(await a.next(), 'pong');
  const d = (await api('/api/devices')).body[0];
  assert.equal(d.online, true);
  assert.equal(d.connectable, true);
  assert.ok(d.last_seen_at);
  const b = await connectAgent(dev.id, dev.key);
  b.ws.send('ping');
  assert.equal(await b.next(), 'pong');
  assert.equal((await a.next()).closed.code, 4000);
  dev.ws = b;
});

// ---- FR-003 오프라인 판정 ----
test('조용한 연결: OFFLINE_DETECT_SEC 지나면 목록에서 꺼짐 + 그 WS 닫힘', async () => {
  const later = Date.now() + OFFLINE + 5_000;
  const d = (await api('/api/devices', { now: later })).body[0];
  assert.equal(d.online, false);
  assert.equal(d.offline_long, false);
  assert.equal((await dev.ws.next()).closed.code, 4001);
});

test('오래 꺼짐: OFFLINE_WARN_HOURS 지나면 offline_long', async () => {
  const d = (await api('/api/devices', { now: Date.now() + 25 * 3_600_000 })).body[0];
  assert.equal(d.offline_long, true);
});

// 빈도 제한은 이 분의 나머지 요청을 막으므로 맨 끝에 둔다.
test('빈도: 분당 60회를 넘으면 rate_limited', async () => {
  let limited = null;
  for (let i = 0; i < 70 && !limited; i++) {
    const r = await hub('/agent/enroll', { method: 'POST', body: '{}' });
    if (r.status === 429) limited = r.body;
  }
  assert.equal(limited?.type, 'rate_limited');
});
