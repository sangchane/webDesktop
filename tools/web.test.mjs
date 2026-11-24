// 웹앱 화면 시험 (T7) — 실제 브라우저(Playwright Chromium)로 웹앱을 열고, 가짜 에이전트(허브 WS) + 가짜 호스트 화면(다른 탭의
// RTCPeerConnection·캔버스 영상)으로 기기 추가 → 목록 → 원격 화면 → 입력 → 이어받기 → 끊기를 끝까지 돈다.
// 실행: node --test tools/web.test.mjs   (trunk build --release 를 먼저 돌린다)
// 안 보는 것: 실제 에이전트(str0m·H.264 — T9), TURN 중계(가짜 자격이라 — T10 C7), 입력 지연 짝짓기(D-08 은 단위 시험).
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import { execSync, execFileSync } from 'node:child_process';
import { existsSync, mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir, homedir } from 'node:os';
import { join, delimiter } from 'node:path';
import { chromium } from 'playwright';
import { ROOT, APP, jwt, hub, agentKey, enrollBody, connectAgent, startStack, stopStack } from './stack.mjs';

const UI_CHECK =
  process.env.UI_CHECK ?? join(homedir(), '.claude/plugins/cache/sangchane/dev/1.14.0/skills/ui/scripts/ui-check.mjs');
const OUT = mkdtempSync(join(tmpdir(), 'webdesktop-web-'));
const HID = { enter: 0x28, esc: 0x29, ctrl: 0xe0, c: 0x06 };

let browser, host, desk, phone, agent, deviceId;
const toAgent = []; // 에이전트가 받은 메시지
const stats = []; // 뷰어가 보낸 session.viewer_stats
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

async function eventually(fn, what, ms = 15000) {
  const end = Date.now() + ms;
  for (;;) {
    const v = await fn();
    if (v) return v;
    if (Date.now() > end) throw new Error(`기다림 초과: ${what}`);
    await sleep(100);
  }
}

const send = (m) => agent.ws.send(JSON.stringify(m));
const inputs = (page, sid) => host.evaluate((sid) => window.sessions[sid]?.inputs ?? [], sid);
const sessionOf = (n) => toAgent.filter((m) => m.t === 'session.start')[n]?.session_id;

/** 가짜 호스트 — 세션마다 RTCPeerConnection 하나, 캔버스를 영상으로 보내고 input 채널을 모은다. */
async function startHost() {
  host = await browser.newPage();
  await host.exposeFunction('hostCandidate', (sid, candidate) => relay(sid, candidate));
  await host.evaluate(() => {
    window.sessions = {};
    window.answer = async (sid, sdp) => {
      const pc = new RTCPeerConnection();
      const s = (window.sessions[sid] = { pc, inputs: [] });
      pc.ondatachannel = (e) => {
        if (e.channel.label === 'input') e.channel.onmessage = (m) => s.inputs.push(JSON.parse(m.data));
      };
      pc.onicecandidate = (e) => e.candidate?.candidate && window.hostCandidate(sid, e.candidate.candidate);
      const c = Object.assign(document.createElement('canvas'), { width: 640, height: 360 });
      const g = c.getContext('2d');
      s.timer = setInterval(() => {
        g.fillStyle = `hsl(${(Date.now() / 10) % 360} 50% 40%)`;
        g.fillRect(0, 0, 640, 360);
      }, 33);
      const stream = c.captureStream(30);
      await pc.setRemoteDescription({ type: 'offer', sdp });
      pc.addTrack(stream.getVideoTracks()[0], stream);
      await pc.setLocalDescription(await pc.createAnswer());
      return pc.localDescription.sdp;
    };
    window.addCandidate = (sid, candidate) => window.sessions[sid]?.pc.addIceCandidate({ candidate, sdpMLineIndex: 0 });
  });
}

// 호스트 후보는 나오는 대로 보낸다 — answer 보다 먼저 가도 뷰어가 모아 두었다가 넣어야 한다(DL-16 리뷰).
const relay = (sid, candidate) => send({ t: 'signal.candidate', session_id: sid, candidate });

