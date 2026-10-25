param([string]$ProjectRoot = (Split-Path $PSScriptRoot -Parent), [string]$GameDir)
$ErrorActionPreference = 'Stop'
$locations = @($ProjectRoot, (Join-Path $ProjectRoot 'release-preview'))
$checked = 0
foreach ($location in $locations) {
    if (-not (Test-Path -LiteralPath $location -PathType Container)) { continue }
    foreach ($script in (Get-ChildItem -LiteralPath $location -Filter '*.ps1' -File)) {
        $bytes = [System.IO.File]::ReadAllBytes($script.FullName)
        $hasBom = $bytes.Length -ge 3 -and $bytes[0] -eq 239 -and $bytes[1] -eq 187 -and $bytes[2] -eq 191
        $nonAscii = @($bytes | Where-Object { $_ -gt 127 }).Count -gt 0
        if ($nonAscii -and -not $hasBom) { throw "Non-ASCII PowerShell script requires UTF-8 BOM: $($script.FullName)" }
        $parseErrors = $null
        $parseTokens = $null
        [System.Management.Automation.Language.Parser]::ParseFile($script.FullName, [ref]$parseTokens, [ref]$parseErrors) | Out-Null
        if ($parseErrors.Count -gt 0) { throw ($parseErrors | Out-String) }
        $checked++
    }
}
$launcher = Join-Path $ProjectRoot 'Launch-Preview.ps1'
$invalidGameDir = Join-Path $ProjectRoot '__missing_game_for_launcher_test__'
if (Test-Path -LiteralPath $invalidGameDir) { throw 'Invalid-path fixture unexpectedly exists' }
$caught = $false
try { & $launcher -GameDir $invalidGameDir } catch {
    if ($_.Exception -is [System.Management.Automation.ParseException]) { throw }
    if ($_.Exception.Message -notmatch 'GameDir') { throw }
    $caught = $true
}
if (-not $caught) { throw 'Launcher did not reject a missing game directory' }
if ($GameDir) {
    foreach ($location in $locations) {
        $entry = Join-Path $location 'Launch-Preview.ps1'
        if (-not (Test-Path -LiteralPath $entry -PathType Leaf)) { continue }
        $result = & $entry -GameDir $GameDir -CheckOnly
        if (-not (Test-Path -LiteralPath $result.LoaderPath -PathType Leaf)) { throw 'Loader resolution failed' }
        if (-not (Test-Path -LiteralPath $result.DllPath -PathType Leaf)) { throw 'DLL resolution failed' }
        if (-not (Test-Path -LiteralPath $result.ProfilePath -PathType Leaf)) { throw 'Profile resolution failed' }
        $cmdEntry = Join-Path $location 'Launch-Preview.cmd'
        if (Test-Path -LiteralPath $cmdEntry -PathType Leaf) {
            $cmdOutput = & $cmdEntry -GameDir $GameDir -CheckOnly
            if ($LASTEXITCODE -ne 0) { throw "CMD launcher preflight failed: $cmdEntry" }
        }
        Write-Output "Validated entry: $entry -> $($result.PackageDirectory)"
    }
}
Write-Output "Launcher compatibility passed: PowerShell $($PSVersionTable.PSVersion), $checked scripts; missing-game guard executed."
