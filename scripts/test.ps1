$ErrorActionPreference = 'Stop'

Clear-Host

$Env:RUST_BACKTRACE = 1
cargo test --no-fail-fast --workspace --message-format short --doc --offline -q
cargo test --no-fail-fast --workspace --message-format short --all-targets --offline -q
$Env:RUST_BACKTRACE = 0
