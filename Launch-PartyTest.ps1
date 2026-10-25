param(
    [Parameter(Mandatory=$true)][string]$StageDirectory,
    [string]$GameDirectory = 'F:\SteamLibrary\steamapps\common\ELDEN RING\Game',
    [switch]$CheckOnly
)
$ErrorActionPreference = 'Stop'
$stage = (Resolve-Path -LiteralPath $StageDirectory).Path
$workRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../../work')) + [IO.Path]::DirectorySeparatorChar
if (-not $stage.StartsWith($workRoot,[StringComparison]::OrdinalIgnoreCase)) { throw 'Test stage must be inside workspace/work.' }
$loader = Join-Path $PSScriptRoot 'release-preview/loader/bin/me3.exe'
$native = Join-Path $stage 'erdtree_tower_party_test.dll'
$assets = Join-Path $stage 'scene-assets'
$seamless = Join-Path $GameDirectory 'SeamlessCoop'
foreach ($file in @($loader,$native,(Join-Path $stage 'native.sha256'),(Join-Path $stage 'assets.sha256.json'),(Join-Path $seamless 'ersc.dll'),(Join-Path $seamless 'ersc_settings.ini'),(Join-Path $GameDirectory 'eldenring.exe'))) {
    if (-not (Test-Path -LiteralPath $file -PathType Leaf)) { throw "Missing test dependency: $file" }
}
if ((Get-FileHash -LiteralPath (Join-Path $seamless 'ersc.dll')).Hash -ne 'FCD11A1849FD37851BB449BE30F1525CF498F63FD1FA568C4E15030EBE7CB251') { throw 'Expected inspected Seamless Co-op 2.0.1.' }
if ((Get-FileHash -LiteralPath $native).Hash -ne (Get-Content -LiteralPath (Join-Path $stage 'native.sha256') -Raw).Trim()) { throw 'Test DLL checksum mismatch.' }
$manifest = Get-Content -LiteralPath (Join-Path $stage 'assets.sha256.json') -Raw | ConvertFrom-Json
if ($manifest.Count -eq 0) { throw 'Empty asset manifest.' }
foreach ($entry in $manifest) {
    $path = [IO.Path]::GetFullPath((Join-Path $assets $entry.Path))
    if (-not $path.StartsWith($assets + [IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)) { throw 'Asset manifest path escapes stage.' }
    if ((Get-FileHash -LiteralPath $path).Hash -ne $entry.SHA256) { throw "Test asset checksum mismatch: $($entry.Path)" }
}
if (Test-Path -LiteralPath (Join-Path $GameDirectory 'dinput8.dll')) { throw 'Existing proxy loader detected.' }
if ($CheckOnly) { [pscustomobject]@{Stage=$stage;AssetsChecked=$manifest.Count;SaveExtension='ertowertest';Deployed=$false};return }
if (Get-Process eldenring,ersc_launcher -ErrorAction SilentlyContinue) { throw 'Exit the running game before starting the isolated test.' }
foreach ($path in @($stage,$GameDirectory)) { if ($path.Contains("'")) { throw 'Unsupported quote in test path.' } }
$settings = [IO.File]::ReadAllText((Join-Path $seamless 'ersc_settings.ini'))
if ([regex]::Matches($settings,'(?m)^save_file_extension\s*=.*$').Count -ne 1) { throw 'Expected one save extension setting.' }
$settings = [regex]::Replace($settings,'(?m)^save_file_extension\s*=.*$','save_file_extension = ertowertest')
$settings = [regex]::Replace($settings,'(?m)^allow_invaders\s*=.*$','allow_invaders = 0')
$runtime = Join-Path $stage 'runtime'
$private = Join-Path $runtime 'SeamlessCoop'
New-Item -ItemType Directory -Path $private -Force | Out-Null
Copy-Item -LiteralPath (Join-Path $seamless 'ersc.dll') -Destination $private -Force
foreach ($folder in @('crashpad','locale')) {
    $source = Join-Path $seamless $folder
    if (Test-Path -LiteralPath $source -PathType Container) { Copy-Item -LiteralPath $source -Destination $private -Recurse -Force }
}
[IO.File]::WriteAllText((Join-Path $private 'ersc_settings.ini'),$settings,[Text.UTF8Encoding]::new($false))
$assetLink = Join-Path $runtime 'scene-assets'
if (-not (Test-Path -LiteralPath $assetLink)) { New-Item -ItemType Junction -Path $assetLink -Target $assets | Out-Null }
$profile = @"
profileVersion = "v1"
start_online = false
[[supports]]
game = "eldenring"
[[natives]]
path = 'SeamlessCoop/ersc.dll'
[[natives]]
path = '$($native.Replace('\','/'))'
load_after = [{ id = "ersc.dll", optional = false }]
[[packages]]
path = '$($assets.Replace('\','/'))'
"@
$profilePath = Join-Path $runtime 'test.me3'
# Custom DLL configuration
$editableProfile = $profilePath
if (!(Test-Path -LiteralPath $editableProfile)) {
    [IO.File]::WriteAllText($editableProfile,$profile,[Text.UTF8Encoding]::new($false))
}
$profile = [IO.File]::ReadAllText($editableProfile)
$customPath = Join-Path $runtime 'custom-natives.toml'
if (!(Test-Path -LiteralPath $customPath)) {
    [IO.File]::WriteAllText($customPath, "# Extra [[natives]] entries; you can also edit the main .me3 profile directly.`r`n",[Text.UTF8Encoding]::new($false))
}
$nativeSections = '(?ms)^\s*\[\[natives\]\][^\r\n]*\r?\n.*?(?=^\s*\[\[|\z)'
function Get-NativeProfileKey([string]$section) {
    $match = [regex]::Match($section, '(?m)^\s*path\s*=\s*[''"]([^''"\r\n]+)[''"]\s*(?:#.*)?$')
    if (!$match.Success) { throw 'Extra native entry requires a single-line quoted path.' }
    $value = $match.Groups[1].Value.Replace('/', '\')
    if (![IO.Path]::IsPathRooted($value)) { $value = Join-Path $runtime $value }
    [IO.Path]::GetFullPath($value).ToUpperInvariant()
}
$knownNatives = @{}
foreach ($section in [regex]::Matches($profile,$nativeSections)) {
    $knownNatives[(Get-NativeProfileKey $section.Value)] = $true
}
foreach ($section in [regex]::Matches([IO.File]::ReadAllText($customPath),$nativeSections)) {
    $key = Get-NativeProfileKey $section.Value
    if (!$knownNatives.ContainsKey($key)) {
        $profile += "`r`n" + $section.Value
        $knownNatives[$key] = $true
    }
}
$profilePath = Join-Path $runtime ([IO.Path]::GetFileNameWithoutExtension($editableProfile) + '.launch.me3')
# End custom DLL configuration
[IO.File]::WriteAllText($profilePath,$profile,[Text.UTF8Encoding]::new($false))
$environment = @{ERDTREE_TOWER_PARTY=$null;ERDTREE_TOWER_SEAMLESS_ENTRY='1';ERDTREE_TOWER_PREVIEW='1';ERDTREE_TOWER_SEAMLESS_PROBE='0';ERDTREE_TOWER_LOG_DIR=$runtime}
$previous = @{}
try {
    foreach ($key in $environment.Keys) {
        $previous[$key]=[Environment]::GetEnvironmentVariable($key)
        if ([string]::IsNullOrEmpty($environment[$key])) { Remove-Item -LiteralPath "Env:$key" -ErrorAction SilentlyContinue }
        else { [Environment]::SetEnvironmentVariable($key,$environment[$key]) }
    }
    & $loader launch --exe (Join-Path $GameDirectory 'eldenring.exe') --profile $profilePath --auto-detect --online false
    if ($LASTEXITCODE -ne 0) { throw "Test loader exited: $LASTEXITCODE" }
} finally {
    foreach ($key in $previous.Keys) {
        if ($null -eq $previous[$key]) { Remove-Item -LiteralPath "Env:$key" -ErrorAction SilentlyContinue }
        else { [Environment]::SetEnvironmentVariable($key,$previous[$key]) }
    }
}
