// 허브 통합 시험 (SPEC C4, 06 표) — 로컬 wrangler dev 두 개(hub·app)에 가짜 에이전트·가짜 뷰어로 붙는다.
// 실행: node --test tools/hub.test.mjs  (처음엔 두 Worker 를 빌드하느라 몇 분 걸린다)
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import { generateKeyPairSync } from 'node:crypto';
import {
  HUB, CODE_TTL, OFFLINE, INSTALL_BYTES, TICKET_TTL, turn, jwt, api, hub, agentKey, enrollBody,
  open, quiet, until, connectAgent, startStack, stopStack,
} from './stack.mjs';

before(startStack);
after(stopStack);

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
  // 토큰이 맞으면 R2 의 설치 파일, 아직 올리지 않은 OS(macOS)는 not_found.
  const file = await fetch(link, { headers: { 'x-test-now': String(t0) } });
  assert.equal(file.status, 200);
  assert.equal(file.headers.get('content-length'), String(INSTALL_BYTES.length));
  assert.match(file.headers.get('content-disposition'), /filename="webdesktop-agent\.exe"/);
  assert.deepEqual(Buffer.from(await file.arrayBuffer()), INSTALL_BYTES);
  const mac = new URL(body.install_links.macos);
  assert.equal((await hub(mac.pathname + mac.search, { now: t0 })).body.type, 'not_found');
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

