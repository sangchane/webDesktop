# 품질 게이트 한 명령 (07 「검증·배포 명령」 1 — just 대신 이 스크립트). 하나라도 실패하면 종료 코드 1.
$ErrorActionPreference = 'Stop'
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
Set-Location (Split-Path $PSScriptRoot -Parent)
$steps = @(
  @{ name = 'fmt';          cmd = { cargo fmt --all -- --check } },
  @{ name = 'clippy-native';cmd = { cargo clippy -q -p proto --all-targets -- -D warnings } },
  @{ name = 'clippy-hub';   cmd = { cargo clippy -q -p hub --target wasm32-unknown-unknown -- -D warnings } },
  @{ name = 'test';         cmd = { cargo test -q -p proto } },
  @{ name = 'deny';         cmd = { cargo deny --log-level error check licenses bans } }
)
foreach ($s in $steps) {
  & $s.cmd
  if ($LASTEXITCODE -ne 0) { Write-Host "VERIFY FAIL: $($s.name)"; exit 1 }
}
Write-Host 'VERIFY OK'