/** 가짜 에이전트의 받기 고리 — 신호를 가짜 호스트 탭에 잇는다. */
function wireAgent() {
  agent.ws.addEventListener('message', async (e) => {
    if (typeof e.data !== 'string' || !e.data.startsWith('{')) return;
    const m = JSON.parse(e.data);
    toAgent.push(m);
    if (m.t === 'signal.offer') {
      const sdp = await host.evaluate(([sid, sdp]) => window.answer(sid, sdp), [m.session_id, m.sdp]);
      send({ t: 'signal.answer', session_id: m.session_id, sdp });
    } else if (m.t === 'signal.candidate') {
      await host.evaluate(([sid, c]) => window.addCandidate(sid, c), [m.session_id, m.candidate]);
    }
  });
}

async function viewerPage(opts) {
  const ctx = await browser.newContext({ extraHTTPHeaders: { 'cf-access-jwt-assertion': jwt() }, ...opts });
  const page = await ctx.newPage();
  page.on('pageerror', (e) => console.log('웹앱 오류:', e.message));
  page.on('websocket', (ws) =>
    ws.on('framesent', (f) => {
      try {
        const m = JSON.parse(f.payload);
        if (m.t === 'session.viewer_stats') stats.push(m);
      } catch {}
    }),
  );
  await page.goto(APP + '/');
  return page;
}

/** 지금 화면을 스크립트 없는 HTML 로 떠서 ui-check 에 준다. */
async function snapshot(page, name) {
  const html = (await page.content()).replace(/<script[\s\S]*?<\/script>/g, '').replace(/<link[^>]+(modulepreload|preload)[^>]*>/g, '');
  const f = join(OUT, `${name}.html`);
  writeFileSync(f, html);
  return f;
}

before(async () => {
  execSync('trunk build --release', {
    cwd: join(ROOT, 'crates/web'),
    stdio: 'ignore',
    env: { ...process.env, PATH: `${join(homedir(), '.cargo', 'bin')}${delimiter}${process.env.PATH}` },
  });
  await startStack();
  // 같은 브라우저의 두 탭이 mDNS 이름 없이 바로 붙게.
  browser = await chromium.launch({ args: ['--disable-features=WebRtcHideLocalIpsWithMdns'] });
  await startHost();
});

after(async () => {
  agent?.ws.close();
  await browser?.close();
  stopStack();
});

test('빈 목록 → 기기 추가 화면에 코드·설치 링크', async () => {
  desk = await viewerPage({ viewport: { width: 1440, height: 900 } });
  await desk.getByText('아직 기기가 없습니다').waitFor();
  await snapshot(desk, 'list-empty');
  await desk.getByLabel('기기 추가').click();
  const code = await desk.locator('[data-code]').getAttribute('data-code', { timeout: 15000 });
  assert.match(code, /^[2-9A-HJ-NP-Z]{8}$/);
  assert.match(await desk.locator('.link-text').innerText(), /\/agent\/install\/windows\?t=/);
  await desk.getByText(/분 \d+초 뒤 만료/).waitFor();
  await snapshot(desk, 'add');
  // 새 PC: 화면의 코드로 등록하고 서명 WS 로 붙는다.
  const key = agentKey();
  const r = await hub('/agent/enroll', { method: 'POST', body: enrollBody(code, key.pub) });
  assert.equal(r.status, 201, JSON.stringify(r.body));
  deviceId = r.body.device_id;
  agent = await connectAgent(deviceId, key);
  wireAgent();
});

test('새 PC 가 붙으면 목록으로 돌아가 「켜짐」', async () => {
  await desk.getByText('「집 PC」를 추가했습니다').waitFor({ timeout: 15000 });
  await desk.locator(`[data-device="${deviceId}"]`).getByText('켜짐').waitFor();
  await snapshot(desk, 'list');
});

test('기기를 누르면 영상이 나오고 첫 프레임·경로를 허브로 보낸다', async () => {
  await desk.locator(`[data-device="${deviceId}"]`).click();
  await desk.locator('.status').waitFor({ state: 'detached', timeout: 20000 });
  assert.ok((await desk.locator('video').evaluate((v) => v.videoWidth)) > 0, '영상 크기');
  await desk.getByText('직접 연결').waitFor();
  const s = await eventually(() => stats.find((m) => m.click_to_first_frame_ms > 0 && m.path === 'direct'), '계측');
  assert.equal(s.session_id, sessionOf(0));
  console.log(`첫 프레임 ${s.click_to_first_frame_ms}ms (로컬)`);
});

