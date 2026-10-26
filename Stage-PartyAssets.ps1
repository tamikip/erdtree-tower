param(
    [Parameter(Mandatory=$true)][string]$GameDirectory,
    [Parameter(Mandatory=$true)][string]$OutputDirectory
)
$ErrorActionPreference = 'Stop'
$workspace = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
$destination = [IO.Path]::GetFullPath($OutputDirectory)
$workRoot = [IO.Path]::GetFullPath((Join-Path $workspace 'work')) + [IO.Path]::DirectorySeparatorChar
if (-not $destination.StartsWith($workRoot, [StringComparison]::OrdinalIgnoreCase)) { throw 'Output must be a new directory under workspace/work.' }
if (Test-Path -LiteralPath $destination) { throw 'Output already exists. Choose a new staging directory.' }
$source = Join-Path $PSScriptRoot 'release-preview/scene-assets'
$project = Join-Path $PSScriptRoot 'scene-builder/SceneBuilder.csproj'
foreach ($pool in @('enemy-pools.txt', 'boss-pool.txt')) {
    $catalogHash = (Get-FileHash -LiteralPath (Join-Path $PSScriptRoot "scene-assets/$pool") -Algorithm SHA256).Hash
    $releaseHash = (Get-FileHash -LiteralPath (Join-Path $source $pool) -Algorithm SHA256).Hash
    if ($catalogHash -ne $releaseHash) { throw "Source and release catalogs differ: $pool" }
}
foreach ($required in @((Join-Path $GameDirectory 'oo2core_6_win64.dll'), (Join-Path $source 'regulation.bin'), (Join-Path $workspace 'work/scene-cache/m11_10_00_00.msb.dcx'))) {
    if (-not (Test-Path -LiteralPath $required -PathType Leaf)) { throw "Missing staging input: $required" }
}
function Invoke-AssetStage([string]$Mode, [string]$InputPath, [string]$OutputPath) {
    # Windows PowerShell 5.1 drops empty native arguments, which would shift the mode into the cache slot.
    if ([string]::IsNullOrEmpty($InputPath) -or [string]::IsNullOrEmpty($OutputPath)) { throw "Asset stage needs both paths: $Mode" }
    & dotnet run --project $project --no-restore -- $GameDirectory $OutputPath $InputPath $Mode
    if ($LASTEXITCODE -ne 0) { throw "Asset stage failed: $Mode" }
}
Push-Location $workspace
try {
    $assets = Join-Path $destination 'scene-assets'
    New-Item -ItemType Directory -Path $destination | Out-Null
    Copy-Item -LiteralPath $source -Destination $assets -Recurse
    Invoke-AssetStage 'party-enemy-audit' $assets (Join-Path $workspace 'work/party-entity-audit')
    Invoke-AssetStage 'personal-events-patch' $assets $assets
    Invoke-AssetStage 'party-enemy-stage' $assets $assets
    Invoke-AssetStage 'party-hub-stage' (Join-Path $workspace 'work/scene-cache') $assets
    Invoke-AssetStage 'party-camp-stage' $assets $assets
    Invoke-AssetStage 'party-service-stage' $assets $assets
    Invoke-AssetStage 'party-completion-patch' $assets $assets
    Invoke-AssetStage 'personal-chest-patch' $assets $assets
    Invoke-AssetStage 'party-scarab-stage' $assets $assets
    Invoke-AssetStage 'party-guard-stage' $assets $assets
    Invoke-AssetStage 'party-status-patch' $assets $assets
    Invoke-AssetStage 'party-model-patch' $assets $assets
    Invoke-AssetStage 'party-placement-audit' $assets $assets
    $parameters = Join-Path $destination 'parameters'
    Invoke-AssetStage 'party-param-stage' $assets $parameters
    Copy-Item -LiteralPath (Join-Path $parameters 'regulation.bin') -Destination (Join-Path $assets 'regulation.bin')
    Invoke-AssetStage 'party-param-audit' $assets (Join-Path $destination 'checks')
    $audit = Get-Content -LiteralPath (Join-Path $destination 'checks/party-param-audit.json') -Raw | ConvertFrom-Json
    if ($audit.Checked -le 0 -or $audit.DisableRespawn.Count -ne 0) { throw 'Repeat encounter parameter validation failed.' }
    Invoke-AssetStage 'personal-events-patch' $assets $assets
    Invoke-AssetStage 'trial-stage' $assets $assets
    $manifest = Get-ChildItem -LiteralPath $assets -File -Recurse | Sort-Object FullName | ForEach-Object {
        [ordered]@{ Path = $_.FullName.Substring($assets.Length + 1); SHA256 = (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash }
    }
    $manifest | ConvertTo-Json -Depth 3 | Set-Content -LiteralPath (Join-Path $destination 'assets.sha256.json') -Encoding UTF8
    Write-Host "Staged assets verified: $assets. Not deployed; multiplayer runtime validation is still required."
} finally { Pop-Location }
