param([string]$GameDir = 'F:\SteamLibrary\steamapps\common\ELDEN RING\Game')
$ErrorActionPreference = 'Stop'
$exePath = Join-Path $GameDir 'eldenring.exe'
if (-not (Test-Path -LiteralPath $exePath -PathType Leaf)) { throw '找不到 eldenring.exe' }
$gameExe = Get-Item -LiteralPath $exePath
$version = $gameExe.VersionInfo.FileVersion
$knownHash = 'f9118fea2cfd8fa3901e8060bfadb1552cb2471771ded8a5b662c4e298ffb328'
$actualHash = (Get-FileHash -LiteralPath $exePath -Algorithm SHA256).Hash.ToLowerInvariant()
[pscustomobject]@{
    GameDirectory = $GameDir
    ExecutableVersion = $version
    ExecutableSHA256 = $actualHash
    MatchesReferenced2720Executable = ($version -eq '2.7.2.0' -and $actualHash -eq $knownHash)
    ModEngine2LauncherPresent = (Test-Path -LiteralPath (Join-Path $GameDir 'modengine2_launcher.exe'))
    ModEngine2RuntimePresent = (Test-Path -LiteralPath (Join-Path $GameDir 'modengine2') -PathType Container)
    GameRunning = (@(Get-Process -Name eldenring -ErrorAction SilentlyContinue).Count -gt 0)
    GameplayIntegrationVerified = $false
} | ConvertTo-Json
