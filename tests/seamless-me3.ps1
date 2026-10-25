param([string]$GameDir = 'F:\SteamLibrary\steamapps\common\ELDEN RING\Game')
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
foreach ($location in @($root,(Join-Path $root 'release-preview'))) {
    $script = Join-Path $location 'Launch-Seamless-Tower.ps1'
    $result = & $script -GameDir $GameDir -CheckOnly
    if ((Split-Path $result.Entry -Leaf) -ne 'me3.exe') {throw 'Wrong loader'}
    if ($result.SaveExtension -ne 'ertowercoop') {throw 'Save extension changed'}
    $source = [IO.File]::ReadAllText($script)
    if ($source -notmatch 'load_after = \[\{ id = "ersc.dll", optional = false \}\]') {throw 'Missing native dependency'}
    if ($source -match 'Copy-Item -LiteralPath \$bootstrap|WriteAllText\(\$settingsPath|MODENGINE_CONFIG') {throw 'Old loader mutation remains'}
}
Write-Output 'ME3 Seamless launcher checks passed.'
