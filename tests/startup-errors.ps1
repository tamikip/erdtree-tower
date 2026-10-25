param([Parameter(Mandatory=$true)][string]$Launcher, [Parameter(Mandatory=$true)][string]$FixtureRoot)
$ErrorActionPreference='Stop'
$tokens=$null;$parseErrors=$null
$ast=[Management.Automation.Language.Parser]::ParseFile($Launcher,[ref]$tokens,[ref]$parseErrors)
if ($parseErrors.Count) {throw $parseErrors[0]}
$functions=@($ast.FindAll({param($node) $node -is [Management.Automation.Language.FunctionDefinitionAst] -and $node.Name -in @('Get-TowerStartupFailure','Format-TowerStartupFailure')},$false))
if ($functions.Count -ne 2) {throw 'Missing user-facing startup diagnostics'}
foreach($function in $functions) {. ([scriptblock]::Create($function.Extent.Text))}
$cases=@(
    @('Game folder cannot be empty.','T001'),
    @('Select the Game folder containing eldenring.exe.','T002'),
    @('Select eldenring.exe or its Game folder.','T002'),
    @('Game version mismatch. Select the inspected executable 2.7.2.0 build. Current file version: 2.7.1.0','T003'),
    @('Unsupported quote in game path.','T004'),
    @('Unsupported quote in test path.','T004'),
    @('Missing test dependency: F:\Game\SeamlessCoop\ersc.dll','T010'),
    @('Missing test dependency: F:\Game\SeamlessCoop\ersc_settings.ini','T011'),
    @('Expected inspected Seamless Co-op 2.0.1.','T012'),
    @('Expected one save extension setting.','T013'),
    @('Expected one always_spectate_on_death setting in Seamless 2.0.1.','T014'),
    @('Missing test dependency: F:\Tower\loader\bin\me3.exe','T020'),
    @('Missing test dependency: F:\Tower\erdtree_tower_party_test.dll','T020'),
    @('Test DLL checksum mismatch.','T021'),
    @('Empty asset manifest.','T022'),
    @('Invalid asset manifest.','T022'),
    @('Asset manifest path escapes stage.','T022'),
    @('Missing test asset: nested/asset.bin','T023'),
    @('Test asset checksum mismatch: nested/asset.bin','T024'),
    @('Existing proxy loader detected.','T030'),
    @('Exit the running game before starting the isolated test.','T031'),
    @('Extra native entry requires a single-line quoted path.','T040'),
    @('Test loader exited: 7','T050'),
    @('Unexpected fixture failure','T099')
)
foreach($case in $cases) {
    $failure=Get-TowerStartupFailure -Message $case[0]
    if($failure.Code -ne $case[1]) {throw "Wrong classification: $($case[0]) -> $($failure.Code)"}
    $text=Format-TowerStartupFailure -Failure $failure
    foreach($label in @('原因：','处理方法：',$case[1])) {if(!$text.Contains($label)) {throw "Missing guidance: $label"}}
    if($text -match 'CategoryInfo|所在位置') {throw 'PowerShell stack leaked into guidance'}
}
$permission=Get-TowerStartupFailure -Message 'Access denied' -ErrorId 'UnauthorizedAccessException'
if($permission.Code -ne 'T060') {throw 'Permission failure is not actionable'}
if(Test-Path -LiteralPath $FixtureRoot) {throw 'Fixture directory must be new'}
New-Item -ItemType Directory -Path $FixtureRoot | Out-Null
$fixture=Join-Path $FixtureRoot 'stage';$game=Join-Path $FixtureRoot 'Game'
foreach($folder in @($fixture,$game,(Join-Path $fixture 'loader/bin'),(Join-Path $fixture 'scene-assets'),(Join-Path $game 'SeamlessCoop'))) {New-Item -ItemType Directory -Path $folder -Force | Out-Null}
Copy-Item -LiteralPath $Launcher -Destination (Join-Path $fixture 'Launch-Tower.ps1')
New-Item -ItemType HardLink -Path (Join-Path $game 'eldenring.exe') -Target 'F:\SteamLibrary\steamapps\common\ELDEN RING\Game\eldenring.exe' | Out-Null
New-Item -ItemType HardLink -Path (Join-Path $game 'SeamlessCoop/ersc.dll') -Target 'F:\SteamLibrary\steamapps\common\ELDEN RING\Game\SeamlessCoop\ersc.dll' | Out-Null
foreach($name in @('loader/bin/me3.exe','erdtree_tower_party_test.dll','scene-assets/fixture.bin')) {[IO.File]::WriteAllText((Join-Path $fixture $name),'fixture')}
$nativeHash=(Get-FileHash -LiteralPath (Join-Path $fixture 'erdtree_tower_party_test.dll')).Hash
[IO.File]::WriteAllText((Join-Path $fixture 'native.sha256'),$nativeHash)
$settings="save_file_extension = co2`r`nalways_spectate_on_death = 0`r`ncooppassword = private-fixture-password`r`n"
$settingsPath=Join-Path $game 'SeamlessCoop/ersc_settings.ini'
[IO.File]::WriteAllText($settingsPath,$settings)
$manifestPath=Join-Path $fixture 'assets.sha256.json'
$assetHash=(Get-FileHash -LiteralPath (Join-Path $fixture 'scene-assets/fixture.bin')).Hash
$manifest=ConvertTo-Json -InputObject @(@{Path='fixture.bin';SHA256=$assetHash})
[IO.File]::WriteAllText($manifestPath,$manifest)
$entry=Join-Path $fixture 'Launch-Tower.ps1'
function Expect-CheckFailure([string]$code) {
    $caught=$false
    try {& $entry -GameDirectory $game -CheckOnly | Out-Null} catch {
        $caught=$true
        if(!$_.Exception.Message.Contains($code)) {throw "Expected $code, got $($_.Exception.Message)"}
        if($_.Exception.Message.Contains('private-fixture-password')) {throw 'Private configuration leaked'}
    }
    if(!$caught) {throw "Expected launch guard: $code"}
}
$result=& $entry -GameDirectory $game -CheckOnly
if($result.AssetsChecked -ne 1 -or $result.Deployed) {throw 'Valid CheckOnly preflight changed'}
$exeResult=& $entry -GameDirectory (Join-Path $game 'eldenring.exe') -CheckOnly
if($exeResult.AssetsChecked -ne 1) {throw 'Selecting eldenring.exe directly failed'}
[IO.File]::WriteAllText((Join-Path $fixture 'scene-assets/second.bin'),'fixture')
$multiple=ConvertTo-Json -InputObject @(@{Path='fixture.bin';SHA256=$assetHash},@{Path='second.bin';SHA256=$assetHash})
[IO.File]::WriteAllText($manifestPath,$multiple)
$multiResult=& $entry -GameDirectory $game -CheckOnly
if($multiResult.AssetsChecked -ne 2) {throw 'Multiple assets must be enumerated on Windows PowerShell 5.1'}
[IO.File]::WriteAllText($manifestPath,$manifest)
[IO.File]::WriteAllText($settingsPath,'cooppassword = private-fixture-password');Expect-CheckFailure 'T013'
[IO.File]::WriteAllText($settingsPath,$settings+"save_file_extension = duplicate`r`n");Expect-CheckFailure 'T013'
[IO.File]::WriteAllText($settingsPath,"save_file_extension = co2`r`n");Expect-CheckFailure 'T014'
[IO.File]::WriteAllText($settingsPath,$settings+"always_spectate_on_death = 1`r`n");Expect-CheckFailure 'T014'
[IO.File]::WriteAllText($settingsPath,$settings)
foreach($bad in @('[]','{broken','[{"Path":"../escape.bin","SHA256":"00"}]')) {[IO.File]::WriteAllText($manifestPath,$bad);Expect-CheckFailure 'T022'}
[IO.File]::WriteAllText($manifestPath,'[{"Path":"missing.bin","SHA256":"00"}]');Expect-CheckFailure 'T023'
[IO.File]::WriteAllText($manifestPath,'[{"Path":"fixture.bin","SHA256":"00"}]');Expect-CheckFailure 'T024'
[IO.File]::WriteAllText($manifestPath,$manifest)
[IO.File]::WriteAllText((Join-Path $fixture 'native.sha256'),'wrong');Expect-CheckFailure 'T021'
[IO.File]::WriteAllText((Join-Path $fixture 'native.sha256'),$nativeHash)
[IO.File]::WriteAllText((Join-Path $game 'dinput8.dll'),'fixture');Expect-CheckFailure 'T030'
if(Test-Path (Join-Path $fixture 'runtime')) {throw 'CheckOnly created private runtime'}
if(Test-Path (Join-Path $fixture 'GamePath.txt')) {throw 'CheckOnly wrote game path'}
$wrongGame=Join-Path $FixtureRoot 'wrong-game';New-Item -ItemType Directory -Path $wrongGame | Out-Null
[IO.File]::WriteAllText((Join-Path $wrongGame 'eldenring.exe'),'wrong')
$psi=New-Object Diagnostics.ProcessStartInfo
$psi.FileName=(Get-Process -Id $PID).Path
$psi.Arguments='-NoProfile -ExecutionPolicy Bypass -File "'+$entry+'" -GameDirectory "'+$wrongGame+'"'
$psi.UseShellExecute=$false;$psi.CreateNoWindow=$true;$psi.RedirectStandardOutput=$true;$psi.RedirectStandardError=$true;$psi.RedirectStandardInput=$true
$process=[Diagnostics.Process]::Start($psi);$process.StandardInput.Close()
if(!$process.WaitForExit(10000)) {$process.Kill();throw 'Version mismatch unexpectedly prompted or hung'}
$console=$process.StandardOutput.ReadToEnd()+$process.StandardError.ReadToEnd()
if($process.ExitCode -ne 1 -or !$console.Contains('T003') -or $console -match 'CategoryInfo|FullyQualifiedErrorId|所在位置') {throw "Bad console failure: $console"}
Write-Output "PASS: $($cases.Count) error mappings, 12 failure fixtures, valid preflight, clean console exit, no private writes."
