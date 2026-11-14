function Assert-TowerNetwork([string]$GameDir, [string]$Network) {
    if ($Network -notin @('Steam','OnlineFix')) { throw 'Unknown network mode.' }
    if ($Network -eq 'Steam') {
        foreach ($name in @('OnlineFix64.dll','OnlineFix.ini')) {
            if (Test-Path -LiteralPath (Join-Path $GameDir $name)) { throw 'Official mode cannot use an OnlineFix game directory. Select the separate official Game folder.' }
        }
        return
    }
    $hashes = @{
        'OnlineFix64.dll' = '71a4f566abbf2ceb3b0d6159cd7950d120593c5badda4dde2711671957f16003'
        'steam_api64.dll' = 'ca1fa9a7609ab10b7926400559cf073e5888423cc156af72c6027d72a89eea73'
        'winmm.dll' = '54a6ece54641ee6e437caa351e493ddf4435f3e23e3d4ab4655c35f57b94fc59'
    }
    foreach ($name in $hashes.Keys) {
        $file = Join-Path $GameDir $name
        if (!(Test-Path -LiteralPath $file -PathType Leaf)) { throw "Missing $name. Select a separate Game directory with the supplied OnlineFix 2.0.3 patch installed." }
        if ((Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash -ine $hashes[$name]) { throw "Unsupported OnlineFix component: $name. This profile supports the supplied 2.0.3 patch only." }
    }
    $ini = [IO.File]::ReadAllText((Join-Path $GameDir 'OnlineFix.ini'))
    foreach ($setting in @(@('RealAppId','1245620'), @('FakeAppId','480'))) {
        $entries = [regex]::Matches($ini, '(?mi)^\s*' + $setting[0] + '\s*=\s*([^\r\n;#]+)')
        if ($entries.Count -ne 1 -or $entries[0].Groups[1].Value.Trim() -ne $setting[1]) { throw "Unexpected OnlineFix $($setting[0])." }
    }
    if (Test-Path -LiteralPath (Join-Path $GameDir 'steam_api64_o.dll')) { throw 'Mixed Steam API proxies detected. Use a separate OnlineFix Game directory without steam_api64_o.dll.' }
    $null = Get-TowerOnlineFixDllList $GameDir
}

function Get-TowerOnlineFixDllList([string]$GameDir) {
    $file = Join-Path $GameDir 'dlllist.txt'
    $entries = @([IO.File]::ReadAllLines($file) | ForEach-Object { $_.Trim().Replace('/','\') } | Where-Object { $_ })
    if (@($entries | Where-Object { $_ -ieq 'OnlineFix64.dll' }).Count -ne 1 -or
        @($entries | Where-Object { $_ -ine 'OnlineFix64.dll' -and $_ -ine 'SeamlessCoop\ersc.dll' }).Count -ne 0 -or
        @($entries | Where-Object { $_ -ieq 'SeamlessCoop\ersc.dll' }).Count -gt 1) {
        throw 'Unsupported dlllist.txt. Expected OnlineFix64.dll and optional SeamlessCoop\ersc.dll only.'
    }
    return $entries
}

function Set-TowerOnlineFixDllList([string]$GameDir) {
    if (Get-Process eldenring,ersc_launcher -ErrorAction SilentlyContinue) { throw 'Exit the game before preparing the OnlineFix loader.' }
    Assert-TowerNetwork $GameDir 'OnlineFix'
    $entries = @(Get-TowerOnlineFixDllList $GameDir)
    if ($entries.Count -eq 1) { return }
    $file = Join-Path $GameDir 'dlllist.txt'
    $backup = $file + '.before-tower.bak'
    $bytes = [IO.File]::ReadAllBytes($file)
    if (Test-Path -LiteralPath $backup) {
        if ([Convert]::ToBase64String([IO.File]::ReadAllBytes($backup)) -cne [Convert]::ToBase64String($bytes)) { throw 'Existing dlllist backup differs; no changes made.' }
    } else {
        $stream = [IO.File]::Open($backup, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write)
        try { $stream.Write($bytes, 0, $bytes.Length) } finally { $stream.Dispose() }
    }
    $temporary = $file + '.tower.tmp'
    $stream = [IO.File]::Open($temporary, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write)
    try {
        $value = [Text.Encoding]::ASCII.GetBytes("OnlineFix64.dll`r`n")
        $stream.Write($value, 0, $value.Length)
    } finally { $stream.Dispose() }
    if ([Convert]::ToBase64String([IO.File]::ReadAllBytes($file)) -cne [Convert]::ToBase64String($bytes)) {
        [IO.File]::Delete($temporary)
        throw 'dlllist.txt changed during preparation; no changes applied.'
    }
    [IO.File]::Replace($temporary, $file, [NullString]::Value)
    Write-Host 'OnlineFix proxy enabled; Seamless is loaded once by the tower loader. Original dlllist.txt backed up.'
}
