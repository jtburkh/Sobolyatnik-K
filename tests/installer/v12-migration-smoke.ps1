#requires -Version 5.1
<# Synthetic Windows fixture ONLY. The game path and save are created under %TEMP%.
   Start with the exact public 0.1.16 bundle, simulate an owner Metro upgrade
   while the fake game is closed, then verify safe uninstall -> v1.2 install. #>
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string] $PreviousKitPath,
    [Parameter(Mandatory = $true)][string] $NewKitPath
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$previous = (Resolve-Path -LiteralPath $PreviousKitPath).ProviderPath
$next = (Resolve-Path -LiteralPath $NewKitPath).ProviderPath
$root = Join-Path ([IO.Path]::GetTempPath()) ('rtv-v12-migration-' + [guid]::NewGuid().ToString('N'))
$oldLocal = $env:LOCALAPPDATA
$oldApp = $env:APPDATA
function Check([bool] $Condition, [string] $Message) { if (-not $Condition) { throw "FAIL: $Message" } }
try {
    $env:LOCALAPPDATA = Join-Path $root 'Local'
    $env:APPDATA = Join-Path $root 'Roaming'
    $steam = Join-Path $root 'Steam'
    $game = Join-Path $steam 'steamapps\common\Road to Vostok'
    $manifest = Join-Path $steam 'steamapps\appmanifest_1963610.acf'
    $save = Join-Path $env:APPDATA 'Road to Vostok\Character.tres'
    $cache = Join-Path $env:APPDATA 'Road to Vostok\vmz_mount_cache\RtVRadarLoot.zip'
    $radar = Join-Path $game 'mods\RtVRadarLoot.vmz'
    $metro = Join-Path $game 'modloader.gd'
    $toolkit = Join-Path $env:LOCALAPPDATA 'Programs\Sobolyatnik-K\rtv-toolkit.exe'
    $receipt = Join-Path $env:LOCALAPPDATA 'Sobolyatnik-K\installed.json'
    New-Item -ItemType Directory -Force -Path $game, (Split-Path $save), (Split-Path $cache) | Out-Null
    [IO.File]::WriteAllText((Join-Path $game 'RTV.exe'), 'synthetic game binary only')
    [IO.File]::WriteAllText((Join-Path $game 'RTV.pck'), 'synthetic game pack only')
    [IO.File]::WriteAllText($save, 'synthetic character save only')
    $prefix = '"appid" "1963610"' + "`n" + '"installdir" "Road to Vostok"' + "`n"
    [IO.File]::WriteAllText($manifest, $prefix + '"buildid" "25710663"')
    $previousRunner = Join-Path $previous 'run-sobolyatnik.ps1'
    $newRunner = Join-Path $next 'run-sobolyatnik.ps1'
    $oldHash = '276844B086EB27F559B71AA74743F49E6EE045C639A7203B1B1CE02CA98BE1A4'
    $newHash = (Get-FileHash -LiteralPath (Join-Path $next 'RtVRadarLoot.vmz') -Algorithm SHA256).Hash
    $oldMetroHash = '60FCF7FEEC0A47C6472E3B7A190B46987B374618AE3D149C081B222542BC6135'
    $newMetroHash = '30C39E3846957F43925E49EF2449C2CA8F6940C77446AF169CFAFC858975BCC8'
    Check ((Get-FileHash -LiteralPath (Join-Path $previous 'RtVRadarLoot.vmz') -Algorithm SHA256).Hash -eq $oldHash) 'Previous public VMZ changed'
    Check ((Get-FileHash -LiteralPath (Join-Path $next 'metro\modloader.gd') -Algorithm SHA256).Hash -eq $newMetroHash) 'New Metro asset changed'
    & $previousRunner -GamePath $game -NoIntegration | Out-Null
    Check ((Get-FileHash -LiteralPath $radar -Algorithm SHA256).Hash -eq $oldHash) 'Previous bundle failed to install'
    Check ((Get-FileHash -LiteralPath $metro -Algorithm SHA256).Hash -eq $oldMetroHash) 'Previous bundle used wrong Metro'
    Check (([IO.File]::ReadAllText($receipt) | ConvertFrom-Json).Version -eq '0.1.16-experimental.1') 'Previous receipt missing'
    $oldToolkitHash = (Get-FileHash -LiteralPath $toolkit -Algorithm SHA256).Hash
    # Only the synthetic game directory is modified here; the real game is never referenced.
    Copy-Item -LiteralPath (Join-Path $next 'metro\modloader.gd') -Destination $metro -Force
    [IO.File]::WriteAllText($manifest, $prefix + '"buildid" "25837777"')
    Copy-Item -LiteralPath $radar -Destination $cache
    $collision = ''
    try { & $newRunner -GamePath $game -NoIntegration | Out-Null } catch { $collision = $_.Exception.Message }
    Check ($collision -like '*v0.1.16 is installed*') "New setup must refuse the old receipt; got '$collision'"
    Check ((Get-FileHash -LiteralPath $radar -Algorithm SHA256).Hash -eq $oldHash) 'Collision changed existing VMZ'
    Check ((Get-FileHash -LiteralPath $toolkit -Algorithm SHA256).Hash -eq $oldToolkitHash) 'Collision changed existing Toolkit'
    & $previousRunner -NoIntegration -Uninstall | Out-Null
    Check (-not (Test-Path -LiteralPath $receipt) -and -not (Test-Path -LiteralPath $radar)) 'Old uninstall did not remove owned files'
    Check ((Get-FileHash -LiteralPath $metro -Algorithm SHA256).Hash -eq $newMetroHash) 'Old uninstall changed owner-updated Metro'
    & $newRunner -GamePath $game -NoIntegration | Out-Null
    Check ((Get-FileHash -LiteralPath $radar -Algorithm SHA256).Hash -eq $newHash) 'v1.2 VMZ did not install'
    Check (([IO.File]::ReadAllText($receipt) | ConvertFrom-Json).Version -eq '1.2.0') 'v1.2 receipt missing'
    Check (-not (Test-Path -LiteralPath $cache)) 'Stale radar-only cache was not reconciled'
    $backups = @(Get-ChildItem -LiteralPath (Join-Path $env:LOCALAPPDATA 'Sobolyatnik-K\backups') -Filter 'RtVRadarLoot-cache-*.zip' -File)
    Check (@($backups | Where-Object { (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash -eq $oldHash }).Count -gt 0) 'Old cache backup was not verified'
    Check ((Get-FileHash -LiteralPath $metro -Algorithm SHA256).Hash -eq $newMetroHash) 'v1.2 overwrote existing Metro'
    $helpText = & $toolkit --help | Out-String
    Check ($LASTEXITCODE -eq 0 -and $helpText -match 'rtv-toolkit 1\.2\.0') 'v1.2 Toolkit EXE does not match'
    & $newRunner -NoIntegration -Uninstall | Out-Null
    Check (-not (Test-Path -LiteralPath $receipt) -and -not (Test-Path -LiteralPath $radar) -and -not (Test-Path -LiteralPath $toolkit)) 'v1.2 uninstall left owned files'
    Check ((Get-FileHash -LiteralPath $metro -Algorithm SHA256).Hash -eq $newMetroHash) 'v1.2 uninstall changed Metro'
    Check ([IO.File]::ReadAllText($save) -eq 'synthetic character save only') 'Save changed'
    Check ([IO.File]::ReadAllText((Join-Path $game 'RTV.exe')) -eq 'synthetic game binary only') 'Game EXE changed'
    Check ([IO.File]::ReadAllText((Join-Path $game 'RTV.pck')) -eq 'synthetic game pack only') 'Game PCK changed'
    Write-Host 'V1.2 MIGRATION OK: public v0.1.16 -> retained owner-updated Metro 3.4.2 -> safe uninstall -> v1.2 install/cache backup/uninstall, game/save unchanged.'
} finally {
    $env:LOCALAPPDATA = $oldLocal
    $env:APPDATA = $oldApp
    if (Test-Path -LiteralPath $root) { Remove-Item -LiteralPath $root -Recurse -Force }
}
