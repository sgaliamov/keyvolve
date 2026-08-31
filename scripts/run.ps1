param (
    [switch]
    [alias('d')]
    $dev,
    [ValidateRange(1, 2147483647)]
    [alias('r')]
    [int]
    $repeat = 1,
    [Parameter(ValueFromRemainingArguments=$true)]
    [string[]]
    $AppArgs
)

$ErrorActionPreference = 'Stop'

function Invoke-SequentialRun {
    param (
        [string[]]
        $CargoArgs,
        [switch]
        $BelowNormal
    )

    for ($i = 1; $i -le $repeat; $i++) {
        if ($repeat -gt 1) {
            Write-Host ("Run {0}/{1}" -f $i, $repeat)
        }

        if ($BelowNormal) {
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
        }

        & cargo @CargoArgs
    }
}

if ($dev) {
    $Env:RUST_BACKTRACE = "full"
    $Env:RAYON_NUM_THREADS = 1

    cargo build
    Clear-Host

    $cargoArgs = @('run')
    if ($AppArgs -and $AppArgs.Count -gt 0) { $cargoArgs += '--'; $cargoArgs += $AppArgs }
    Invoke-SequentialRun -CargoArgs $cargoArgs
}
else {
    $Env:RUST_BACKTRACE = 0
    $Env:RAYON_NUM_THREADS = 0

    cargo build --release
    Clear-Host

    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    $cargoArgs = @('run','--release')
    if ($AppArgs -and $AppArgs.Count -gt 0) { $cargoArgs += '--'; $cargoArgs += $AppArgs }
    Invoke-SequentialRun -CargoArgs $cargoArgs -BelowNormal

    $sw.Stop()
    $minutes = [int][Math]::Floor($sw.Elapsed.TotalMinutes)
    $seconds = $sw.Elapsed.Seconds
    $milliseconds = $sw.Elapsed.Milliseconds

    $label = if ($repeat -gt 1) { 'Total execution time' } else { 'Execution time' }
    Write-Host ('{0}: {1:D2}:{2:D2}:{3:D3}' -f $label, $minutes, $seconds, $milliseconds)
}
