param(
    [ValidateSet('Solo', 'Seamless')][string]$Mode = 'Solo',
    [string]$GameDir,
    [switch]$CheckOnly,
    [string]$PartyConfig
)
$ErrorActionPreference = 'Stop'
$pathFile = Join-Path $PSScriptRoot 'GamePath.txt'
if (-not $GameDir -and (Test-Path -LiteralPath $pathFile)) {
    $GameDir = [IO.File]::ReadAllText($pathFile).Trim()
}
if (-not $GameDir) {
    if ($CheckOnly) { throw 'Specify -GameDir or create GamePath.txt first.' }
    $GameDir = Read-Host 'Paste the Game folder path containing eldenring.exe'
}
$GameDir = $GameDir.Trim().Trim('"')
if (-not (Test-Path -LiteralPath (Join-Path $GameDir 'eldenring.exe') -PathType Leaf)) {
    throw 'Invalid GameDir: select the Game folder containing eldenring.exe.'
}
$GameDir = (Resolve-Path -LiteralPath $GameDir).Path
$launcher = if ($Mode -eq 'Seamless') { 'Launch-Seamless-Tower.ps1' } else { 'Launch-Preview.ps1' }
$entry = Join-Path $PSScriptRoot $launcher
$options = @{GameDir=$GameDir}
if ($PartyConfig) {
    if ($Mode -ne 'Seamless') {throw 'PartyConfig requires Seamless mode.'}
    $options.PartyConfig=$PartyConfig
}
$result = & $entry @options -CheckOnly
if ($CheckOnly) { $result; return }
[IO.File]::WriteAllText($pathFile, $GameDir, (New-Object Text.UTF8Encoding($true)))
& $entry @options
