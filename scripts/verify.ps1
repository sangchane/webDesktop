# 품질 게이트 한 명령 (07 「검증·배포 명령」 1 — just 대신 이 스크립트). 하나라도 실패하면 종료 코드 1.
$ErrorActionPreference = 'Stop'
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
Set-Location (Split-Path $PSScriptRoot -Parent)
$steps = @(
  @{ name = 'fmt';          cmd = { cargo fmt --all -- --check } },
  @{ name = 'clippy-native';cmd = { cargo clippy -q -p proto --all-targets -- -D warnings } },
  @{ name = 'clippy-hub';   cmd = { cargo clippy -q -p hub -p app --target wasm32-unknown-unknown -- -D warnings } },
  @{ name = 'test';         cmd = { cargo test -q -p proto } },
  # test 가 web/src/proto 를 다시 만든다 → 커밋(또는 스테이징)한 것과 달라지면 실패 (바닥선 15).
  @{ name = 'proto-gen';    cmd = { git diff --quiet -- web/src/proto; if ($LASTEXITCODE -eq 0 -and (git ls-files --others --exclude-standard web/src/proto)) { $global:LASTEXITCODE = 1 } } },
  @{ name = 'tsc';          cmd = { npx tsc -p web } },
  @{ name = 'deny';         cmd = { cargo deny --log-level error check licenses bans } }
)
foreach ($s in $steps) {
  & $s.cmd
  if ($LASTEXITCODE -ne 0) { Write-Host "VERIFY FAIL: $($s.name)"; exit 1 }
}
Write-Host 'VERIFY OK'
