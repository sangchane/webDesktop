// T6 / SC C3: 웹앱은 Access 로그인 없이는 막히고, 허브의 공개 경로는 열려 있는지 확인한다.
const APP = 'https://webdesktop-app.sangchane87.workers.dev';
const HUB = 'https://webdesktop-hub.sangchane87.workers.dev';
const checks = [
  ['app / 는 Access 로그인으로 보냄', `${APP}/`, (r) => r.status === 302 && /cloudflareaccess\.com/.test(r.headers.get('location') ?? '')],
  ['app /api/devices 는 Access 로그인으로 보냄', `${APP}/api/devices`, (r) => r.status === 302 && /cloudflareaccess\.com/.test(r.headers.get('location') ?? '')],
  ['hub /healthz 는 공개', `${HUB}/healthz`, (r) => r.status === 200],
  ['hub /api/* 는 받지 않음', `${HUB}/api/devices`, (r) => r.status === 404],
  // /agent/* 는 로그인 없이 허브가 직접 답한다(Access 화면이 아니라 Problem JSON).
  ['hub /agent/enroll 은 공개(꼴 검사 400)', [`${HUB}/agent/enroll`, { method: 'POST', body: '{}' }], (r) => r.status === 400],
  ['hub /agent/install 은 공개(토큰 거절 400)', `${HUB}/agent/install/windows?t=x`, (r) => r.status === 400],
];
let fail = 0;
for (const [name, url, ok] of checks) {
  const [u, init] = Array.isArray(url) ? url : [url, {}];
  const r = await fetch(u, { redirect: 'manual', ...init });
  const pass = ok(r);
  if (!pass) fail++;
  console.log(`${pass ? 'PASS' : 'FAIL'} ${name} (${r.status})`);
}
console.log(fail ? `ACCESS FAIL ${fail}` : 'ACCESS OK');
process.exit(fail ? 1 : 0);
