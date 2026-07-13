# Merge layout CSV files into one, keeping only keys_1..keys_6 and name columns.
# Usage: ./scripts/merge-layouts.ps1 [-Pattern <glob>] [-Out <path>]
param(
    [string]$Pattern = "data/layouts*.csv",
    [string]$Out = "data/m-layouts.csv"
)

$cols = @('keys_1', 'keys_2', 'keys_3', 'keys_4', 'keys_5', 'keys_6', 'name')

Get-ChildItem $Pattern |
    ForEach-Object { Import-Csv $_ } |
    ForEach-Object {
        # trim spaces in headers/values (seed.csv has padded header)
        $row = [ordered]@{}
        foreach ($p in $_.PSObject.Properties) { $row[$p.Name.Trim()] = "$($p.Value)".Trim() }
        [pscustomobject]$row
    } |
    Select-Object $cols |
    Export-Csv $Out -NoTypeInformation

Write-Host "wrote $Out"
