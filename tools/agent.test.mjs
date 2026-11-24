// 에이전트 시험 (tasks T8) — 실제 에이전트 실행 파일을 로컬 허브에 등록하고 목록에서 「켜짐」을 본다.
// 실행: node --test tools/agent.test.mjs  (처음엔 Worker·에이전트를 빌드하느라 몇 분 걸린다)
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import { spawn, execSync } from 'node:child_process';
import { mkdtempSync, readFileSync, statSync, rmSync } from 'node:fs';
import { tmpdir, homedir } from 'node:os';
import { join } from 'node:path';
import { ROOT, HUB, OFFLINE, api, startStack, stopStack } from './stack.mjs';

const EXE = join(ROOT, 'target/debug/webdesktop-agent.exe');
const HOME = mkdtempSync(join(tmpdir(), 'webdesktop-agent-'));
const running = [];

before(async () => {
  execSync('cargo build -q -p agent', { cwd: ROOT, stdio: 'inherit', env: { ...process.env, PATH: `${join(homedir(), '.cargo', 'bin')};${process.env.PATH}` } });
  await startStack();
});
after(() => {
  for (const a of running) a.p.kill();
  stopStack();
  rmSync(HOME, { recursive: true, force: true });
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
