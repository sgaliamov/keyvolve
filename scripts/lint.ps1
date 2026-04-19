$ErrorActionPreference = 'Stop'

Clear-Host

cargo fix --edition-idioms --allow-dirty
cargo clippy --fix --allow-dirty --no-deps
cargo clippy
cargo fmt
