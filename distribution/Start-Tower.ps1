param(
    [ValidateSet('Solo', 'Seamless')][string]$Mode = 'Seamless',
    [ValidateSet('ME3', 'YAFSML')][string]$Loader = 'ME3',
    [string]$GameDir,
    [switch]$CheckOnly,
    [string]$PartyConfig,
    # Launcher use: write the failure message here so it can be shown without a console window
    [string]$ErrorFile
)
$ErrorActionPreference = 'Stop'
trap {
    if ($ErrorFile) { [IO.File]::WriteAllText($ErrorFile, $_.Exception.Message, (New-Object Text.UTF8Encoding($false))) }
    exit 1
}
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
$launcher = 'Launch-Seamless-Tower.ps1'
$entry = Join-Path $PSScriptRoot $launcher
$options = @{GameDir=$GameDir;Loader=$Loader}
if ($PartyConfig) {
    $options.PartyConfig=$PartyConfig
}
$result = & $entry @options -CheckOnly
if ($CheckOnly) { $result; return }
[IO.File]::WriteAllText($pathFile, $GameDir, (New-Object Text.UTF8Encoding($true)))
& $entry @options
