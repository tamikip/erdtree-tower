param([string]$GameDirectory, [switch]$CheckOnly, [switch]$ResetGamePath)
$ErrorActionPreference = 'Stop'
function Get-TowerStartupFailure([string]$Message, [string]$ErrorId = '') {
    $code='T099';$cause='启动过程遇到未识别的错误。';$fix='请截图整个启动窗口，并将错误编号和最后一步操作反馈给作者。';$detail=$Message;$retry=$false
    switch -Regex ($Message) {
        '^(Game folder cannot be empty\.|Specify -GameDirectory for validation\.)' {
            $code='T001';$cause='没有填写游戏目录。';$fix='请输入包含 eldenring.exe 的 Game 文件夹路径；也可以直接粘贴 eldenring.exe 的完整路径。';$detail='';$retry=$true;break
        }
        '^(Select the Game folder containing eldenring\.exe\.|Select eldenring\.exe or its Game folder\.|Missing test dependency: .*eldenring\.exe$)' {
            $code='T002';$cause='选择的位置不是法环的 Game 目录，或保存的游戏路径已失效。';$fix='在 Steam 中浏览艾尔登法环的本地文件，再进入 Game 文件夹并复制路径。以后更换位置时，双击 Reset-GamePath.cmd 重新选择。';$detail='示例：D:\SteamLibrary\steamapps\common\ELDEN RING\Game';$retry=$true;break
        }
        '^Game version mismatch\.' {
            $code='T003';$cause='游戏程序与当前爬塔包支持的版本不一致。';$fix='本包支持 eldenring.exe 文件版本 2.7.2.0，并校验文件内容。请使用与游戏版本对应的爬塔包；版本相同仍报错时，可通过 Steam 验证游戏文件后重试。';$detail=''
            if($Message -match 'Current file version: ([0-9.]+)') {$detail='检测到的游戏文件版本：'+$Matches[1]};break
        }
        '^Unsupported quote in (game|test) path\.' {
            $code='T004';$cause='游戏或爬塔文件夹路径中含有单引号。';$fix='请更换文件夹位置或重命名，去掉路径中的单引号（''）。路径两端的双引号可以保留。';$detail='';$retry=$Message.Contains('game path');break
        }
        '^Missing test dependency: .*SeamlessCoop[\\/]ersc\.dll$' {
            $code='T010';$cause='没有在所选游戏目录中找到无缝联机组件。';$fix='请自行安装 Seamless Co-op 2.0.1，把完整 SeamlessCoop 文件夹放在 eldenring.exe 同级。不要多套一层文件夹。';$detail='应存在：Game\SeamlessCoop\ersc.dll';break
        }
        '^Missing test dependency: .*SeamlessCoop[\\/]ersc_settings\.ini$' {
            $code='T011';$cause='缺少无缝联机配置文件 ersc_settings.ini。';$fix='请从 Seamless Co-op 2.0.1 原包恢复配置文件，放入 Game\SeamlessCoop；再自行填写联机密码。不要使用旧版配置。';$detail='';break
        }
        '^Expected inspected Seamless Co-op 2\.0\.1\.' {
            $code='T012';$cause='无缝联机组件的版本或文件内容与本包要求不一致。';$fix='本包要求已适配的 Seamless Co-op 2.0.1。请使用该版本的完整 SeamlessCoop 文件夹，确认没有混用新旧 DLL。';$detail='';break
        }
        '^Expected one save extension setting\.' {
            $code='T013';$cause='无缝配置中的 save_file_extension 缺失或重复。';$fix='请用无缝 2.0.1 原包中的 ersc_settings.ini 替换游戏目录下的配置，再填写自己的 cooppassword；不要合并多份旧配置。';$detail='配置位置：Game\SeamlessCoop\ersc_settings.ini';break
        }
        '^Expected one always_spectate_on_death setting' {
            $code='T014';$cause='无缝配置中的 always_spectate_on_death 缺失、重复或格式不正确。';$fix='请恢复无缝 2.0.1 原包的 ersc_settings.ini，再填写联机密码。不要从旧版复制配置，也不要手动重复添加这一项。';$detail='配置位置：Game\SeamlessCoop\ersc_settings.ini';break
        }
        '^Missing test dependency: ' {
            $code='T020';$cause='爬塔整合包缺少必要文件，可能未完整解压或文件被安全软件隔离。';$fix='请将完整整合包重新解压到新文件夹，不要直接在压缩包中启动，也不要混合覆盖旧包。若文件再次消失，请检查安全软件的隔离记录。';$detail='缺少文件：'+$Message.Substring('Missing test dependency: '.Length);break
        }
        '^Test DLL checksum mismatch\.' {
            $code='T021';$cause='爬塔 DLL 与校验文件不配套，或 DLL 内容已损坏。';$fix='请从同一份完整包重新解压 erdtree_tower_party_test.dll 和 native.sha256，或重新解压整个包；不要单独混用其他版本的 DLL。';$detail='';break
        }
        '^(Empty asset manifest\.|Invalid asset manifest\.|Asset manifest path escapes stage\.)' {
            $code='T022';$cause='资源清单为空、格式损坏或含有无效路径。';$fix='请重新下载并完整解压同一版本整合包，恢复 assets.sha256.json 和 scene-assets；不要手动修改资源清单。';$detail='';break
        }
        '^Missing test asset: ' {
            $code='T023';$cause='资源清单中要求的文件没有解压出来或已丢失。';$fix='请完整解压整合包中的 scene-assets 文件夹，并保留其目录结构；不要只复制 DLL 或启动器。';$detail='缺少资源：'+$Message.Substring('Missing test asset: '.Length);break
        }
        '^Test asset checksum mismatch: ' {
            $code='T024';$cause='资源文件与当前整合包不配套、被修改或已损坏。';$fix='请把完整包重新解压到新文件夹，避免旧资源残留；使用补丁时应连同资源清单一起更新。';$detail='不匹配的资源：'+$Message.Substring('Test asset checksum mismatch: '.Length);break
        }
        '^Existing proxy loader detected\.' {
            $code='T030';$cause='游戏目录已有 dinput8.dll，可能与当前加载器冲突。';$fix='请先把 Game\dinput8.dll 移到其他文件夹备份，再用 Start-Seamless.cmd 启动；使用原来的 MOD 时可以放回。';$detail='启动器不会自动删除或覆盖这个文件。';break
        }
        '^Exit the running game before starting the isolated test\.' {
            $code='T031';$cause='游戏或无缝启动器仍在运行，暂时不能重复启动。';$fix='请退出游戏后等待十几秒。若 Steam 仍显示运行中，可先点“停止”，或在任务管理器中确认 eldenring.exe、ersc_launcher.exe 已退出，再重试。';$detail='';break
        }
        '^Extra native entry requires a single-line quoted path\.' {
            $code='T040';$cause='额外 DLL 配置中的 path 写法不正确。';$fix='请检查 runtime\test.me3 和 runtime\custom-natives.toml：path 必须写在一行内并带引号，例如 path = ''D:/Plugins/xxx.dll''。详见“额外DLL插件说明.txt”。';$detail='';break
        }
        '^Test loader exited: (-?\d+)' {
            $code='T050';$cause='ME3 加载器未能正常启动或运行游戏。';$detail='加载器退出码：'+$Matches[1];$fix='请确认 Steam 已启动并登录，检查窗口上方的 ME3 输出和安全软件隔离记录；修改过额外 DLL 配置时，请先检查插件路径。仍失败请截图整个窗口反馈。';break
        }
    }
    if($code -eq 'T099' -and $ErrorId -match 'UnauthorizedAccess|PermissionDenied') {
        $code='T060';$cause='启动器没有权限读写所需文件。';$fix='请将整合包完整解压到自己有读写权限的普通文件夹，确认文件未被其他程序占用；如有安全软件拦截，请检查其记录。'
    }
    [pscustomobject]@{Code=$code;Cause=$cause;Fix=$fix;Detail=$detail;RetryPath=$retry}
}
function Format-TowerStartupFailure($Failure) {
    $text="`r`n【爬塔启动失败 $($Failure.Code)】`r`n原因：$($Failure.Cause)`r`n处理方法：$($Failure.Fix)"
    if($Failure.Detail) {$text+="`r`n$($Failure.Detail)"}
    $text+="`r`n"
    $text
}

