param([string]$GameDir = 'F:\SteamLibrary\steamapps\common\ELDEN RING\Game', [switch]$CheckOnly, [string]$PartyConfig, [ValidateSet('ME3','YAFSML')][string]$Loader='ME3')
$ErrorActionPreference = 'Stop'
$loaderSupport=Join-Path $PSScriptRoot 'Tower-Loader.ps1'
if(!(Test-Path -LiteralPath $loaderSupport)){$loaderSupport=Join-Path $PSScriptRoot 'distribution/Tower-Loader.ps1'}
. $loaderSupport
function Assert-TowerLobbyPassword([string]$RoomText, [string]$Settings) {
    $room = $RoomText | ConvertFrom-Json
    $entries = [regex]::Matches($Settings, '(?mi)^[ \t]*cooppassword[ \t]*=[ \t]*([^\r\n;#]*)')
    if ($entries.Count -ne 1) { throw 'Lobby password setting is missing or duplicated. Recreate the room in the launcher.' }
    $password = $entries[0].Groups[1].Value.Trim()
    $hash = [Security.Cryptography.SHA256]::Create()
    try { $actual = [BitConverter]::ToString($hash.ComputeHash([Text.Encoding]::UTF8.GetBytes($password))).Replace('-', '').ToLowerInvariant() }
    finally { $hash.Dispose() }
    if (!$room.password_sha256 -or $actual -cne $room.password_sha256 -or (!$room.locked -and $password -cne $room.password)) {
        throw 'Lobby password does not match this launch configuration. Close the game and recreate the room in the launcher.'
    }
}
# 1.6.2 起只有统一入口：原单人入口的角色存档（%APPDATA%\EldenRing\<SteamID>\ER_TOWER_PREVIEW.sl2）与包根目录里的爬塔进度（tower-*）
# 都不在统一入口读取的位置，玩家看到的是“存档不见了”。第一次启动时各复制一份过来：只在目标不存在时复制，不覆盖、不删除旧文件。
function Copy-TowerSoloData([string]$PackageRoot, [string]$Runtime, [string]$Extension) {
    $saves = if ($env:APPDATA) {Join-Path $env:APPDATA 'EldenRing'} else {$null}
    if ($saves -and (Test-Path -LiteralPath $saves -PathType Container)) {
        foreach ($profile in Get-ChildItem -LiteralPath $saves -Directory) {
            $solo = Join-Path $profile.FullName 'ER_TOWER_PREVIEW.sl2'
            if (!(Test-Path -LiteralPath $solo -PathType Leaf)) {continue}
            $target = Join-Path $profile.FullName ('ER0000.'+$Extension)
            if (Test-Path -LiteralPath $target) {Write-Host "Solo save kept as is: $solo ($target already exists and was not overwritten)."; continue}
            Copy-Item -LiteralPath $solo -Destination $target
            Write-Host "Solo save copied to $target; the original is kept."
        }
    }
    $skip = @('tower-session.json','tower-debug.enable','tower-lobby.json')
    foreach ($file in Get-ChildItem -LiteralPath $PackageRoot -File -Filter 'tower-*') {
        if ($file.Extension -in @('.log','.party-award') -or $skip -contains $file.Name) {continue}
        $target = Join-Path $Runtime $file.Name
        if (Test-Path -LiteralPath $target) {continue}
        Copy-Item -LiteralPath $file.FullName -Destination $target
        Write-Host "Solo tower progress copied: $($file.Name)."
    }
}
function Set-TowerSpectatorMode([string]$Settings) {
    $section = [regex]'(?m)^\[GAMEPLAY\][ \t]*(\r?\n|$)'
    $newline = if ($Settings.Contains("`r`n")) {"`r`n"} else {"`n"}
    foreach ($setting in @(@('always_spectate_on_death','1'),@('death_debuffs','0'))) {
        $pattern = '(?m)^[ \t]*'+$setting[0]+'[ \t]*=[^\r\n]*'
        $value = $setting[0]+' = '+$setting[1]
        if ([regex]::IsMatch($Settings,$pattern)) {
            $Settings = [regex]::Replace($Settings,$pattern,$value)
        } else {
            if (!$section.IsMatch($Settings)) {throw 'Missing GAMEPLAY section for tower death settings.'}
            $Settings = $section.Replace($Settings,('[GAMEPLAY]'+$newline+$value+$newline),1)
        }
    }
    return $Settings
}
function Set-TowerMultiplayerPreset([string]$Settings,[string]$Preset) {
    $allowed = @{
        GAMEPLAY = @('allow_invaders','allow_summons','always_spectate_on_death','death_debuffs')
        SCALING = @('enemy_health_scaling','enemy_damage_scaling','enemy_posture_scaling','boss_health_scaling','boss_damage_scaling','boss_posture_scaling')
    }
    $section = ''; $seen = @{}
    $newline = if ($Settings.Contains("`r`n")) {"`r`n"} else {"`n"}
    foreach ($line in ($Preset -split '\r?\n')) {
        if ($line -match '^\s*(;|#|$)') {continue}
        if ($line -match '^\s*\[([A-Z]+)\]\s*$') { $section=$Matches[1]; continue }
        if ($line -notmatch '^\s*([a-z_]+)\s*=\s*(\d+)\s*$') {throw 'Invalid seamless-preset.ini entry'}
        $key=$Matches[1]; $value=[int]$Matches[2]
        if (!$allowed.ContainsKey($section) -or $key -notin $allowed[$section] -or $seen.ContainsKey($key)) {throw "Unsupported or duplicate preset key: $key"}
        if ($value -gt $(if ($section -eq 'GAMEPLAY') {1} else {1000})) {throw "Preset value out of range: $key"}
        $seen[$key]=$true
        $sectionPattern = '(?ms)^\['+$section+'\][ \t]*\r?\n.*?(?=^\[|\z)'
        $block = [regex]::Match($Settings,$sectionPattern)
        if (!$block.Success) {
            $Settings += $newline+'['+$section+']'+$newline+$key+' = '+$value+$newline
            continue
        }
        $pattern = '(?m)^[ \t]*'+$key+'[ \t]*=[^\r\n]*'
        $entry = $key+' = '+$value
        $updated = if ([regex]::IsMatch($block.Value,$pattern)) { [regex]::Replace($block.Value,$pattern,$entry) } else { $block.Value.TrimEnd("`r","`n")+$newline+$entry+$newline }
        $Settings = $Settings.Substring(0,$block.Index)+$updated+$Settings.Substring($block.Index+$block.Length)
    }
    if ($seen.Count -ne 10) {throw 'Incomplete seamless-preset.ini: expected 10 settings'}
    return $Settings
}
# Extra plugins (plugins/natives.toml, managed by the launcher)
# Each enabled [[natives]] section becomes an absolute path for ME3; disabled or missing plugins are skipped with a warning.
function Get-TowerPluginSections([string]$PackageRoot) {
    $file = Join-Path $PackageRoot 'plugins'
    $file = Join-Path $file 'natives.toml'
    if (-not (Test-Path -LiteralPath $file -PathType Leaf)) { return @() }
    $sections = [regex]::Matches([IO.File]::ReadAllText($file), '(?ms)^\s*\[\[natives\]\][^\r\n]*\r?\n.*?(?=^\s*\[\[|\z)')
    foreach ($section in $sections) {
        $text = $section.Value
        if ($text -match '(?m)^\s*enabled\s*=\s*false\b') { continue }
        $match = [regex]::Match($text, '(?m)^(\s*path\s*=\s*)[''"]([^''"\r\n]+)[''"]')
        if (-not $match.Success) { Write-Warning 'plugins/natives.toml: a [[natives]] entry has no single-line quoted path, skipped.'; continue }
        $path = $match.Groups[2].Value.Replace('\', '/').Replace('/', [IO.Path]::DirectorySeparatorChar)
        if (-not [IO.Path]::IsPathRooted($path)) { $path = Join-Path $PackageRoot $path }
        $path = [IO.Path]::GetFullPath($path)
        if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { Write-Warning "Plugin not found, skipped: $path"; continue }
        if ($path.Contains("'")) { Write-Warning "Plugin path contains a quote, skipped: $path"; continue }
        $text = $text.Substring(0, $match.Index) + $match.Groups[1].Value + "'" + $path.Replace('\', '/') + "'" + $text.Substring($match.Index + $match.Length)
        # enabled is the launcher's switch, not an ME3 option
        $text = [regex]::Replace($text, '(?m)^\s*enabled\s*=.*(\r?\n|\z)', '')
        $text.TrimEnd()
    }
}
# End extra plugins
$base = & (Join-Path $PSScriptRoot 'Launch-Preview.ps1') -GameDir $GameDir -Loader $Loader -CheckOnly
$root = $base.PackageDirectory
if ($PartyConfig) {
    $PartyConfig = (Resolve-Path -LiteralPath $PartyConfig -ErrorAction Stop).Path
    if ((Get-Item -LiteralPath $PartyConfig).Length -gt 8192) {throw 'Party configuration is too large.'}
    $party = [IO.File]::ReadAllText($PartyConfig) | ConvertFrom-Json
    if (!$party.session -or !$party.player -or !$party.host -or !$party.address) {throw 'Invalid party configuration.'}
}
$entry = Join-Path $root 'seamless-entry'
$launcher = $base.LoaderPath
$settingsPath = Join-Path $root 'config\SeamlessCoop\ersc_settings.ini'
$ersc = Join-Path $root 'SeamlessCoop\ersc.dll'
if (!(Test-Path -LiteralPath $settingsPath)) {
    $previousSettings = Join-Path $entry 'runtime\SeamlessCoop\ersc_settings.ini'
    $template = Join-Path $root 'SeamlessCoop\default.ini'
    $source = if (Test-Path -LiteralPath $previousSettings) {$previousSettings} else {$template}
    $initial = [IO.File]::ReadAllText($source)
    if ($source -eq $template) {
        $password = [guid]::NewGuid().ToString('N')
        $initial = [regex]::Replace($initial,'(?m)^cooppassword\s*=[^\r\n]*',('cooppassword = '+$password))
    }
    if (!$CheckOnly) {
        New-Item -ItemType Directory -Path (Split-Path -Parent $settingsPath) -Force | Out-Null
        $stream = [IO.File]::Open($settingsPath,[IO.FileMode]::CreateNew,[IO.FileAccess]::Write)
        try {
            $bytes = [Text.UTF8Encoding]::new($false).GetBytes($initial)
            $stream.Write($bytes,0,$bytes.Length)
        } finally { $stream.Dispose() }
    }
}
foreach ($file in @($launcher,$ersc)) {
    if (!(Test-Path -LiteralPath $file -PathType Leaf)) {throw "Missing dependency: $file"}
}
# Seamless Co-op builds verified with this package. Other builds still launch: the tower DLL's revival fix checks the
# Seamless build itself and switches off on a mismatch; everything else was only tested on the builds listed here.
$verifiedSeamless = @(
    'FCD11A1849FD37851BB449BE30F1525CF498F63FD1FA568C4E15030EBE7CB251' # 2.0.1 (revival fix supported)
    '0A093D8490967BF19C62F895FDF1FDA21840282D2B2B371B9B4FC24A0CFE146E' # 2.0.2 (version-specific revival interface)
    '2EBDE62BE864BA3D759728AAC44C459F0F813E8B6D7A342304CE68F6513F280C' # 2.0.3 (pinned component; offline interface checks passed)
)
$sha256 = [System.Security.Cryptography.SHA256]::Create()
try {
    $erscStream = [System.IO.File]::OpenRead($ersc)
    try { $erscHash = [System.BitConverter]::ToString($sha256.ComputeHash($erscStream)).Replace('-', '') }
    finally { $erscStream.Dispose() }
} finally { $sha256.Dispose() }
if ($verifiedSeamless -notcontains $erscHash) {
    Write-Warning "This Seamless Co-op build is not verified with the tower (SHA256 $erscHash). Launching anyway: the revival fix turns itself off; all players must use the same Seamless build."
}
$proxy = Join-Path $GameDir 'dinput8.dll'
if (Test-Path -LiteralPath $proxy) {throw 'Existing dinput8.dll detected. It will not be overwritten.'}
$settings = if (Test-Path -LiteralPath $settingsPath) {[IO.File]::ReadAllText($settingsPath)} else {$initial}
if ([regex]::Matches($settings,'(?m)^save_file_extension\s*=.*$').Count -ne 1) {throw 'Expected one save extension setting.'}
$saveExtension = if ($PartyConfig) {'ertowerparty'} else {'ertowercoop'}
$modified = [regex]::Replace($settings,'(?m)^save_file_extension\s*=.*$',('save_file_extension = '+$saveExtension))
$modified = [regex]::Replace($modified,'(?m)^allow_invaders\s*=.*$','allow_invaders = 0')
$preset = Join-Path $root 'seamless-preset.ini'
if (!(Test-Path -LiteralPath $preset -PathType Leaf)) {throw 'Missing seamless-preset.ini; update the complete tower package.'}
$modified = Set-TowerMultiplayerPreset $modified ([IO.File]::ReadAllText($preset))
$modified = Set-TowerSpectatorMode $modified
$modified = [regex]::Replace($modified,'(?m)^skip_splash_screens\s*=[^\r\n]*','skip_splash_screens = 1')
$modified = [regex]::Replace($modified,'(?m)^allow_summons\s*=[^\r\n]*','allow_summons = 1')
if ($CheckOnly) {[pscustomobject]@{Entry=$launcher;SaveExtension=$saveExtension;Assets= (Join-Path $root 'scene-assets');MultiplayerVerified=$false};return}
if (Get-Process eldenring,ersc_launcher -ErrorAction SilentlyContinue) {throw 'Exit the game and Seamless launcher first.'}
$assetMode = Join-Path $root 'asset-mode.txt'
if ((Test-Path -LiteralPath $assetMode) -and [IO.File]::ReadAllText($assetMode).Trim() -notin @('full', 'local-v1')) {throw 'Invalid asset mode; verify and repair the package.'}
if (-not (Test-Path -LiteralPath $assetMode) -and (Test-Path -LiteralPath (Join-Path $root 'asset-patches\manifest.json'))) {throw 'Missing asset mode; verify and repair the package.'}
if ((Test-Path -LiteralPath $assetMode) -and [IO.File]::ReadAllText($assetMode).Trim() -eq 'local-v1') {
    if (-not (Test-Path -LiteralPath (Join-Path $root 'asset-patches\manifest.json'))) {throw 'Missing asset patch manifest; verify and repair the package.'}
    $restore = Join-Path $root 'Tower.AssetRestore.exe'
    if (-not (Test-Path -LiteralPath $restore)) {throw 'Missing asset generator; verify and repair the package.'}
    & $restore $root $GameDir
    if ($LASTEXITCODE -ne 0) {throw 'Local asset generation failed; see logs or use the full package.'}
}
$native = $base.DllPath
$assets = Join-Path $root 'scene-assets'
$runtime = Join-Path $entry $(if ($PartyConfig) {'party-runtime'} else {'runtime'})
New-Item -ItemType Directory -Path $runtime -Force | Out-Null
if (!$PartyConfig) {Copy-TowerSoloData $root $runtime $saveExtension}
$assetLink = Join-Path $runtime 'scene-assets'
# The tower DLL checks arena files through this link (its log directory is the runtime folder). A link left behind by
# a moved or copied installation, or a plain copied folder, keeps serving old files while the game loads the new ones,
# so every launch makes sure it is a junction to this installation's scene-assets.
if (Test-Path -LiteralPath $assetLink) {
    $linkItem = Get-Item -LiteralPath $assetLink -Force
    $linkTarget = @($linkItem.Target | Where-Object { $_ }) | Select-Object -First 1
    $current = $linkItem.LinkType -eq 'Junction' -and $linkTarget -and ([IO.Path]::GetFullPath($linkTarget).TrimEnd('\') -ieq [IO.Path]::GetFullPath($assets).TrimEnd('\'))
    if (-not $current) {
        if ($linkItem.LinkType) { [IO.Directory]::Delete($assetLink) }
        else { Move-Item -LiteralPath $assetLink -Destination ($assetLink + '.stale-' + (Get-Date -Format 'yyyyMMddHHmmss')) }
        Write-Host "Replaced stale runtime scene-assets link (was: $(if ($linkItem.LinkType) {"$($linkItem.LinkType) -> $linkTarget"} else {'a copied folder'}))."
    }
}
if (!(Test-Path -LiteralPath $assetLink)) {New-Item -ItemType Junction -Path $assetLink -Target $assets | Out-Null}
foreach ($path in @($native,$assets)) {if ($path.Contains("'")) {throw 'Unsupported quote in installation path.'}}
$config = @"
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
$configPath = Join-Path $runtime 'seamless.me3'
# Custom DLL configuration
$editableProfile = $configPath
if (!(Test-Path -LiteralPath $editableProfile)) {
    [IO.File]::WriteAllText($editableProfile,$config,[Text.UTF8Encoding]::new($false))
}
$config = [IO.File]::ReadAllText($editableProfile)
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
foreach ($section in [regex]::Matches($config,$nativeSections)) {
    $knownNatives[(Get-NativeProfileKey $section.Value)] = $true
}
foreach ($section in [regex]::Matches([IO.File]::ReadAllText($customPath),$nativeSections)) {
    $key = Get-NativeProfileKey $section.Value
    if (!$knownNatives.ContainsKey($key)) {
        $config += "`r`n" + $section.Value
        $knownNatives[$key] = $true
    }
}
$configPath = Join-Path $runtime ([IO.Path]::GetFileNameWithoutExtension($editableProfile) + '.launch.me3')
# End custom DLL configuration
foreach ($section in @(Get-TowerPluginSections $root)) {
    $key = Get-NativeProfileKey $section
    if (!$knownNatives.ContainsKey($key)) {
        $config += "`r`n" + $section
        $knownNatives[$key] = $true
    }
}
# The editable profile keeps user entries; tower-owned paths always follow this installation.
$config = [regex]::Replace($config,"(?m)^(\s*path\s*=\s*)['""][^'""\r\n]*[\\/]erdtree_tower_\w+\.dll['""]",('${1}'''+$native.Replace('\','/').Replace('$','$$')+''''))
$config = [regex]::Replace($config,"(?m)^(\s*path\s*=\s*)['""][^'""\r\n]*[\\/]scene-assets['""]",('${1}'''+$assets.Replace('\','/').Replace('$','$$')+''''))
$config=Add-TowerLanguagePackage $config $root $GameDir
[IO.File]::WriteAllText($configPath,$config,(New-Object Text.UTF8Encoding($false)))
$private = Join-Path $runtime 'SeamlessCoop'
New-Item -ItemType Directory -Path $private -Force | Out-Null
Copy-Item -LiteralPath $ersc -Destination (Join-Path $private 'ersc.dll') -Force
foreach ($directory in @('crashpad','locale')) {
    $source = Join-Path (Split-Path $ersc) $directory
    if (Test-Path -LiteralPath $source -PathType Container) {Copy-Item -LiteralPath $source -Destination $private -Recurse -Force}
}
[IO.File]::WriteAllText((Join-Path $private 'ersc_settings.ini'),$modified,[Text.UTF8Encoding]::new($false))
$environment = @{
    ERDTREE_TOWER_PARTY=$PartyConfig
    ERDTREE_TOWER_SEAMLESS_ENTRY='1'; ERDTREE_TOWER_PREVIEW='1'; ERDTREE_TOWER_SEAMLESS_PROBE='0'
    ERDTREE_TOWER_LOG_DIR=$runtime
}
$previous = @{}
$roomGuard = $null
try {
    $roomPath = Join-Path $root 'tower-lobby.json'
    if (Test-Path -LiteralPath $roomPath) {
        $roomGuard = [IO.File]::Open($roomPath, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::Read)
        Assert-TowerLobbyPassword ([IO.File]::ReadAllText($roomPath)) ([IO.File]::ReadAllText((Join-Path $private 'ersc_settings.ini')))
    }
    foreach ($key in $environment.Keys) {
        $previous[$key]=[Environment]::GetEnvironmentVariable($key)
        if ([string]::IsNullOrEmpty($environment[$key])) { Remove-Item -LiteralPath "Env:$key" -ErrorAction SilentlyContinue }
        else { [Environment]::SetEnvironmentVariable($key,$environment[$key]) }
    }
    Write-Host "${Loader}: Seamless + tower. Separate save: $saveExtension."
    Invoke-TowerLoader $root $Loader (Join-Path $GameDir 'eldenring.exe') $configPath
} finally {
    if ($roomGuard) { $roomGuard.Dispose() }
    foreach ($key in $previous.Keys) {
        if ($null -eq $previous[$key]) { Remove-Item -LiteralPath "Env:$key" -ErrorAction SilentlyContinue }
        else { [Environment]::SetEnvironmentVariable($key,$previous[$key]) }
    }
}