test('마우스·글자·한글·Enter·특수키 막대가 input 채널로 간다', async () => {
  const sid = sessionOf(0);
  const box = await desk.locator('video').boundingBox();
  await desk.mouse.click(box.x + box.width / 2, box.y + box.height / 2);
  const clicks = await eventually(async () => {
    const v = await inputs(desk, sid);
    return v.some((i) => i.t === 'mouse_button' && !i.down) && v;
  }, '클릭');
  const mv = clicks.filter((i) => i.t === 'mouse_move').at(-1);
  assert.ok(Math.abs(mv.x - 0.5) < 0.01 && Math.abs(mv.y - 0.5) < 0.01, JSON.stringify(mv));
  assert.deepEqual(
    clicks.filter((i) => i.t === 'mouse_button').map((i) => [i.button, i.down]),
    [['left', true], ['left', false]],
  );

  await desk.keyboard.type('ab');
  await desk.keyboard.insertText('안녕하세요 원격 테스트');
  await desk.keyboard.press('Enter');
  await desk.getByRole('button', { name: 'Esc' }).click();
  await desk.getByRole('button', { name: 'Ctrl' }).click();
  await desk.keyboard.type('c');
  const v = await eventually(async () => {
    const v = await inputs(desk, sid);
    return v.filter((i) => i.t === 'key').length >= 8 && v;
  }, '키');
  assert.equal(v.filter((i) => i.t === 'text').map((i) => i.text).join(''), 'ab안녕하세요 원격 테스트');
  assert.deepEqual(
    v.filter((i) => i.t === 'key').map((i) => [i.hid_usage, i.down]),
    [
      [HID.enter, true], [HID.enter, false],
      [HID.esc, true], [HID.esc, false],
      [HID.ctrl, true], [HID.c, true], [HID.c, false], [HID.ctrl, false],
    ],
  );
  const seqs = v.map((i) => i.seq);
  assert.deepEqual(seqs, [...seqs].sort((a, b) => a - b), '순번이 늘어난다');
});

test('폰에서 열면 앞 화면에 이어받기 안내, 폰의 탭은 가운데 클릭', async () => {
  phone = await viewerPage({ viewport: { width: 390, height: 844 }, hasTouch: true, isMobile: true });
  await phone.locator(`[data-device="${deviceId}"]`).tap();
  await desk.getByText('다른 기기에서 이 화면을 열었습니다').waitFor({ timeout: 15000 });
  assert.ok(toAgent.some((m) => m.t === 'session.end' && m.session_id === sessionOf(0) && m.reason === 'taken_over'));
  await phone.locator('.status').waitFor({ state: 'detached', timeout: 20000 });
  await phone.getByText('탭 = 클릭').waitFor();
  const box = await phone.locator('video').boundingBox();
  await phone.touchscreen.tap(box.x + box.width / 2, box.y + box.height / 2);
  const v = await eventually(async () => {
    const v = await inputs(phone, sessionOf(1));
    return v.some((i) => i.t === 'mouse_button' && !i.down) && v;
  }, '탭');
  assert.deepEqual(
    v.map((i) => (i.t === 'mouse_button' ? [i.button, i.down] : i.t === 'mouse_move' ? [i.x, i.y] : i.t)),
    [[0.5, 0.5], ['left', true], ['left', false]],
  );
  await phone.locator('.cursor').waitFor();
  await phone.locator('.hint').waitFor({ state: 'detached' });
  const end = await phone.getByRole('button', { name: '끊기' }).boundingBox();
  assert.ok(end.x + end.width <= 390, `끊기 버튼이 화면 밖: ${JSON.stringify(end)}`);
  await phone.screenshot({ path: join(OUT, 'remote-390.png') });
});

/** 호스트가 받은 text·Backspace 를 메모장처럼 다시 쳐 본 글. */
function replay(v) {
  let out = '';
  for (const i of v) {
    if (i.t === 'text') out += i.text;
    else if (i.t === 'key' && i.hid_usage === 0x2a && i.down) out = [...out].slice(0, -1).join('');
  }
  return out;
}

