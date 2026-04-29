param (
    [switch]
    [alias('u')]
    $upgrade
)

$ErrorActionPreference = 'Stop'

Clear-Host

rustup update

if ($upgrade) {
    # run `cargo install cargo-edit` to have this command
    cargo upgrade
}

cargo update
