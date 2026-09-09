$ErrorActionPreference = 'Stop'

$dev = $false
$repeat = 1
$AppArgs = [System.Collections.Generic.List[string]]::new()

for ($i = 0; $i -lt $args.Count; $i++) {
    $arg = [string]$args[$i]
    $next = if ($i + 1 -lt $args.Count) { [string]$args[$i + 1] } else { $null }

    if ($arg -eq '--') {
        for ($j = $i + 1; $j -lt $args.Count; $j++) {
            $AppArgs.Add([string]$args[$j])
        }
        break
    }

    if ($arg -in @('-d', '-dev')) {
        $dev = $true
        continue
    }

    if ($arg -match '^-r(?:epeat)?[:=](.+)$') {
        $repeat = [int]$Matches[1]
        if ($repeat -lt 1) {
            throw "repeat must be at least 1."
        }
        continue
    }

    if ($arg -in @('-r', '-repeat')) {
        if ($null -eq $next) {
            throw "repeat requires a value."
        }

        $repeat = [int]$next
        if ($repeat -lt 1) {
            throw "repeat must be at least 1."
        }
        $i++
        continue
    }

    while ($i + 1 -lt $args.Count -and ([string]$args[$i + 1]).StartsWith('.')) {
        $arg += [string]$args[$i + 1]
        $i++
    }

    $AppArgs.Add($arg)
}

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
    if ($AppArgs.Count -gt 0) { $cargoArgs += '--'; $cargoArgs += $AppArgs }
    Invoke-SequentialRun -CargoArgs $cargoArgs
}
else {
    $Env:RUST_BACKTRACE = 0
    $Env:RAYON_NUM_THREADS = 0

    cargo build --release
    Clear-Host

    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    $cargoArgs = @('run','--release')
    if ($AppArgs.Count -gt 0) { $cargoArgs += '--'; $cargoArgs += $AppArgs }
    Invoke-SequentialRun -CargoArgs $cargoArgs -BelowNormal

    $sw.Stop()
    $minutes = [int][Math]::Floor($sw.Elapsed.TotalMinutes)
    $seconds = $sw.Elapsed.Seconds
    $milliseconds = $sw.Elapsed.Milliseconds

    $label = if ($repeat -gt 1) { 'Total execution time' } else { 'Execution time' }
    Write-Host ('{0}: {1:D2}:{2:D2}:{3:D3}' -f $label, $minutes, $seconds, $milliseconds)
}
