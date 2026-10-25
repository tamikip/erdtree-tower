param([Parameter(Mandatory=$true)][string]$SettingsPath)
$ErrorActionPreference='Stop'
if(Get-Process eldenring -ErrorAction SilentlyContinue){throw 'Exit Elden Ring before changing spectator settings.'}
$path=(Resolve-Path -LiteralPath $SettingsPath).Path
$text=[IO.File]::ReadAllText($path)
$pattern='(?m)^\s*always_spectate_on_death\s*=\s*\d+[^\r\n]*'
if([regex]::Matches($text,$pattern).Count -ne 1){throw 'Expected exactly one always_spectate_on_death setting; no changes made.'}
$updated=[regex]::Replace($text,$pattern,'always_spectate_on_death = 1')
if($updated -eq $text){Write-Host 'Death spectating already enabled.';return}
$backup=$path+'.before-spectate-'+[Guid]::NewGuid().ToString('N')+'.bak'
Copy-Item -LiteralPath $path -Destination $backup
[IO.File]::WriteAllText($path,$updated,[Text.UTF8Encoding]::new($false))
Write-Host "Death spectating enabled. Backup: $backup"