try {
$stage = $PSScriptRoot
$pathFile = Join-Path $stage 'GamePath.txt'
if (-not $ResetGamePath -and -not $GameDirectory -and (Test-Path -LiteralPath $pathFile)) { $GameDirectory = [IO.File]::ReadAllText($pathFile).Trim() }
while ($true) {
    if (-not $GameDirectory) {
        if ($CheckOnly) { throw 'Specify -GameDirectory for validation.' }
        $GameDirectory = Read-Host '请粘贴包含 eldenring.exe 的 Game 文件夹路径（输入 Q 退出）'
        if($null -eq $GameDirectory) {return}
        if ($GameDirectory.Trim() -eq 'q') { return }
    }
    try {
        $GameDirectory = $GameDirectory.Trim().Trim('"')
        if (-not $GameDirectory) { throw 'Game folder cannot be empty.' }
        if ($GameDirectory.Contains("'")) { throw 'Unsupported quote in game path.' }
        if (Test-Path -LiteralPath $GameDirectory -PathType Leaf) {
            if ([IO.Path]::GetFileName($GameDirectory) -ine 'eldenring.exe') { throw 'Select eldenring.exe or its Game folder.' }
            $GameDirectory = Split-Path -LiteralPath $GameDirectory
        }
        $exe = Join-Path $GameDirectory 'eldenring.exe'
        if (-not (Test-Path -LiteralPath $exe -PathType Leaf)) { throw 'Select the Game folder containing eldenring.exe.' }
        $GameDirectory = (Resolve-Path -LiteralPath $GameDirectory).Path
        if ((Get-FileHash -LiteralPath $exe).Hash -ne 'F9118FEA2CFD8FA3901E8060BFADB1552CB2471771DED8A5B662C4E298FFB328') { throw "Game version mismatch. Select the inspected executable 2.7.2.0 build. Current file version: $((Get-Item -LiteralPath $exe).VersionInfo.FileVersion)" }
        break
    } catch {
        if ($CheckOnly) { throw }
        $failure=Get-TowerStartupFailure -Message $_.Exception.Message -ErrorId $_.FullyQualifiedErrorId
        if(!$failure.RetryPath) {throw}
        Write-Host (Format-TowerStartupFailure $failure) -ForegroundColor Red
        $GameDirectory = $null
    }
}
$loader = Join-Path $stage 'loader/bin/me3.exe'
$native = Join-Path $stage 'erdtree_tower_party_test.dll'
$assets = Join-Path $stage 'scene-assets'
$seamless = Join-Path $GameDirectory 'SeamlessCoop'
foreach ($file in @($loader,$native,(Join-Path $stage 'native.sha256'),(Join-Path $stage 'assets.sha256.json'),(Join-Path $seamless 'ersc.dll'),(Join-Path $seamless 'ersc_settings.ini'),(Join-Path $GameDirectory 'eldenring.exe'))) {
    if (-not (Test-Path -LiteralPath $file -PathType Leaf)) { throw "Missing test dependency: $file" }
}
if ((Get-FileHash -LiteralPath (Join-Path $seamless 'ersc.dll')).Hash -ne 'FCD11A1849FD37851BB449BE30F1525CF498F63FD1FA568C4E15030EBE7CB251') { throw 'Expected inspected Seamless Co-op 2.0.1.' }
if ((Get-FileHash -LiteralPath $native).Hash -ne (Get-Content -LiteralPath (Join-Path $stage 'native.sha256') -Raw).Trim()) { throw 'Test DLL checksum mismatch.' }
try {$manifest = @(Get-Content -LiteralPath (Join-Path $stage 'assets.sha256.json') -Raw | ConvertFrom-Json | ForEach-Object { $_ })} catch {throw 'Invalid asset manifest.'}
if ($manifest.Count -eq 0) { throw 'Empty asset manifest.' }
foreach ($entry in $manifest) {
    if(!$entry -or $entry.Path -isnot [string] -or [string]::IsNullOrWhiteSpace($entry.Path) -or !$entry.SHA256) {throw 'Invalid asset manifest.'}
    $path = [IO.Path]::GetFullPath((Join-Path $assets $entry.Path))
    if (-not $path.StartsWith($assets + [IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)) { throw 'Asset manifest path escapes stage.' }
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {throw "Missing test asset: $($entry.Path)"}
    if ((Get-FileHash -LiteralPath $path).Hash -ne $entry.SHA256) { throw "Test asset checksum mismatch: $($entry.Path)" }
}
if (Test-Path -LiteralPath (Join-Path $GameDirectory 'dinput8.dll')) { throw 'Existing proxy loader detected.' }
foreach ($path in @($stage,$GameDirectory)) { if ($path.Contains("'")) { throw 'Unsupported quote in test path.' } }
$settings = [IO.File]::ReadAllText((Join-Path $seamless 'ersc_settings.ini'))
if ([regex]::Matches($settings,'(?m)^save_file_extension\s*=.*$').Count -ne 1) { throw 'Expected one save extension setting.' }
$settings = [regex]::Replace($settings,'(?m)^save_file_extension\s*=.*$','save_file_extension = ertowertest')
$settings = [regex]::Replace($settings,'(?m)^allow_invaders\s*=.*$','allow_invaders = 0')
$spectatePattern='(?m)^\s*always_spectate_on_death\s*=\s*\d+[^\r\n]*'
if ([regex]::Matches($settings,$spectatePattern).Count -ne 1) { throw 'Expected one always_spectate_on_death setting in Seamless 2.0.1.' }
$settings = [regex]::Replace($settings,$spectatePattern,'always_spectate_on_death = 1')
if ($CheckOnly) { [pscustomobject]@{Stage=$stage;AssetsChecked=$manifest.Count;SaveExtension='ertowertest';Deployed=$false};return }
if (Get-Process eldenring,ersc_launcher -ErrorAction SilentlyContinue) { throw 'Exit the running game before starting the isolated test.' }
[IO.File]::WriteAllText($pathFile,$GameDirectory,[Text.UTF8Encoding]::new($true))
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
    $stabilityArgs = if ($env:ERDTREE_TOWER_LOADER_STABILITY -ne '0') { @('--disable-arxan','true') } else { @() }
    & $loader launch --exe (Join-Path $GameDirectory 'eldenring.exe') --profile $profilePath --auto-detect --online false @stabilityArgs
    if ($LASTEXITCODE -ne 0) { throw "Test loader exited: $LASTEXITCODE" }
} finally {
    foreach ($key in $previous.Keys) {
        if ($null -eq $previous[$key]) { Remove-Item -LiteralPath "Env:$key" -ErrorAction SilentlyContinue }
        else { [Environment]::SetEnvironmentVariable($key,$previous[$key]) }
    }
}
} catch {
    $failure=Get-TowerStartupFailure -Message $_.Exception.Message -ErrorId $_.FullyQualifiedErrorId
    $message=Format-TowerStartupFailure $failure
    if($CheckOnly) {throw $message}
    Write-Host $message -ForegroundColor Red
    exit 1
}
