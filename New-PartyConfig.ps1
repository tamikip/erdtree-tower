param(
    [Parameter(Mandatory=$true)][string]$HostAddress,
    [ValidateRange(1024,65535)][int]$Port=47931,
    [string]$OutputDirectory=(Join-Path $PSScriptRoot 'party-config')
)
$ErrorActionPreference='Stop'
$address=[Net.IPAddress]::Parse($HostAddress)
if ($address.AddressFamily -ne [Net.Sockets.AddressFamily]::InterNetwork -or $HostAddress -eq '0.0.0.0') {throw 'Use the host PC IPv4 LAN or VPN address.'}
New-Item -ItemType Directory -Path $OutputDirectory -Force | Out-Null
foreach ($name in @('host.json','guest.json')) {if (Test-Path -LiteralPath (Join-Path $OutputDirectory $name)) {throw 'Configuration already exists; choose a new output directory.'}}
$rng=[Security.Cryptography.RandomNumberGenerator]::Create()
function New-Token {
    $bytes=New-Object byte[] 32
    $rng.GetBytes($bytes)
    return [Convert]::ToBase64String($bytes)
}
try {
    $session=[uint64][DateTime]::UtcNow.Ticks
    $peerToken=New-Token
    $common=@{session=$session;seed=$session;host=1;address=($HostAddress+':'+$Port)}
    $hostConfig=$common.Clone();$hostConfig.player=1;$hostConfig.token=New-Token
    $hostConfig.peers=@(@{player=2;token=$peerToken})
    $guestConfig=$common.Clone();$guestConfig.player=2;$guestConfig.token=$peerToken;$guestConfig.peers=@()
    [IO.File]::WriteAllText((Join-Path $OutputDirectory 'host.json'),($hostConfig|ConvertTo-Json -Depth 5),[Text.UTF8Encoding]::new($false))
    [IO.File]::WriteAllText((Join-Path $OutputDirectory 'guest.json'),($guestConfig|ConvertTo-Json -Depth 5),[Text.UTF8Encoding]::new($false))
} finally {$rng.Dispose()}
Write-Host 'Created host.json and guest.json. Give only guest.json to the other player. Keep both files private.'
