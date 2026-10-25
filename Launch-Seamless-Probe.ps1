param([string]$GameDir = 'F:\SteamLibrary\steamapps\common\ELDEN RING\Game', [switch]$CheckOnly, [switch]$PrepareOnly)
$ErrorActionPreference = 'Stop'
$base = & (Join-Path $PSScriptRoot 'Launch-Preview.ps1') -GameDir $GameDir -CheckOnly
$sourceDll = Join-Path $GameDir 'SeamlessCoop\ersc.dll'
$sourceSettings = Join-Path $GameDir 'SeamlessCoop\ersc_settings.ini'
$probeDll = Join-Path $base.PackageDirectory 'erdtree_tower_seamless_probe.dll'
foreach ($file in @($sourceDll,$sourceSettings,$probeDll)) {if (!(Test-Path -LiteralPath $file -PathType Leaf)) {throw "Missing probe dependency: $file"}}
$expectedHash = 'FCD11A1849FD37851BB449BE30F1525CF498F63FD1FA568C4E15030EBE7CB251'
if ((Get-FileHash -LiteralPath $sourceDll -Algorithm SHA256).Hash -ne $expectedHash) {throw 'Seamless DLL differs from the inspected build. Probe requires a new compatibility check.'}
$probeRoot = Join-Path $base.PackageDirectory 'seamless-probe'
if ($CheckOnly) {[pscustomobject]@{Mode='ReadOnlyProbe';ProbeDirectory=$probeRoot;SeamlessHash=$expectedHash;SaveExtension='ertowerprobe';GameplayCompatible=$false};return}
if (!$PrepareOnly -and (Get-Process -Name eldenring -ErrorAction SilentlyContinue)) {throw 'Exit Elden Ring before starting the probe.'}
$seamlessRoot = Join-Path $probeRoot 'SeamlessCoop'
New-Item -ItemType Directory -Path $seamlessRoot -Force | Out-Null
Copy-Item -LiteralPath $sourceDll -Destination (Join-Path $seamlessRoot 'ersc.dll') -Force
foreach ($directory in @('crashpad', 'locale')) {
    $sourceDirectory = Join-Path (Split-Path $sourceDll) $directory
    if (Test-Path -LiteralPath $sourceDirectory -PathType Container) {
        Copy-Item -LiteralPath $sourceDirectory -Destination $seamlessRoot -Recurse -Force
    }
}
$settings = [System.IO.File]::ReadAllText($sourceSettings)
if ([regex]::Matches($settings,'(?m)^save_file_extension\s*=.*$').Count -ne 1) {throw 'Expected one Seamless save_file_extension setting.'}
$settings = [regex]::Replace($settings,'(?m)^save_file_extension\s*=.*$','save_file_extension = ertowerprobe')
$settings = [regex]::Replace($settings,'(?m)^allow_invaders\s*=.*$','allow_invaders = 0')
[System.IO.File]::WriteAllText((Join-Path $seamlessRoot 'ersc_settings.ini'),$settings,(New-Object System.Text.UTF8Encoding($false)))
$nativePath = $probeDll.Replace('\','/')
if ($nativePath.Contains("'")) {throw 'Unsupported quote in profile path.'}
$profile = @"
profileVersion = "v1"
savefile = "ER_TOWER_SEAMLESS_PROBE.sl2"
start_online = false
[[supports]]
game = "eldenring"
[[natives]]
path = 'SeamlessCoop/ersc.dll'
[[natives]]
path = '$nativePath'
"@
$profilePath = Join-Path $probeRoot 'probe.me3'
[System.IO.File]::WriteAllText($profilePath,$profile,(New-Object System.Text.UTF8Encoding($false)))
if ($PrepareOnly) {[pscustomobject]@{ProfilePath=$profilePath;GameplayCompatible=$false};return}
$oldPreview=$env:ERDTREE_TOWER_PREVIEW
$oldProbe=$env:ERDTREE_TOWER_SEAMLESS_PROBE
$oldLog=$env:ERDTREE_TOWER_LOG_DIR
try {
    $env:ERDTREE_TOWER_PREVIEW='1'
    $env:ERDTREE_TOWER_SEAMLESS_PROBE='1'
    $env:ERDTREE_TOWER_LOG_DIR=$probeRoot
    [System.IO.File]::WriteAllText((Join-Path $probeRoot 'tower-preview.log'),"Seamless read-only probe; no tower gameplay. DLL SHA256: $expectedHash`r`n")
    Write-Host 'Read-only compatibility probe. Create a temporary character; no tower gameplay is enabled.'
    & $base.LoaderPath launch --exe (Join-Path $GameDir 'eldenring.exe') --profile $profilePath --auto-detect --online false
    if ($LASTEXITCODE -ne 0) {throw "Probe loader exit: $LASTEXITCODE"}
} finally {
    $env:ERDTREE_TOWER_PREVIEW=$oldPreview
    $env:ERDTREE_TOWER_SEAMLESS_PROBE=$oldProbe
    $env:ERDTREE_TOWER_LOG_DIR=$oldLog
}
