#!/usr/bin/env pwsh
# Aggregates bigram pairs by combining AB + BA counts (order-independent pairs).

param(
    [string]$InputFile  = "$PSScriptRoot\..\data\stats\news-2014.bigrams.csv",
    [string]$OutputFile = "$PSScriptRoot\..\data\stats\news-2014.bigrams.aggregated.csv"
)

$rows = Import-Csv $InputFile

$agg = @{}

foreach ($row in $rows) {
    $pair    = $row.pair
    $rev     = -join $pair[-1..-($pair.Length)]
    $key     = if ($pair -le $rev) { $pair } else { $rev }

    if (-not $agg.ContainsKey($key)) {
        $agg[$key] = @{ count = 0L; raw = 0L }
    }
    $agg[$key].count += [long]$row.count
    $agg[$key].raw   += [long]$row.raw
}

$totalCount = ($agg.Values | Measure-Object -Property count -Sum).Sum
$totalRaw   = ($agg.Values | Measure-Object -Property raw   -Sum).Sum

$agg.GetEnumerator() |
    Sort-Object { $_.Value.count } -Descending |
    ForEach-Object {
        [pscustomobject]@{
            pair    = $_.Key
            count   = $_.Value.count
            '%'     = [math]::Round($_.Value.count / $totalCount * 100, 4)
            raw     = $_.Value.raw
            'raw%'  = [math]::Round($_.Value.raw   / $totalRaw   * 100, 4)
        }
    } |
    Export-Csv -Path $OutputFile -NoTypeInformation

Write-Host "Written $($agg.Count) aggregated pairs → $OutputFile"
