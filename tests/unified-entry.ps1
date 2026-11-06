param([Parameter(Mandatory=$true)][string]$PackageRoot, [Parameter(Mandatory=$true)][string]$GameDir)
$ErrorActionPreference = 'Stop'
$testRoot = Join-Path ([IO.Path]::GetTempPath()) ('tower-unified-'+[guid]::NewGuid().ToString('N'))
$game = Join-Path $testRoot 'Game'
New-Item -ItemType Directory -Path $game -Force | Out-Null
Copy-Item -LiteralPath (Join-Path $GameDir 'eldenring.exe') -Destination (Join-Path $game 'eldenring.exe')
try {
    foreach ($loader in @('ME3','YAFSML')) {
        foreach ($legacyMode in @('Solo','Seamless')) {
            $result = & (Join-Path $PackageRoot 'Start-Tower.ps1') -GameDir $game -Loader $loader -Mode $legacyMode -CheckOnly
            if (!$result -or $result.SaveExtension -ne 'ertowercoop') {throw 'Unified entry did not preserve the co-op save extension'}
            $expected = if ($loader -eq 'ME3') {'me3.exe'} else {'YAFSML.exe'}
            if ((Split-Path -Leaf $result.Entry) -ne $expected) {throw 'Loader selection was lost'}
        }
    }
    if (@(Get-ChildItem -LiteralPath $game).Count -ne 1) {throw 'Launch preflight modified the game directory'}
    Write-Output 'PASS: both loaders and legacy modes use the bundled Seamless entry without external Seamless files.'
} finally {
    [IO.File]::Delete((Join-Path $game 'eldenring.exe'))
    [IO.Directory]::Delete($game)
    [IO.Directory]::Delete($testRoot)
}
