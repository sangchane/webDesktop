// 에이전트 시험 (tasks T8·T9) — 실제 에이전트 실행 파일을 로컬 허브에 등록하고 목록에서 「켜짐」을 본다.
// T9: 시험 브라우저가 뷰어가 되어 이 PC 의 실제 화면(DXGI → H.264)을 받고, 입력 채널로 보낸 입력에 input.applied 가 오는지 본다.
// 주의: 에이전트는 이 PC 에 진짜 입력을 넣는다 — 시험은 호스트에 효과가 없는 키(HID 0x90, 표에 없음)만 보낸다.
// 실행: node --test tools/agent.test.mjs  (처음엔 Worker·에이전트를 빌드하느라 몇 분 걸린다)
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import { spawn, execSync } from 'node:child_process';
import { mkdtempSync, readFileSync, readdirSync, statSync, rmSync } from 'node:fs';
import { tmpdir, homedir } from 'node:os';
import { join } from 'node:path';
import { chromium } from 'playwright';
import { ROOT, HUB, OFFLINE, api, open, until, startStack, stopStack } from './stack.mjs';

// AGENT_PROFILE=release 면 배포판으로 잰다(지연은 배포판 기준 — 디버그는 인코딩이 몇 배 느리다).
const PROFILE = process.env.AGENT_PROFILE === 'release' ? 'release' : 'debug';
const EXE = join(ROOT, `target/${PROFILE}/webdesktop-agent.exe`);
const HOME = mkdtempSync(join(tmpdir(), 'webdesktop-agent-'));
const running = [];
let browser;

before(async () => {
  execSync(`cargo build -q -p agent${PROFILE === 'release' ? ' --release' : ''}`, { cwd: ROOT, stdio: 'inherit', env: { ...process.env, PATH: `${join(homedir(), '.cargo', 'bin')};${process.env.PATH}` } });
  await startStack();
});
after(async () => {
  await browser?.close();
  for (const a of running) a.p.kill();
  stopStack();
  // KEEP_AGENT_HOME=1 이면 에이전트 로그를 남겨 둔다(지연 단계 시간 보기).
  if (process.env.KEEP_AGENT_HOME) console.log(`에이전트 폴더 ${HOME}`);
  else rmSync(HOME, { recursive: true, force: true });
});

function agent() {
  const p = spawn(EXE, [], { env: { ...process.env, WEBDESKTOP_HOME: HOME, WEBDESKTOP_HUB: HUB }, stdio: ['pipe', 'pipe', 'pipe'] });
  const a = { p, out: '' };
  p.stdout.on('data', (d) => (a.out += d));
  p.stderr.on('data', (d) => (a.out += d));
  running.push(a);
  return a;
}

async function waitFor(what, pred, ms = 20_000) {
  const end = Date.now() + ms;
  for (;;) {
    const v = await pred();
    if (v) return v;
    if (Date.now() > end) throw new Error(`기다림 초과: ${what}\n에이전트 출력:\n${running.map((a) => a.out).join('\n---\n')}`);
    await new Promise((r) => setTimeout(r, 300));
  }
}

const online = async () => (await api('/api/devices')).body.find((d) => d.online);

let first;

test('코드 입력 → 등록 → 목록 켜짐 (틀린 코드는 다시 물음, 소문자·붙임표 받음)', async () => {
  const { code } = (await api('/api/enroll-codes', { method: 'POST' })).body;
  first = agent();
  await waitFor('코드 물음', () => first.out.includes('등록 코드'));
  first.p.stdin.write('nope\n');
  await waitFor('꼴 안내', () => first.out.includes('숫자·영문 8자리'));
  // 코드 글자(0·1·I·O 없음)로만 쓴 틀린 코드여야 허브까지 간다.
  first.p.stdin.write('ZZZZ2345\n');
  await waitFor('틀린 코드 안내', () => first.out.includes('코드가 맞지 않습니다'));
  first.p.stdin.write(`${code.slice(0, 4).toLowerCase()}-${code.slice(4)}\n`);
  const d = await waitFor('목록 켜짐', online);
  assert.equal(d.os, 'windows');
  assert.ok(first.out.includes('등록했습니다'), first.out);
  const cfg = JSON.parse(readFileSync(join(HOME, 'config.json'), 'utf8'));
  assert.equal(cfg.device_id, d.id);
  assert.equal(cfg.hub_url, HUB);
  // 키는 DPAPI 로 잠긴 덩어리 — 씨앗 32바이트 그대로가 아니다.
  assert.ok(statSync(join(HOME, 'device.key')).size > 100);
});

