$ErrorActionPreference='Stop'
. "$PSScriptRoot/../distribution/Tower-Loader.ps1"
$root=Join-Path ([IO.Path]::GetTempPath()) ('tower-loader-test-'+[guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path "$root/assets","$root/SeamlessCoop" -Force | Out-Null
foreach($name in 'tower.dll','extra.dll','SeamlessCoop/ersc.dll'){[IO.File]::WriteAllText((Join-Path $root $name),'test')}
function Assert($condition,[string]$message){if(!$condition){throw $message}}
try {
    $profile=@'
profileVersion = "v1"
start_online = false
savefile = "ER_TOWER_PREVIEW.sl2"
[[supports]]
game = "eldenring"
[[natives]]
path = 'tower.dll'
load_after = [{ id = "ersc.dll", optional = false }]
[[natives]]
path = 'SeamlessCoop/ersc.dll'
[[natives]]
path = 'extra.dll'
enabled = false
[[packages]]
path = 'assets'
'@
    $ini=ConvertTo-TowerYafsmlConfig $profile $root
    Assert ($ini.Contains('replace_save_filename=ER_TOWER_PREVIEW.sl2')) 'Solo save was changed'
    Assert ($ini.IndexOf('ersc.dll') -lt $ini.IndexOf('tower.dll')) 'Seamless must load first'
    Assert (!$ini.Contains('extra.dll')) 'Disabled plugin was loaded'
    Assert ($ini.Contains('tower='+[IO.Path]::GetFullPath("$root/assets").Replace('\','/'))) 'Assets must use an absolute root'
    New-Item -ItemType Directory -Path "$root/scene-assets/locales/en-US/menu" -Force | Out-Null
    [IO.File]::WriteAllText("$root/tower-options.ini",'language=en-US')
    $localized=Add-TowerLanguagePackage $profile $root $root
    Assert ($localized.Contains("id = 'tower-language'")) 'English package missing'
    $localizedIni=ConvertTo-TowerYafsmlConfig $localized $root
    Assert ($localizedIni.IndexOf('overlay1=') -gt $localizedIni.IndexOf('tower=')) 'Language override must load after base assets'
    Assert ((Add-TowerLanguagePackage $localized $root $root) -eq $localized) 'Language package must be idempotent'
    [IO.File]::WriteAllText("$root/tower-options.ini",'language=zh-CN')
    Assert (!(Add-TowerLanguagePackage $localized $root $root).Contains("id = 'tower-language'")) 'Chinese selection retained English override'
    $game=Join-Path $root 'steamapps/common/ELDEN RING/Game'
    New-Item -ItemType Directory -Path $game -Force | Out-Null
    [IO.File]::WriteAllText("$root/tower-options.ini",'language=auto')
    [IO.File]::WriteAllText("$root/steamapps/appmanifest_1245620.acf",'"UserConfig" { "language" "english" }')
    Assert ((Add-TowerLanguagePackage $profile $root $game).Contains("id = 'tower-language'")) 'Auto did not read Steam English language'
    [IO.File]::WriteAllText("$root/steamapps/appmanifest_1245620.acf",'"UserConfig" { "language" "schinese" }')
    Assert (!(Add-TowerLanguagePackage $profile $root $game).Contains("id = 'tower-language'")) 'Auto selected English for Chinese game'
    $coop=ConvertTo-TowerYafsmlConfig ($profile -replace 'savefile = "ER_TOWER_PREVIEW.sl2"','') $root
    Assert ($coop.Contains("replace_save_filename=`n")) 'Seamless must keep its own save extension'
    Assert ($coop.Contains("replace_seamless_coop_save_filename=`n")) 'Seamless save name was overridden'
    $plugin=ConvertTo-TowerYafsmlConfig ($profile -replace 'enabled = false','enabled = true') $root
    Assert ($plugin.Contains('extra.dll')) 'Enabled plugin missing'
    foreach($bad in @(
        ($profile -replace 'ersc.dll", optional = false','missing.dll", optional = false'),
        ($profile -replace "path = 'assets'","path = 'missing-assets'"),
        ($profile -replace 'start_online = false','start_online = true'),
        ($profile -replace "path = 'tower.dll'","path = 'tower.dll'`ninitializer = { function = 'Init' }"),
        ($profile -replace "path = 'SeamlessCoop/ersc.dll'","path = 'SeamlessCoop/ersc.dll'`nload_after = [{ id = 'tower.dll', optional = false }]"))) {
        $rejected=$false;try{ConvertTo-TowerYafsmlConfig $bad $root | Out-Null}catch{$rejected=$true}
        Assert $rejected 'Invalid or unsupported profile was silently accepted'
    }
    $optional=ConvertTo-TowerYafsmlConfig ($profile -replace 'ersc.dll", optional = false','missing.dll", optional = true') $root
    Assert ($optional.Contains('tower.dll')) 'Missing optional dependency blocked launch'
    Copy-Item -LiteralPath "$PSScriptRoot/../distribution/Start-Tower.ps1" -Destination "$root/Start-Tower.ps1"
    [IO.File]::WriteAllText("$root/eldenring.exe",'fixture')
    $entry='param($GameDir,$Loader,[switch]$CheckOnly,$PartyConfig); if(!$CheckOnly){throw "Must not launch"}; Write-Output $Loader'
    [IO.File]::WriteAllText("$root/Launch-Preview.ps1",$entry)
    [IO.File]::WriteAllText("$root/Launch-Seamless-Tower.ps1",$entry)
    foreach($mode in 'Solo','Seamless'){foreach($loader in 'ME3','YAFSML'){
        $result=& "$root/Start-Tower.ps1" -GameDir $root -Mode $mode -Loader $loader -CheckOnly
        Assert ($result -eq $loader) "Loader selection lost for $mode"
    }}
    Assert (!(Test-Path -LiteralPath "$root/GamePath.txt")) 'CheckOnly unexpectedly changed settings'
    Write-Output 'YAFSML profile tests passed: save names, order, plugins, absolute paths, invalid settings and cycle rejection.'
} finally {
    $resolved=[IO.Path]::GetFullPath($root)
    $temp=[IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\')+'\'
    if(!$resolved.StartsWith($temp,[StringComparison]::OrdinalIgnoreCase) -or !(Split-Path -Leaf $resolved).StartsWith('tower-loader-test-')){throw 'Unsafe cleanup path'}
    Remove-Item -LiteralPath $resolved -Recurse -Force
}
