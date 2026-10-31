$ErrorActionPreference = 'Stop'

Clear-Host

$Env:RUST_BACKTRACE = 1
cargo test --no-fail-fast --workspace --message-format short --doc --offline -q
$failed = $LASTEXITCODE -ne 0
cargo test --no-fail-fast --workspace --message-format short --all-targets --offline -q
$failed = $failed -or $LASTEXITCODE -ne 0
$Env:RUST_BACKTRACE = 0
if ($failed) { exit 1 }
