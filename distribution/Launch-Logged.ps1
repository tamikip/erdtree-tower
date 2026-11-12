param(
    [Parameter(Mandatory=$true)][string]$Entry,
    [ValidateSet('Solo','Seamless')][string]$Mode = 'Seamless',
    [ValidateSet('ME3','YAFSML')][string]$Loader = 'ME3',
    [Parameter(Mandatory=$true)][string]$GameDir,
    [Parameter(Mandatory=$true)][string]$ErrorFile,
    [Parameter(Mandatory=$true)][string]$LogDirectory
)
$ErrorActionPreference = 'Stop'
try {
    [void][IO.Directory]::CreateDirectory($LogDirectory)
    $bootstrap = 'foreach ($module in @(''Microsoft.PowerShell.Management'',''Microsoft.PowerShell.Utility'')) { Import-Module ([IO.Path]::Combine($PSHOME,''Modules'',$module,($module + ''.psd1''))) -ErrorAction Stop }; '
    & ([ScriptBlock]::Create($bootstrap))
    function Literal([string]$Value) { "'" + $Value.Replace("'", "''") + "'" }
    $command = $bootstrap + '$ProgressPreference = ''SilentlyContinue''; [Console]::OutputEncoding = [Text.UTF8Encoding]::new($false); & ' + (Literal $Entry) + ' -Mode ' + (Literal $Mode) + ' -GameDir ' + (Literal $GameDir) + ' -ErrorFile ' + (Literal $ErrorFile)
    $command += ' -Loader ' + (Literal $Loader)
    $command += '; $ok = $?; if ($null -ne $LASTEXITCODE) { exit $LASTEXITCODE }; if (-not $ok) { exit 1 }; exit 0'
    $encoded = [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($command))
    $shell = (Get-Process -Id $PID).Path
    $worker = Start-Process -FilePath $shell -ArgumentList @('-NoProfile','-NonInteractive','-ExecutionPolicy','Bypass','-EncodedCommand',$encoded) -WorkingDirectory (Split-Path -LiteralPath $Entry) -WindowStyle Hidden -RedirectStandardOutput (Join-Path $LogDirectory 'stdout.log') -RedirectStandardError (Join-Path $LogDirectory 'stderr.log') -PassThru
    $handle = $worker.Handle
    $worker.WaitForExit()
    $code = $worker.ExitCode
    [IO.File]::WriteAllText((Join-Path $LogDirectory 'exit-code.txt'),[string]$code)
    exit $code
} catch {
    [IO.File]::WriteAllText($ErrorFile,$_.Exception.Message)
    exit 1
}
