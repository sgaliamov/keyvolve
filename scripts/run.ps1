param (
    [switch]
    [alias('d')]
    $debug
)

$ErrorActionPreference = 'Stop'

if ($debug) {
    $Env:RUST_BACKTRACE = "full"
    $Env:RAYON_NUM_THREADS = 1

    cargo build
    Clear-Host
    cargo run
}
else {
    # Set `BelowNormal` after the application started to be able to stop it with Ctrl+C.
    Start-Job -ScriptBlock {
        while ($true) {
            $process = Get-Process -Name "keyvolve" -ErrorAction SilentlyContinue
            if ($process) {
                $process.PriorityClass = "BelowNormal"
                break
            }
            Start-Sleep -Milliseconds 10000
        }
    } | Out-Null

    $Env:RUST_BACKTRACE = 0
    $Env:RAYON_NUM_THREADS = 0

    cargo build --release
    Clear-Host

    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    cargo run --release

    $sw.Stop()
    $minutes = [int][Math]::Floor($sw.Elapsed.TotalMinutes)
    $seconds = $sw.Elapsed.Seconds
    $milliseconds = $sw.Elapsed.Milliseconds

    Write-Host ('Execution time: {0:D2}:{1:D2}:{2:D3}' -f $minutes, $seconds, $milliseconds)
}