test('허브가 조용한 연결로 보고 닫으면 다시 붙어 켜짐', async () => {
  await api('/api/devices', { now: Date.now() + OFFLINE + 5_000 });
  await waitFor('다시 붙음 안내', () => first.out.includes('다시 붙습니다'));
  await waitFor('다시 켜짐', online);
});

test('다시 실행하면 코드 없이 저장한 키로 붙음, 기기는 하나', async () => {
  first.p.kill();
  await waitFor('꺼짐', async () => !(await online()));
  const again = agent();
  again.p.stdin.end();
  await waitFor('다시 켜짐', online);
  assert.ok(!again.out.includes('등록 코드'), again.out);
  assert.equal((await api('/api/devices')).body.length, 1);
});

test('실제 화면: 뷰어 offer → 에이전트 answer → H.264 프레임, 입력 → input.applied', async (t) => {
  const dev = (await api('/api/devices')).body[0];
  const viewer_id = 'viewer-e2e';
  const tk = (await api('/api/ws-ticket', { method: 'POST', body: { viewer_id } })).body;
  const v = open(`${tk.hub_ws_url}?ticket=${tk.ticket}`);
  await v.opened;
  v.ws.send(JSON.stringify({ t: 'viewer.hello', viewer_id, proto: 1 }));
  const s = (await api('/api/sessions', { method: 'POST', body: { device_id: dev.id, viewer_id } })).body;
  assert.ok(s.session_id, JSON.stringify(s));

  browser = await chromium.launch();
  const page = await browser.newPage();
  await page.exposeFunction('toHub', (m) => v.ws.send(JSON.stringify(m)));
  // 웹앱(session.rs)과 같은 꼴: recvonly 영상 + input·control 채널, 뷰어가 offer. 같은 PC 라 TURN 없이 호스트 후보로.
  const offer = await page.evaluate(async (sid) => {
    const pc = (window.pc = new RTCPeerConnection());
    window.applied = [];
    pc.addTransceiver('video', { direction: 'recvonly' });
    window.input = pc.createDataChannel('input');
    pc.createDataChannel('control').onmessage = (e) => window.applied.push(JSON.parse(e.data));
    pc.onicecandidate = (e) =>
      e.candidate?.candidate && window.toHub({ t: 'signal.candidate', session_id: sid, candidate: e.candidate.candidate });
    pc.ontrack = (e) => {
      const el = Object.assign(document.createElement('video'), { muted: true, srcObject: new MediaStream([e.track]) });
      document.body.append(el);
      el.play();
    };
    await pc.setLocalDescription(await pc.createOffer());
    return pc.localDescription.sdp;
  }, s.session_id);
  v.ws.send(JSON.stringify({ t: 'signal.offer', session_id: s.session_id, sdp: offer }));
  const answer = await until(v, (m) => m.t === 'signal.answer', 20_000);
  await page.evaluate((sdp) => window.pc.setRemoteDescription({ type: 'answer', sdp }), answer.sdp);
  // 뷰어 jitter 버퍼를 최소로 — 에이전트가 playout-delay 확장을 받아 준다(지연 개선 2026-11-24).
  assert.match(answer.sdp, /playout-delay/, 'answer 에 playout-delay 확장');

  const agentLog = () => readdirSync(join(HOME, 'logs')).map((f) => readFileSync(join(HOME, 'logs', f), 'utf8')).join('');
  const video = await waitFor(
    '영상 프레임 디코딩',
    () =>
      page.evaluate(async () => {
        const r = await window.pc.getStats();
        for (const x of r.values()) {
          if (x.type === 'inbound-rtp' && x.kind === 'video' && x.framesDecoded > 0 && x.frameWidth) {
            return { w: x.frameWidth, h: x.frameHeight, codec: r.get(x.codecId)?.mimeType };
          }
        }
        return null;
      }),
    30_000,
  ).catch((e) => {
    // 화면이 잠겼거나 디스플레이가 꺼져 있으면 이 PC 에서는 캡처를 못 연다 — 실패가 아니라 건너뜀.
    if (agentLog().includes('capture.open_failed')) return null;
    throw e;
  });
  if (!video) return t.skip('이 PC 화면을 캡처하지 못함(잠김·디스플레이 꺼짐) — 에이전트 로그 capture.open_failed');
  assert.equal(video.codec, 'video/H264');
  // 그림이 실제 화면인지(검은·한 색 화면이 아닌지) — GPU 색 변환이 깨지면 여기서 걸린다. 그림 파일은 눈으로 볼 때.
  const pic = await page.evaluate(() => {
    const v = document.querySelector('video');
    const c = Object.assign(document.createElement('canvas'), { width: 160, height: 90 });
    const g = c.getContext('2d');
    g.drawImage(v, 0, 0, 160, 90);
    const d = g.getImageData(0, 0, 160, 90).data;
    const ys = [];
    for (let i = 0; i < d.length; i += 4) ys.push((d[i] + d[i + 1] + d[i + 2]) / 3);
    const mean = ys.reduce((a, b) => a + b, 0) / ys.length;
    return { mean: Math.round(mean), sd: Math.round(Math.sqrt(ys.reduce((a, y) => a + (y - mean) ** 2, 0) / ys.length)) };
  });
  await page.locator('video').screenshot({ path: join(HOME, 'frame.png') });
  console.log(`화면 밝기 평균 ${pic.mean} · 퍼짐 ${pic.sd} · 그림 ${join(HOME, 'frame.png')}`);
  assert.ok(pic.sd > 5, `한 색 화면: ${JSON.stringify(pic)}`);
  assert.ok(video.w >= 640 && video.w <= 2560 && video.h >= 360, JSON.stringify(video)); // VIDEO_MAX_EDGE

  await page.evaluate(() => window.input.readyState === 'open' || new Promise((r) => (window.input.onopen = r)));
  // 화면이 안 바뀌는 입력도 한두 프레임 안에 반영 프레임이 나간다(1초 재전송을 기다리지 않음). 다섯 번 재서 가장 느린 값.
  let slowest = 0;
  let applied;
  for (let seq = 7; seq < 12; seq++) {
    const r = await page.evaluate(async (seq) => {
      await new Promise((ok) => setTimeout(ok, 300));
      const t0 = performance.now();
      window.input.send(JSON.stringify({ t: 'key', seq, hid_usage: 0x90, down: true }));
      for (;;) {
        const a = window.applied.find((a) => a.seq === seq);
        if (a) return { a, ms: performance.now() - t0 };
        if (performance.now() - t0 > 5000) return null;
        await new Promise((ok) => setTimeout(ok, 5));
      }
    }, seq);
    assert.ok(r, `input.applied seq ${seq} 안 옴`);
    applied = r.a;
    slowest = Math.max(slowest, Math.round(r.ms));
    console.log(`input.applied seq ${seq}: ${Math.round(r.ms)}ms`);
  }
  assert.equal(applied.t, 'input.applied');
  assert.ok(Number.isInteger(applied.rtp_ts));
  // 1초 재전송을 기다리던 잘못(최대 1000ms)이 돌아오지 않게. 보통 70~130ms, 가끔 230ms 안팎(30장/초·CPU 인코딩 — 2026-11-24).
  if (PROFILE === "release") assert.ok(slowest < 400, `input.applied 가장 느림 ${slowest}ms`);

  v.ws.send(JSON.stringify({ t: 'session.end', session_id: s.session_id, reason: 'viewer_closed' }));
  console.log(`실제 화면 ${video.w}x${video.h} ${video.codec}, input.applied 가장 느림 ${slowest}ms`);
});

test('같은 기기로 두 번째 에이전트가 붙으면 앞 에이전트가 끝남', async () => {
  const [old] = running.slice(-1);
  const twin = agent();
  twin.p.stdin.end();
  const code = await Promise.race([
    new Promise((r) => old.p.once('exit', r)),
    new Promise((_, j) => setTimeout(() => j(new Error(`앞 에이전트가 끝나지 않음
${old.out}`)), 20_000)),
  ]);
  assert.equal(code, 0, old.out);
  assert.ok(old.out.includes('이 에이전트를 끝냅니다'), old.out);
  await waitFor('켜짐 유지', online);
});