test('폰 가상 키보드: 한글 문장·Ctrl+C·F5 (SC-011)', async () => {
  const sid = sessionOf(1);
  const from = (await inputs(phone, sid)).length;
  const videoBox = () => phone.locator('video').boundingBox();
  const before = await videoBox();
  await phone.getByRole('button', { name: '가상 키보드' }).tap();
  const kb = phone.getByRole('group', { name: '가상 키보드' });
  await kb.waitFor();
  // 키보드는 화면 위에 덮인다 — PC 화면 크기·자리가 그대로(사용자 2026-11-24 「화면이 축소가 되네」).
  assert.deepEqual(await videoBox(), before, '키보드를 열어도 화면 크기 그대로');
  const press = (name) => kb.getByRole('button', { name, exact: true }).tap();
  await press('한글');
  // 정답지: 두벌식으로 「안녕하세요 원격 테스트」를 치는 자모 순서.
  for (const k of [...'ㅇㅏㄴㄴㅕㅇㅎㅏㅅㅔㅇㅛ', '띄어쓰기', ...'ㅇㅜㅓㄴㄱㅕㄱ', '띄어쓰기', ...'ㅌㅔㅅㅡㅌㅡ']) await press(k);
  const typed = await eventually(async () => {
    const v = (await inputs(phone, sid)).slice(from);
    return replay(v) === '안녕하세요 원격 테스트' && v;
  }, '한글 문장', 5000).catch(async () => assert.fail(replay((await inputs(phone, sid)).slice(from))));
  assert.ok(typed);
  await phone.screenshot({ path: join(OUT, 'remote-vkbd-ko-390.png') });
  await snapshot(phone, 'remote-vkbd');

  const mark = (await inputs(phone, sid)).length;
  await phone.getByRole('button', { name: 'Ctrl' }).tap();
  await press('ㅊ'); // c 자리 — Ctrl 을 잡았으면 글자 대신 C 키
  await press('기능키');
  await press('F5');
  const keys = await eventually(async () => {
    const v = (await inputs(phone, sid)).slice(mark).filter((i) => i.t === 'key');
    return v.length >= 6 && v;
  }, '단축키');
  assert.deepEqual(
    keys.map((i) => [i.hid_usage, i.down]),
    [[HID.ctrl, true], [HID.c, true], [HID.c, false], [HID.ctrl, false], [0x3e, true], [0x3e, false]],
  );

  // 키보드를 열어도 막대·키가 폰 폭 안에 있고 누를 만한 크기다.
  for (const b of await phone.locator('.vk, .keys .key').all()) {
    const r = await b.boundingBox();
    assert.ok(r.x >= 0 && r.x + r.width <= 390 && r.height >= 34, `${await b.textContent()}: ${JSON.stringify(r)}`);
  }
  // 키보드는 무대 위에 덮이고, 특수키 막대는 무대 아래에 그대로 있다.
  const stage = await phone.locator('.stage').boundingBox();
  const vk = await kb.boundingBox();
  const bar = await phone.locator('.keys').boundingBox();
  assert.ok(vk.y >= stage.y && vk.y + vk.height <= stage.y + stage.height + 0.5, '키보드가 무대 안에 덮인다');
  assert.ok(stage.y + stage.height <= bar.y + 0.5, '막대는 무대 아래');
  await phone.screenshot({ path: join(OUT, 'remote-vkbd-390.png') });
  await phone.getByRole('button', { name: '가상 키보드' }).tap();
  await kb.waitFor({ state: 'detached' });
});

test('가상 키보드 → 폰 자판으로 바꿔 쳐도 호스트 글이 지워지지 않는다', async () => {
  const sid = sessionOf(1);
  const from = (await inputs(phone, sid)).length;
  await phone.getByRole('button', { name: '가상 키보드' }).tap();
  const kb = phone.getByRole('group', { name: '가상 키보드' });
  for (const k of 'ab') await kb.getByRole('button', { name: k, exact: true }).tap();
  await phone.getByRole('button', { name: '더 보기' }).tap();
  await phone.getByRole('button', { name: /폰 자판 열기/ }).tap();
  await kb.waitFor({ state: 'detached' });
  await phone.keyboard.insertText('d');
  const v = await eventually(async () => {
    const v = (await inputs(phone, sid)).slice(from);
    return v.some((i) => i.t === 'text' && i.text === 'd') && v;
  }, '폰 자판 글');
  assert.equal(replay(v), 'abd');
});

