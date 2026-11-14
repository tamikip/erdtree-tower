param([Parameter(Mandatory=$true)][string]$PatchDir, [Parameter(Mandatory=$true)][string]$WorkDir)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot '../distribution/Tower-Network.ps1')
$fixture = Join-Path $WorkDir ('network-test-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $fixture | Out-Null
foreach ($name in @('OnlineFix64.dll','steam_api64.dll','winmm.dll','OnlineFix.ini','dlllist.txt')) {
    Copy-Item -LiteralPath (Join-Path $PatchDir $name) -Destination $fixture
}
function Must-Fail([scriptblock]$Action) {
    $failed = $false
    try { & $Action } catch { $failed = $true }
    if (!$failed) { throw 'Expected rejection.' }
}
$list = Join-Path $fixture 'dlllist.txt'
$before = [IO.File]::ReadAllBytes($list)
Assert-TowerNetwork $fixture 'OnlineFix'
Must-Fail { Assert-TowerNetwork $fixture 'Steam' }
if ([Convert]::ToBase64String($before) -cne [Convert]::ToBase64String([IO.File]::ReadAllBytes($list))) { throw 'Check changed dlllist.' }
# Test the running-game guard without interacting with a real process.
function Get-Process { param($Name,$ErrorAction); [pscustomobject]@{Id=1} }
Must-Fail { Set-TowerOnlineFixDllList $fixture }
function Get-Process { param($Name,$ErrorAction) }
Set-TowerOnlineFixDllList $fixture
if ([IO.File]::ReadAllText($list).Trim() -cne 'OnlineFix64.dll') { throw 'Duplicate Seamless loader not removed.' }
if ([Convert]::ToBase64String($before) -cne [Convert]::ToBase64String([IO.File]::ReadAllBytes($list + '.before-tower.bak'))) { throw 'Backup mismatch.' }
Set-TowerOnlineFixDllList $fixture
Assert-TowerNetwork $fixture 'OnlineFix'
[IO.File]::WriteAllText($list,"OnlineFix64.dll`r`nunknown.dll`r`n")
Must-Fail { Assert-TowerNetwork $fixture 'OnlineFix' }
[IO.File]::WriteAllBytes($list,$before)
$iniPath = Join-Path $fixture 'OnlineFix.ini'
$ini = [IO.File]::ReadAllText($iniPath)
[IO.File]::WriteAllText($iniPath,$ini.Replace('FakeAppId=480','FakeAppId=1245620'))
Must-Fail { Assert-TowerNetwork $fixture 'OnlineFix' }
[IO.File]::WriteAllText($iniPath,$ini)
[IO.File]::WriteAllText((Join-Path $fixture 'steam_api64_o.dll'),'mixed proxy')
Must-Fail { Assert-TowerNetwork $fixture 'OnlineFix' }
Remove-Item -LiteralPath (Join-Path $fixture 'steam_api64_o.dll')
[IO.File]::WriteAllText((Join-Path $fixture 'OnlineFix64.dll'),'unsupported component')
Must-Fail { Assert-TowerNetwork $fixture 'OnlineFix' }
Write-Output 'PASS: known patch hashes, separate modes, no-op preflight, running-game guard, one Seamless load, backup, idempotence, malformed list, wrong App ID and mixed proxies.'
