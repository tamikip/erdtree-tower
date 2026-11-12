param([string]$InstallDir=$PSScriptRoot,[string]$GameDir)
$ErrorActionPreference='Stop'
foreach($module in @('Microsoft.PowerShell.Management','Microsoft.PowerShell.Utility')){Import-Module ([IO.Path]::Combine($PSHOME,'Modules',$module,($module+'.psd1'))) -ErrorAction Stop}
$root=[IO.Path]::GetFullPath($InstallDir)
if(!(Test-Path -LiteralPath (Join-Path $root 'asset-patches/manifest.json'))){throw 'Extract this diagnostic package into the launcher folder first.'}
if(Get-Process eldenring,Tower.AssetRestore -ErrorAction SilentlyContinue){throw 'Save and exit the game and wait for resource generation to finish before diagnosing.'}
if(!$GameDir -and (Test-Path -LiteralPath (Join-Path $root 'GamePath.txt'))){$GameDir=[IO.File]::ReadAllText((Join-Path $root 'GamePath.txt')).Trim().Trim('"')}
if(!$GameDir -or !(Test-Path -LiteralPath (Join-Path $GameDir 'eldenring.exe'))){
    Add-Type -AssemblyName System.Windows.Forms
    $dialog=New-Object Windows.Forms.OpenFileDialog
    $dialog.Title='Select eldenring.exe in the game installation folder'
    $dialog.Filter='Elden Ring (eldenring.exe)|eldenring.exe'
    try {if($dialog.ShowDialog() -ne [Windows.Forms.DialogResult]::OK){Write-Output 'Cancelled. No diagnostic started.';exit 2};$GameDir=[IO.Path]::GetDirectoryName($dialog.FileName)}finally{$dialog.Dispose()}
}
$GameDir=[IO.Path]::GetFullPath($GameDir).TrimEnd('\','/')
if(!(Test-Path -LiteralPath (Join-Path $GameDir 'Data0.bdt')) -or !(Test-Path -LiteralPath (Join-Path $GameDir 'oo2core_6_win64.dll'))){throw 'Selected folder is missing Data0.bdt or oo2core_6_win64.dll. Choose the original game Game folder.'}
$output=Join-Path $root ('launcher-data/dcx-diagnostic/'+[DateTime]::Now.ToString('yyyyMMdd-HHmmss-fff'))
[IO.Directory]::CreateDirectory($output) | Out-Null
$report=Join-Path $output 'report.txt'
$utf8=[Text.UTF8Encoding]::new($false)
[IO.File]::WriteAllText($report,"DCX diagnostic - no game, settings or asset files modified.`r`n",$utf8)
function Record([string]$line){[IO.File]::AppendAllText($report,$line+"`r`n",$utf8);Write-Output $line}
Record ('PowerShell: '+$PSVersionTable.PSVersion+'; OS: '+[Environment]::OSVersion+'; x64: '+[Environment]::Is64BitProcess)
try {
    Import-Module ([IO.Path]::Combine($PSHOME,'Modules/CimCmdlets/CimCmdlets.psd1'))
    foreach($cpu in Get-CimInstance Win32_Processor){Record ('CPU: '+$cpu.Name)}
    foreach($page in Get-CimInstance Win32_PageFileUsage){Record ('Pagefile: allocated_MB='+$page.AllocatedBaseSize+' used_MB='+$page.CurrentUsage+' peak_MB='+$page.PeakUsage)}
}catch{Record ('System details unavailable: '+$_.Exception.Message)}
$exe=Join-Path $PSScriptRoot 'Tower.AssetDiagnose.exe'
$index=0
foreach($mode in @('original','trace','original','trace')){
    $index++
    $stdout=Join-Path $output ($index.ToString()+'-'+$mode+'-stdout.log')
    $stderr=Join-Path $output ($index.ToString()+'-'+$mode+'-stderr.log')
    Record ('Attempt '+$index+': '+$mode)
    try {
        $start=New-Object Diagnostics.ProcessStartInfo
        $start.FileName=$exe;$start.Arguments='"'+$root+'" '+$mode+' "'+$GameDir+'"'
        $start.UseShellExecute=$false;$start.CreateNoWindow=$true
        $start.RedirectStandardOutput=$true;$start.RedirectStandardError=$true
        $start.StandardOutputEncoding=$utf8;$start.StandardErrorEncoding=$utf8
        $child=[Diagnostics.Process]::Start($start)
        $outTask=$child.StandardOutput.ReadToEndAsync();$errTask=$child.StandardError.ReadToEndAsync()
        if(!$child.WaitForExit(60000)){
            $child.Kill();$child.WaitForExit()
            Record 'Result: TIMEOUT after 60 seconds; diagnostic child stopped.'
        }else{Record ('Exit code: '+$child.ExitCode)}
        [IO.File]::WriteAllText($stdout,$outTask.GetAwaiter().GetResult(),$utf8)
        [IO.File]::WriteAllText($stderr,$errTask.GetAwaiter().GetResult(),$utf8)
        $child.Dispose()
    }catch{Record ('Diagnostic launch failed: '+$_.Exception.Message)}
    foreach($file in @($stdout,$stderr)){if(Test-Path -LiteralPath $file){[IO.File]::AppendAllText($report,[IO.File]::ReadAllText($file)+"`r`n",$utf8)}}
}
Record 'Done. Send report.txt; do not change pagefile or disable security software based on this report alone.'
Write-Output ('Report: '+$report)