test('폰 집기 확대 뒤 직접 터치 탭이 확대된 화면의 그 자리를 누른다', async () => {
  const sid = sessionOf(1);
  const st = await phone.locator('.stage').boundingBox();
  const [cx, cy] = [st.x + st.width / 2, st.y + st.height / 2];
  const cdp = await phone.context().newCDPSession(phone);
  const two = (d) => [{ x: cx - d, y: cy, id: 0 }, { x: cx + d, y: cy, id: 1 }];
  await cdp.send('Input.dispatchTouchEvent', { type: 'touchStart', touchPoints: two(20) });
  for (const d of [40, 60, 80, 100]) await cdp.send('Input.dispatchTouchEvent', { type: 'touchMove', touchPoints: two(d) });
  await cdp.send('Input.dispatchTouchEvent', { type: 'touchEnd', touchPoints: [] });
  await phone.getByRole('button', { name: /배 · 원래대로/ }).waitFor();
  const scale = await phone.evaluate(() => new DOMMatrix(getComputedStyle(document.querySelector('video')).transform).a);
  assert.ok(scale > 4, `확대 ${scale}`);

  await phone.getByRole('button', { name: '더 보기' }).tap();
  await phone.getByRole('button', { name: '직접 터치' }).tap();
  const mark = (await inputs(phone, sid)).length;
  const [px, py] = [cx + 30, cy + 12];
  // 기대값은 브라우저가 그린 영상 자리(getBoundingClientRect — 확대 반영)에서 따로 계산한다.
  const want = await phone.evaluate(([px, py]) => {
    const v = document.querySelector('video');
    const r = v.getBoundingClientRect();
    const k = Math.min(r.width / v.videoWidth, r.height / v.videoHeight);
    const [cw, ch] = [v.videoWidth * k, v.videoHeight * k];
    return [(px - r.left - (r.width - cw) / 2) / cw, (py - r.top - (r.height - ch) / 2) / ch];
  }, [px, py]);
  await phone.touchscreen.tap(px, py);
  const v = await eventually(async () => {
    const v = (await inputs(phone, sid)).slice(mark);
    return v.some((i) => i.t === 'mouse_button' && !i.down) && v;
  }, '직접 터치 탭');
  const mv = v.find((i) => i.t === 'mouse_move');
  assert.ok(Math.abs(mv.x - want[0]) < 0.002 && Math.abs(mv.y - want[1]) < 0.002, `${JSON.stringify(mv)} 기대 ${want}`);
  assert.ok(mv.x < 0.55, '확대를 빼고 계산하면 0.58 — 확대가 좌표에 반영됐다');
  await phone.screenshot({ path: join(OUT, 'remote-zoom-390.png') });
  await phone.getByRole('button', { name: /배 · 원래대로/ }).tap();
  await phone.getByRole('button', { name: /배 · 원래대로/ }).waitFor({ state: 'detached' });
});

test('끊기 → 에이전트에 viewer_closed, 목록으로', async () => {
  await phone.getByRole('button', { name: '끊기' }).click();
  await phone.getByRole('heading', { name: '내 기기' }).waitFor();
  await eventually(
    () => toAgent.some((m) => m.t === 'session.end' && m.session_id === sessionOf(1) && m.reason === 'viewer_closed'),
    'viewer_closed',
  );
});

test('?bench=3 — 저절로 3번 열고, 앞 2번은 닫고, 마지막은 열어 둔다 (SC-002 시험)', async () => {
  const n0 = toAgent.filter((m) => m.t === 'session.start').length;
  const page = await viewerPage();
  await page.goto(APP + '/?bench=3');
  await eventually(() => sessionOf(n0 + 2), '세 번째 열기', 60000);
  await page.locator('.status').waitFor({ state: 'detached', timeout: 20000 });
  await sleep(4000); // 남은 횟수가 0 이면 3초 뒤에도 닫지 않아야 한다
  const ended = (i) => toAgent.some((m) => m.t === 'session.end' && m.session_id === sessionOf(i) && m.reason === 'viewer_closed');
  assert.deepEqual([ended(n0), ended(n0 + 1), ended(n0 + 2)], [true, true, false]);
  assert.equal(sessionOf(n0 + 3), undefined, '네 번째 열기는 없어야');
  await page.context().close();
});

test('ui-check: 빈 목록·목록·기기 추가·가상 키보드 (1440·390)', { skip: !existsSync(UI_CHECK) && 'ui-check 없음' }, () => {
  for (const name of ['list-empty', 'list', 'add', 'remote-vkbd']) {
    try {
      const out = execFileSync('node', [UI_CHECK, join(OUT, `${name}.html`), '--out', join(OUT, 'shots')], { encoding: 'utf8' });
      assert.match(out, /ui-check passed/, out);
    } catch (e) {
      assert.fail(`${name}: ${e.stdout ?? e.message}`);
    }
  }
  console.log(`그림: ${OUT}`);
});
