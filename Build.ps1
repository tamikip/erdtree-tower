param([switch]$Offline)
$ErrorActionPreference = 'Stop'
$workspace = Split-Path (Split-Path $PSScriptRoot -Parent) -Parent
$coreTarget = Join-Path $workspace 'work\erdtree-core-build'
$nativeTarget = Join-Path $workspace 'work\erdtree-native-build'
$release = Join-Path $PSScriptRoot 'release-preview'
$rewardCard=Join-Path $PSScriptRoot 'scene-assets\reward-art\card.rgba'
if ((Get-Item -LiteralPath (Join-Path $PSScriptRoot 'scene-assets\reward-art\rare-blue.rgba')).Length -ne 6294152) {throw 'Invalid blue reward texture'}
if ((Get-Item -LiteralPath (Join-Path $PSScriptRoot 'scene-assets\reward-art\rarity-cards.rgba')).Length -ne 6291456) {throw 'Invalid reward rarity atlas'}
if (!(Test-Path -LiteralPath $rewardCard) -or (Get-Item -LiteralPath $rewardCard).Length -ne 6291456) {throw 'Invalid reward card texture'}
foreach ($asset in @(@('cards',1254,1254),@('background',1672,941),@('button',2172,724))) {
    $assetPath=Join-Path $PSScriptRoot "scene-assets\oath-art\$($asset[0]).rgba"
    if (!(Test-Path -LiteralPath $assetPath) -or (Get-Item -LiteralPath $assetPath).Length -ne ($asset[1]*$asset[2]*4)) {throw "Invalid oath texture: $assetPath"}
}
foreach ($asset in @(@('paper', 6291456), @('icons', 6290064))) {
    $assetPath = Join-Path $PSScriptRoot "scene-assets\map-art\$($asset[0]).rgba"
    if (-not (Test-Path -LiteralPath $assetPath) -or (Get-Item -LiteralPath $assetPath).Length -ne $asset[1]) { throw "Invalid map asset: $assetPath" }
}
foreach ($line in Get-Content -LiteralPath (Join-Path $PSScriptRoot 'scene-assets\reward-details.tsv') -Encoding UTF8) {
    $fields = $line -split "`t"
    if ($fields.Count -ne 7) { throw 'Invalid reward detail row' }
    $iconFile = Join-Path $PSScriptRoot "scene-assets\reward-icons\$($fields[1]).rgba"
    if (-not (Test-Path -LiteralPath $iconFile) -or (Get-Item -LiteralPath $iconFile).Length -ne 102400) { throw "Invalid reward icon: $iconFile" }
}
[string[]]$networkArgs = if ($Offline) { @('--offline') } else { @() }
function Invoke-Cargo([string[]]$Arguments) {
    & cargo @Arguments
    if ($LASTEXITCODE -ne 0) { throw "Cargo failed: $($Arguments -join ' ')" }
}
Invoke-Cargo (@('test', '--locked', '--manifest-path', "$PSScriptRoot\Cargo.toml", '--target-dir', $coreTarget) + $networkArgs)
Invoke-Cargo (@('test', '--locked', '--release', '--manifest-path', "$PSScriptRoot\native\Cargo.toml", '--target-dir', $nativeTarget) + $networkArgs)
Invoke-Cargo (@('test', '--locked', '--manifest-path', "$PSScriptRoot\native\vendor\hudhook\Cargo.toml", '--target-dir', $nativeTarget, '--no-default-features', '--features', 'dx12', '--lib', 'dx12_queue_tests') + $networkArgs)
& python (Join-Path $PSScriptRoot 'tests\package-native.py')
if ($LASTEXITCODE -ne 0) { throw 'Native packaging regression checks failed' }
Invoke-Cargo (@('build', '--locked', '--release', '--manifest-path', "$PSScriptRoot\Cargo.toml", '--target-dir', $coreTarget) + $networkArgs)
Invoke-Cargo (@('build', '--locked', '--release', '--manifest-path', "$PSScriptRoot\native\Cargo.toml", '--target-dir', $nativeTarget) + $networkArgs)
$allNotices = [System.Collections.Generic.List[string]]::new()
$allNotices.Add('Generated dependency license notices. See Cargo.lock for exact revisions.')
$seenPackages = [System.Collections.Generic.HashSet[string]]::new()
foreach ($manifest in @("$PSScriptRoot\Cargo.toml", "$PSScriptRoot\native\Cargo.toml")) {
    $metadataText = & cargo metadata --locked --filter-platform x86_64-pc-windows-msvc --format-version 1 --manifest-path $manifest @networkArgs
    if ($LASTEXITCODE -ne 0) { throw 'Unable to collect dependency licenses' }
    $metadata = ($metadataText -join "`n") | ConvertFrom-Json
    foreach ($package in $metadata.packages) {
        if ((-not $package.source -and $package.name -ne 'hudhook') -or -not $seenPackages.Add($package.id)) { continue }
        $allNotices.Add("`n===== $($package.name) $($package.version) | $($package.license) | $($package.source) =====")
        $packageDir = Split-Path $package.manifest_path -Parent
        $licenseFiles = @(Get-ChildItem -LiteralPath $packageDir -File | Where-Object { $_.Name -match '^(LICENSE|LICENCE|COPYING|NOTICE)' })
        $minHookNotice = Join-Path $packageDir 'vendor\minhook\LICENSE.txt'
        if (Test-Path -LiteralPath $minHookNotice -PathType Leaf) { $licenseFiles += Get-Item -LiteralPath $minHookNotice }
        foreach ($licenseFile in $licenseFiles) {
            $allNotices.Add("--- $($licenseFile.Name) ---")
            $allNotices.Add([System.IO.File]::ReadAllText($licenseFile.FullName))
        }
    }
}
[System.IO.File]::WriteAllText((Join-Path $PSScriptRoot 'licenses\rust-dependencies.txt'), ($allNotices -join "`n"))
New-Item -ItemType Directory -Path $release -Force | Out-Null
Copy-Item -LiteralPath "$coreTarget\release\erdtree-tower.exe" -Destination $release
Copy-Item -LiteralPath "$nativeTarget\release\erdtree_tower_preview.dll" -Destination (Join-Path $release 'erdtree_tower_preview_v95.dll')
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'scene-assets') -Destination $release -Recurse -Force
foreach ($name in @('preview.me3', 'Launch-Preview.ps1', 'Launch-Preview.cmd', 'README.md', 'THIRD_PARTY_NOTICES.md')) {
    Copy-Item -LiteralPath (Join-Path $PSScriptRoot $name) -Destination $release
}
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'licenses') -Destination $release -Recurse -Force
& powershell.exe -NoProfile -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot 'tests\launcher-compatibility.ps1')
if ($LASTEXITCODE -ne 0) { throw 'Windows PowerShell 5.1 launcher compatibility check failed' }
