$ErrorActionPreference = 'Stop'
$root = Join-Path ([IO.Path]::GetTempPath()) ('tower launch test '+[guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $root | Out-Null
$entry = Join-Path $root "fake 'game'.ps1"
@'
param($Mode,$GameDir,$ErrorFile,$Loader)
if($Loader -ne 'YAFSML'){throw 'Selected loader was lost'}
Write-Output 'stdout-marker'
[Console]::Error.WriteLine('ERROR stderr-marker')
exit 17
'@ | Set-Content -LiteralPath $entry -Encoding utf8
$wrapper = Join-Path $PSScriptRoot '../distribution/Launch-Logged.ps1'
& powershell.exe -NoProfile -ExecutionPolicy Bypass -File $wrapper -Entry $entry -Loader YAFSML -GameDir 'F:\Game With Spaces' -ErrorFile "$root/error.txt" -LogDirectory "$root/logs"
if ($LASTEXITCODE -ne 17) {throw "Wrong exit code: $LASTEXITCODE; $(Get-Content $root/error.txt -ErrorAction SilentlyContinue)"}
if ((Get-Content "$root/logs/stdout.log" -Raw) -notmatch 'stdout-marker') {throw 'Missing stdout'}
if ((Get-Content "$root/logs/stderr.log" -Raw) -notmatch 'stderr-marker') {throw 'Missing stderr'}
Write-Output 'PASS: hidden worker captures stdout, stderr and exit code with quoted paths.'
@'
param($Mode,$GameDir,$ErrorFile)
[IO.File]::WriteAllText($ErrorFile,'missing dependency')
throw 'test startup failure'
'@ | Set-Content -LiteralPath $entry -Encoding utf8
& powershell.exe -NoProfile -ExecutionPolicy Bypass -File $wrapper -Entry $entry -GameDir 'F:\Game With Spaces' -ErrorFile "$root/error.txt" -LogDirectory "$root/failure"
if ($LASTEXITCODE -eq 0) {throw 'Script exception incorrectly reported success'}
if ((Get-Content "$root/error.txt" -Raw) -ne 'missing dependency') {throw 'Missing script error summary'}
Write-Output 'PASS: script exceptions report failure and preserve the summary.'
@'
param($Mode,$GameDir,$ErrorFile,$Loader)
New-Item -ItemType Directory -Path ([IO.Path]::Combine($PSScriptRoot,'worker-module-check')) -Force | Out-Null
Write-Output 'modules-ready'
exit 0
'@ | Set-Content -LiteralPath $entry -Encoding utf8
function Quote-Literal([string]$Value) { "'" + $Value.Replace("'", "''") + "'" }
$isolated = '$PSModuleAutoLoadingPreference = ''None''; $env:PSModulePath = ''Z:\missing-tower-test-modules''; & ' + (Quote-Literal $wrapper) + ' -Entry ' + (Quote-Literal $entry) + ' -GameDir ''F:\Game With Spaces'' -ErrorFile ' + (Quote-Literal "$root/module-error.txt") + ' -LogDirectory ' + (Quote-Literal "$root/modules")
$encoded = [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($isolated))
& powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -EncodedCommand $encoded
if ($LASTEXITCODE -ne 0) { throw "Built-in module bootstrap failed: $(Get-Content "$root/module-error.txt" -ErrorAction SilentlyContinue)" }
if ((Get-Content "$root/modules/stdout.log" -Raw) -notmatch 'modules-ready') { throw 'Worker did not load built-in modules' }
if (!(Test-Path -LiteralPath "$root/worker-module-check")) { throw 'New-Item was unavailable in the worker' }
Write-Output 'PASS: wrapper and worker run with disabled autoload and an invalid inherited module path.'
