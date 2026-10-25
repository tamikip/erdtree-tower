$ErrorActionPreference='Stop'
$root=Split-Path $PSScriptRoot -Parent
$testRoot=Join-Path ([IO.Path]::GetTempPath()) ('tower-profile-'+[guid]::NewGuid())
New-Item -ItemType Directory -Path $testRoot | Out-Null
foreach ($relative in @('Launch-Seamless-Tower.ps1','release-preview/Launch-Seamless-Tower.ps1','Launch-PartyTest.ps1','dist/ErdtreeTower-Multiplayer-Test-06/Launch-Tower.ps1','dist/ErdtreeTower-Test06-Full-20261012/Launch-Tower.ps1')) {
    $source=[IO.File]::ReadAllText((Join-Path $root $relative))
    $block=[regex]::Match($source,'(?s)# Custom DLL configuration\r?\n(.*?)# End custom DLL configuration')
    if (!$block.Success) {throw "Missing custom configuration support: $relative"}
    $runtime=Join-Path $testRoot ([guid]::NewGuid().ToString())
    New-Item -ItemType Directory -Path $runtime | Out-Null
    $editable=Join-Path $runtime 'test.me3'
    $config=$profile="profileVersion = 'v1'"
    $configPath=$profilePath=$editable
    . ([scriptblock]::Create($block.Groups[1].Value))
    if ([IO.File]::ReadAllText($editable) -ne "profileVersion = 'v1'") {throw 'Initial editable profile missing'}
    $main=@"
profileVersion = 'v1'
# User comment
[[natives]]
path = 'D:/直播插件/plugin.dll'
enabled = true
load_after = [{ id = 'ersc.dll', optional = false }]
"@
    [IO.File]::WriteAllText($editable,$main,[Text.UTF8Encoding]::new($false))
    $custom=@"
[[natives]]
path = 'D:/直播插件/plugin.dll'
enabled = false
[[natives]]
path = 'D:/Second/plugin2.dll'
"@
    $file=Join-Path $runtime 'custom-natives.toml'
    [IO.File]::WriteAllText($file,$custom,[Text.UTF8Encoding]::new($false))
    $mainHash=(Get-FileHash -LiteralPath $editable).Hash
    $customHash=(Get-FileHash -LiteralPath $file).Hash
    1..3 | ForEach-Object {
        $config=$profile="profileVersion = 'v1'"
        $configPath=$profilePath=$editable
        . ([scriptblock]::Create($block.Groups[1].Value))
        $merged=if ($relative -like '*Seamless-Tower*') {$config} else {$profile}
        $outputPath=if ($relative -like '*Seamless-Tower*') {$configPath} else {$profilePath}
        if ($outputPath -eq $editable -or !(Split-Path $outputPath -Leaf).EndsWith('.launch.me3')) {throw 'Launch output still overwrites editable profile'}
        [IO.File]::WriteAllText($outputPath,$merged,[Text.UTF8Encoding]::new($false))
        if (!$merged.StartsWith($main) -or !$merged.Contains('plugin2.dll')) {throw 'User native entry or options lost'}
        if ((Get-FileHash -LiteralPath $editable).Hash -ne $mainHash) {throw 'Editable .me3 overwritten'}
        if ((Get-FileHash -LiteralPath $file).Hash -ne $customHash) {throw 'Custom natives overwritten'}
        if ([regex]::Matches($merged,'\[\[natives\]\]').Count -ne 2) {throw 'Duplicate native entry'}
    }
    $tokens=$null;$errors=$null
    [void][Management.Automation.Language.Parser]::ParseInput($source,[ref]$tokens,[ref]$errors)
    if ($errors.Count) {throw $errors[0]}
}
Write-Output 'Five launchers passed: creation, direct .me3 edits, repeated launches, custom merge, duplicate suppression and syntax.'
