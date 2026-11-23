// T6 / SC C3: 웹앱은 Access 로그인 없이는 막히고, 허브의 공개 경로는 열려 있는지 확인한다.
const APP = 'https://webdesktop-app.sangchane87.workers.dev';
const HUB = 'https://webdesktop-hub.sangchane87.workers.dev';
const checks = [
  ['app / 는 Access 로그인으로 보냄', `${APP}/`, (r) => r.status === 302 && /cloudflareaccess\.com/.test(r.headers.get('location') ?? '')],
  ['app /api/devices 는 Access 로그인으로 보냄', `${APP}/api/devices`, (r) => r.status === 302 && /cloudflareaccess\.com/.test(r.headers.get('location') ?? '')],
  ['hub /healthz 는 공개', `${HUB}/healthz`, (r) => r.status === 200],
  ['hub /api/* 는 받지 않음', `${HUB}/api/devices`, (r) => r.status === 404],
];
let fail = 0;
for (const [name, url, ok] of checks) {
  const r = await fetch(url, { redirect: 'manual' });
  const pass = ok(r);
  if (!pass) fail++;
  console.log(`${pass ? 'PASS' : 'FAIL'} ${name} (${r.status})`);
}
console.log(fail ? `ACCESS FAIL ${fail}` : 'ACCESS OK');
process.exit(fail ? 1 : 0);
