param(
    [Alias("i")][string]$InputFolder = ".",
    [Alias("o")][string]$OutputFile = "merged.txt",
    [Alias("e")][string]$Extension = "*.txt"
)

$files = Get-ChildItem -Path $InputFolder -Filter $Extension
Write-Host "Merging $($files.Count) files from '$InputFolder' → '$OutputFile'"

$files | ForEach-Object {
    Write-Host "  + $($_.Name)"
    Get-Content $_.FullName
    ""
} | Set-Content $OutputFile

Write-Host "Done."