test('서명 WS: 인증 전 연결 상한은 IP 마다 — 한 IP 가 채워도 다른 IP 는 붙는다', async () => {
  const url = HUB.replace('http', 'ws') + '/agent/ws';
  const flood = [];
  for (let i = 0; i < 16; i++) {
    const c = open(url, { 'x-test-ip': 'flood' });
    await c.next();
    flood.push(c);
  }
  await assert.rejects(open(url, { 'x-test-ip': 'flood' }).opened);
  const other = open(url, { 'x-test-ip': 'other' });
  assert.equal((await other.next()).t, 'hello');
  for (const c of [...flood, other]) c.ws.close();
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

// ---- T5 세션 (FR-004, E-13·E-06·E-07, M-01~M-04·M-10) ----
// 이 묶음은 빈도 한도를 따로 센다.
const IP = 'session';
const ticket = (viewer_id, opts = {}) => api('/api/ws-ticket', { method: 'POST', body: { viewer_id }, ip: IP, ...opts });
const openSession = (viewer_id, device_id = dev.id) =>
  api('/api/sessions', { method: 'POST', body: { device_id, viewer_id }, ip: IP });
const S = {};

/** 입장권을 받아 뷰어 WS 를 열고 viewer.hello 까지. */
async function viewer(viewer_id, hello = { t: 'viewer.hello', viewer_id, proto: 1 }) {
  const t = (await ticket(viewer_id)).body;
  const v = open(`${t.hub_ws_url}?ticket=${t.ticket}`, { 'x-test-ip': IP });
  await v.opened;
  v.ws.send(JSON.stringify(hello));
  return v;
}

const send = (c, m) => c.ws.send(JSON.stringify(m));

test('입장권: Access 필요, 없음·틀림·만료·재사용은 401', async () => {
  assert.equal((await ticket('viewer-a', { token: null })).status, 401);
  assert.equal((await ticket('bad id!')).body.type, 'bad_request');
  assert.equal((await hub('/ws/viewer', { ip: IP })).body.type, 'unauthorized');
  assert.equal((await hub('/ws/viewer?ticket=nope', { ip: IP })).body.type, 'unauthorized');
  const t0 = Date.now();
  const t = await ticket('viewer-a', { now: t0 });
  assert.equal(t.status, 201);
  assert.equal(t.body.hub_ws_url, HUB.replace('http', 'ws') + '/ws/viewer');
  assert.equal(Date.parse(t.body.expires_at), t0 + TICKET_TTL);
  assert.equal((await hub(`/ws/viewer?ticket=${t.body.ticket}`, { now: t0 + TICKET_TTL, ip: IP })).status, 401);
  const t2 = (await ticket('viewer-a')).body;
  const v = open(`${t2.hub_ws_url}?ticket=${t2.ticket}`, { 'x-test-ip': IP });
  await v.opened;
  assert.equal((await hub(`/ws/viewer?ticket=${t2.ticket}`, { ip: IP })).status, 401);
  v.ws.close();
});

test('viewer.hello: 입장권과 다른 viewer_id 는 unauthorized, 판이 다르면 upgrade_required', async () => {
  const a = await viewer('viewer-a', { t: 'viewer.hello', viewer_id: 'someone-else', proto: 1 });
  assert.equal((await a.next()).type, 'unauthorized');
  assert.equal((await a.next()).closed.code, 4002);
  const b = await viewer('viewer-a', { t: 'viewer.hello', viewer_id: 'viewer-a', proto: 2 });
  assert.equal((await b.next()).type, 'upgrade_required');
  const c = await viewer('viewer-a', { t: 'signal.restart', session_id: 'x' });
  assert.equal((await c.next()).type, 'unauthorized');
});

test('device.status: 붙으면 지금 목록, 에이전트가 붙고 끊기면 밀어줌', async () => {
  const a = await viewer('viewer-a');
  const first = await a.next();
  assert.equal(first.t, 'device.status');
  assert.equal(first.device_id, dev.id);
  assert.equal(first.online, false);
  const agent = await connectAgent(dev.id, dev.key, { ip: IP });
  const on = await until(a, (m) => m.t === 'device.status');
  assert.deepEqual([on.device_id, on.online, on.connectable], [dev.id, true, true]);
  // 같은 기기가 다시 붙으면 앞 연결이 닫혀도 「꺼짐」을 밀어주지 않는다.
  const again = await connectAgent(dev.id, dev.key, { ip: IP });
  assert.equal((await agent.next()).closed.code, 4000);
  assert.ok(await quiet(a, (m) => m.t === 'device.status' && !m.online));
  again.ws.close();
  const off = await until(a, (m) => m.t === 'device.status' && !m.online);
  assert.equal(off.device_id, dev.id);
  S.a = a;
});

test('세션 요청: 모르는 기기 not_found, 꺼진 기기 device_offline, 꼴 틀림 bad_request', async () => {
  assert.equal((await openSession('viewer-a', '01234567-89ab-7fff-bfff-ffffffffffff')).body.type, 'not_found');
  const off = await openSession('viewer-a');
  assert.equal(off.status, 409);
  assert.equal(off.body.type, 'device_offline');
  assert.equal((await api('/api/sessions', { method: 'POST', body: { device_id: dev.id }, ip: IP })).body.type, 'bad_request');
});

test('세션: TURN 자격·session.start·신호 중계, 같은 뷰어는 같은 세션', async () => {
  S.agent = await connectAgent(dev.id, dev.key, { ip: IP });
  await until(S.a, (m) => m.t === 'device.status' && m.online);
  turn.calls.length = 0;
  const r = await openSession('viewer-a');
  assert.equal(r.status, 201);
  const sid = r.body.session_id;
  assert.deepEqual(
    turn.calls.map((c) => [c.url, c.auth, c.body.ttl]),
    [['/v1/turn/keys/test-key/credentials/generate-ice-servers', 'Bearer test-token', 3600]],
  );
  const urls = r.body.ice_servers.flatMap((s) => s.urls);
  assert.deepEqual(urls, ['stun:stun.cloudflare.com:3478', 'turn:turn.cloudflare.com:3478?transport=udp']);
  assert.deepEqual([r.body.ice_servers[1].username, r.body.ice_servers[1].credential], ['u1', 'c1']);
  const start = await until(S.agent, (m) => m.t === 'session.start');
  assert.equal(start.session_id, sid);
  assert.deepEqual(start.ice_servers, r.body.ice_servers);

  send(S.a, { t: 'signal.offer', session_id: sid, sdp: 'v=0 offer' });
  assert.deepEqual(await S.agent.next(), { t: 'signal.offer', session_id: sid, sdp: 'v=0 offer' });
  send(S.agent, { t: 'signal.answer', session_id: sid, sdp: 'v=0 answer' });
  assert.deepEqual(await until(S.a, (m) => m.t === 'signal.answer'), { t: 'signal.answer', session_id: sid, sdp: 'v=0 answer' });
  send(S.a, { t: 'signal.candidate', session_id: sid, candidate: 'candidate:1 1 udp 1 10.0.0.1 5000 typ host' });
  assert.equal((await S.agent.next()).t, 'signal.candidate');
  send(S.agent, { t: 'signal.candidate', session_id: sid, candidate: 'candidate:2 1 udp 1 10.0.0.2 5000 typ host' });
  assert.equal((await until(S.a, (m) => m.t === 'signal.candidate')).candidate, 'candidate:2 1 udp 1 10.0.0.2 5000 typ host');
  send(S.a, { t: 'signal.restart', session_id: sid });
  assert.deepEqual(await S.agent.next(), { t: 'signal.restart', session_id: sid });

  const again = await openSession('viewer-a');
  assert.equal(again.status, 200);
  assert.equal(again.body.session_id, sid);
  // 에이전트가 그사이 다시 붙었을 수 있어 session.start 를 다시 보낸다(에이전트가 session_id 로 거름).
  assert.deepEqual(await S.agent.next(), { t: 'session.start', session_id: sid, ice_servers: again.body.ice_servers });
  S.sid = sid;
});

test('이어받기: 다른 viewer_id 가 열면 앞 뷰어·에이전트에 taken_over, 앞 세션 신호는 not_found', async () => {
  const b = await viewer('viewer-b');
  const r = await openSession('viewer-b');
  assert.equal(r.status, 201);
  assert.notEqual(r.body.session_id, S.sid);
  assert.deepEqual(await until(S.a, (m) => m.t === 'session.end'), { t: 'session.end', session_id: S.sid, reason: 'taken_over' });
  assert.deepEqual(await S.agent.next(), { t: 'session.end', session_id: S.sid, reason: 'taken_over' });
  assert.equal((await S.agent.next()).session_id, r.body.session_id);
  send(S.a, { t: 'signal.offer', session_id: S.sid, sdp: 'v=0' });
  assert.equal((await until(S.a, (m) => m.t === 'error')).type, 'not_found');
  send(S.a, { t: 'signal.offer', session_id: r.body.session_id, sdp: 'v=0' });
  assert.equal((await until(S.a, (m) => m.t === 'error')).type, 'not_found');
  S.b = b;
  S.sid2 = r.body.session_id;
});

test('계측·종료: 뷰어 session.end 는 에이전트로, 허브만 쓰는 사유는 bad_request, 계측은 닫힌 뒤에도 받음', async () => {
  // shortcut: 저장된 값 대조는 접속 기록(E-09)이 생기면 그 응답으로 — 지금은 「오류 없음」(저장 실패는 internal 로 옴)까지만.
  const stats = { t: 'session.viewer_stats', session_id: S.sid2, click_to_first_frame_ms: 1200, latency_p50_ms: 80, reconnect_ms: [300], restarts: 1, new_session_used: false, path: 'relay' };
  send(S.a, stats);
  assert.equal((await until(S.a, (m) => m.t === 'error')).type, 'not_found');
  send(S.b, stats);
  send(S.b, { t: 'session.end', session_id: S.sid2, reason: 'taken_over' });
  // 계측이 거절됐다면 이 앞에 그 오류가 먼저 온다.
  assert.equal((await until(S.b, (m) => m.t === 'error')).type, 'bad_request');
  send(S.agent, { ...stats });
  assert.equal((await S.agent.next()).type, 'bad_request');
  send(S.b, { t: 'session.end', session_id: S.sid2, reason: 'viewer_closed' });
  assert.deepEqual(await S.agent.next(), { t: 'session.end', session_id: S.sid2, reason: 'viewer_closed' });
  // 닫힌 뒤 계측이 거절되면 not_found 가 먼저 온다 — 뒤 메시지는 다른 오류(bad_request)를 낸다.
  send(S.b, stats);
  send(S.b, { t: 'device.status', device_id: dev.id, online: true, perm: 'ok', connectable: true, last_seen_at: null });
  assert.equal((await until(S.b, (m) => m.t === 'error')).type, 'bad_request');
  send(S.b, { t: 'signal.offer', session_id: S.sid2, sdp: 'v=0' });
  assert.equal((await until(S.b, (m) => m.t === 'error')).type, 'not_found');
  // E-09: 저장된 계측이 접속 기록에 그대로 나온다.
  const log = await api('/api/access-log');
  assert.equal(log.status, 200);
  const row = log.body.items.find((i) => i.session_id === S.sid2);
  assert.equal(row.path, 'relay');
  assert.equal(row.click_to_first_frame_ms, 1200);
  assert.deepEqual(row.reconnect_ms, [300]);
  assert.equal(row.end_reason, 'viewer_closed');
  // 새것부터, 커서 다음 쪽은 그 줄보다 앞선 것만.
  const keys = log.body.items.map((i) => `${Date.parse(i.started_at)}_${i.session_id}`);
  assert.ok(keys.length >= 2);
  assert.deepEqual(keys, [...keys].sort((a, b) => (Number(a.split('_')[0]) - Number(b.split('_')[0])) || (a < b ? -1 : 1)).reverse());
  assert.equal(log.body.next, null);
  const after = await api(`/api/access-log?before=${keys[0]}`);
  assert.deepEqual(after.body.items.map((i) => i.session_id), log.body.items.slice(1).map((i) => i.session_id));
  assert.equal((await api('/api/access-log?before=x')).body.type, 'bad_request');
  assert.equal((await api('/api/access-log', { token: null })).status, 401);
});

test('에이전트 session.end·TURN 실패·에이전트 끊김', async () => {
  const r = await openSession('viewer-b');
  assert.equal(r.status, 201);
  assert.notEqual(r.body.session_id, S.sid2);
  await until(S.agent, (m) => m.t === 'session.start');
  send(S.agent, { t: 'session.end', session_id: r.body.session_id, reason: 'viewer_closed' });
  assert.equal((await S.agent.next()).type, 'bad_request');
  send(S.agent, { t: 'session.end', session_id: r.body.session_id, reason: 'host_closed' });
  assert.deepEqual(await until(S.b, (m) => m.t === 'session.end'), { t: 'session.end', session_id: r.body.session_id, reason: 'host_closed' });

  turn.fail = true;
  const failed = await openSession('viewer-b');
  turn.fail = false;
  assert.equal(failed.status, 500);
  assert.equal(failed.body.type, 'internal');

  S.agent.ws.close();
  assert.equal((await until(S.b, (m) => m.t === 'device.status' && !m.online)).device_id, dev.id);
  assert.equal((await openSession('viewer-b')).body.type, 'device_offline');
  S.a.ws.close();
  S.b.ws.close();
});

// 빈도 제한은 이 분의 나머지 요청을 막으므로 맨 끝에 둔다.
test('빈도: 분당 60회를 넘으면 rate_limited', async () => {
  let limited = null;
  // 한도는 달력 분 단위라 요청이 분 경계를 넘으면 두 분에 나뉜다 — 시계를 한 분에 고정한다.
  const now = Date.now();
  for (let i = 0; i < 70 && !limited; i++) {
    const r = await hub('/agent/enroll', { method: 'POST', body: '{}', now });
    if (r.status === 429) limited = r.body;
  }
  assert.equal(limited?.type, 'rate_limited');
});
