param([string]$GameDir = 'F:\SteamLibrary\steamapps\common\ELDEN RING\Game', [switch]$CheckOnly)
$ErrorActionPreference = 'Stop'
$exePath = Join-Path $GameDir 'eldenring.exe'
if (-not (Test-Path -LiteralPath $exePath -PathType Leaf)) { throw '找不到游戏，请传入 -GameDir' }
if ((Get-Item -LiteralPath $exePath).VersionInfo.FileVersion -ne '2.7.2.0') { throw '验证包仅支持游戏文件版本 2.7.2.0' }
$sha256 = [System.Security.Cryptography.SHA256]::Create()
$exeStream = [System.IO.File]::OpenRead($exePath)
try { $exeHash = [System.BitConverter]::ToString($sha256.ComputeHash($exeStream)).Replace('-', '').ToLowerInvariant() }
finally { $exeStream.Dispose(); $sha256.Dispose() }
if ($exeHash -ne 'f9118fea2cfd8fa3901e8060bfadb1552cb2471771ded8a5b662c4e298ffb328') {
    throw '游戏文件与已核实的版本不匹配'
}
$packageRoot = $null
$candidates = @($PSScriptRoot, (Join-Path $PSScriptRoot 'release-preview'))
foreach ($candidate in $candidates) {
    if (-not (Test-Path -LiteralPath (Join-Path $candidate 'scene-assets\reward-art\rare-blue.rgba'))) { continue }
    if (-not (Test-Path -LiteralPath (Join-Path $candidate 'scene-assets\reward-art\rarity-cards.rgba'))) { continue }
    $requiredFiles = @('loader\bin\me3.exe', 'erdtree_tower_preview_v95.dll', 'preview.me3', 'scene-assets\regulation.bin', 'scene-assets\msg\zhocn\menu_dlc02.msgbnd.dcx', 'scene-assets\msg\zhocn\item_dlc02.msgbnd.dcx', 'scene-assets\script\talk\m00_00_00_00.talkesdbnd.dcx', 'scene-assets\event\m11_10_00_00.emevd.dcx', 'scene-assets\enemy-pools.txt', 'scene-assets\oath-art\cards.rgba', 'scene-assets\oath-art\background.rgba', 'scene-assets\oath-art\button.rgba')
    foreach ($map in @('m45_00_00_00', 'm45_01_00_00', 'm45_02_00_00')) {
        $requiredFiles += "scene-assets\map\mapstudio\$map.msb.dcx"
        $requiredFiles += "scene-assets\event\$map.emevd.dcx"
        $requiredFiles += "scene-assets\script\talk\$map.talkesdbnd.dcx"
    }
    foreach ($map in @('m10_00_00_00', 'm12_01_00_00', 'm11_00_00_00')) {
        $requiredFiles += "scene-assets\map\mapstudio\$map.msb.dcx"
        $requiredFiles += "scene-assets\event\$map.emevd.dcx"
    }
    $requiredFiles += 'scene-assets\battlefields.txt'
    $requiredFiles += 'scene-assets\reward-details.tsv'
    $requiredFiles += 'scene-assets\reward-art\card.rgba'
    $requiredFiles += @('scene-assets\map-art\paper.rgba', 'scene-assets\map-art\icons.rgba')
    $detailsFile = Join-Path $candidate 'scene-assets\reward-details.tsv'
    if (Test-Path -LiteralPath $detailsFile -PathType Leaf) {
        foreach ($line in Get-Content -LiteralPath $detailsFile -Encoding UTF8) {
            $fields = $line -split "`t"
            if ($fields.Count -eq 7) { $requiredFiles += "scene-assets\reward-icons\$($fields[1]).rgba" }
        }
    }
    $missingFiles = @($requiredFiles | Where-Object { -not (Test-Path -LiteralPath (Join-Path $candidate $_) -PathType Leaf) })
    if ($missingFiles.Count -eq 0) { $packageRoot = $candidate; break }
}
if (-not $packageRoot) { throw "找不到完整验证包，已检查目录：$($candidates -join '；')。请完整解压验证包。" }
$loader = Join-Path $packageRoot 'loader\bin\me3.exe'
$dll = Join-Path $packageRoot 'erdtree_tower_preview_v95.dll'
$profile = Join-Path $packageRoot 'preview.me3'
if ($CheckOnly) {
    [pscustomobject]@{ PackageDirectory = $packageRoot; LoaderPath = $loader; DllPath = $dll; ProfilePath = $profile }
    return
}
if (Get-Process -Name eldenring -ErrorAction SilentlyContinue) { throw '请先退出正在运行的艾尔登法环' }
$previousMarker = $env:ERDTREE_TOWER_PREVIEW
$previousLogDir = $env:ERDTREE_TOWER_LOG_DIR
try {
    $env:ERDTREE_TOWER_PREVIEW = '1'
    $env:ERDTREE_TOWER_LOG_DIR = $packageRoot
    $logPath = Join-Path $packageRoot 'tower-preview.log'
    [System.IO.File]::WriteAllText($logPath, "Tower preview launch: $(Get-Date -Format o)`r`n")
    Write-Host "预览初始化日志：$logPath"
    & $loader launch --exe $exePath --profile $profile --auto-detect --online false
    if ($LASTEXITCODE -ne 0) { throw "加载器退出码：$LASTEXITCODE" }
} finally {
    $env:ERDTREE_TOWER_PREVIEW = $previousMarker
    $env:ERDTREE_TOWER_LOG_DIR = $previousLogDir
}
