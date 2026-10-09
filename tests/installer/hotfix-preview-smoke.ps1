#requires -Version 5.1
<# Synthetic Windows fixture only. Never run against a real game or save. #>
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string] $KitPath,
    [Parameter(Mandatory = $true)][string] $LoaderSourceDirectory,
    [string] $PreviousLoaderDirectory,
    [switch] $TestIntegration
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$kit = (Resolve-Path -LiteralPath $KitPath).ProviderPath
$loader = (Resolve-Path -LiteralPath $LoaderSourceDirectory).ProviderPath
$root = Join-Path ([IO.Path]::GetTempPath()) ('rtv-hotfix-kit-' + [guid]::NewGuid().ToString('N'))
$oldLocal = $env:LOCALAPPDATA
$oldApp = $env:APPDATA
function Check([bool] $Condition, [string] $Message) { if (-not $Condition) { throw "FAIL: $Message" } }
function Fails([scriptblock] $Block, [string] $Message) {
    $failure = ''
    try { & $Block | Out-Null } catch { $failure = $_.Exception.Message }
    Check ($failure -like "*$Message*") "Expected '$Message', got '$failure'"
}
try {
    $env:LOCALAPPDATA = Join-Path $root 'Local'
    $env:APPDATA = Join-Path $root 'Roaming'
    $steam = Join-Path $root 'Steam'
    $game = Join-Path $steam 'steamapps\common\Road to Vostok'
    $manifest = Join-Path $steam 'steamapps\appmanifest_1963610.acf'
    $save = Join-Path $env:APPDATA 'Road to Vostok\Character.tres'
    $radar = Join-Path $game 'mods\RtVRadarLoot.vmz'
    $receipt = Join-Path $env:LOCALAPPDATA 'Sobolyatnik-K\installed.json'
    $tool = Join-Path $env:LOCALAPPDATA 'Programs\Sobolyatnik-K\rtv-toolkit.exe'
    $cache = Join-Path $env:APPDATA 'Road to Vostok\vmz_mount_cache\RtVRadarLoot.zip'
    $fixtureKit = Join-Path $root 'TestKit'
    New-Item -ItemType Directory -Force -Path $game, (Split-Path $save), $fixtureKit | Out-Null
    Copy-Item -Path (Join-Path $kit '*') -Destination $fixtureKit -Recurse -Force
    $runner = Join-Path $fixtureKit 'run-sobolyatnik.ps1'
    $vmz = Join-Path $fixtureKit 'RtVRadarLoot.vmz'
    $exe = Join-Path $fixtureKit 'rtv-toolkit.exe'
    $originalExe = [IO.File]::ReadAllBytes($exe)
    $archiveHash = (Get-FileHash -LiteralPath $vmz -Algorithm SHA256).Hash
    $exeHash = (Get-FileHash -LiteralPath $exe -Algorithm SHA256).Hash
    [IO.File]::WriteAllText((Join-Path $game 'RTV.exe'), 'untouched fixture binary')
    [IO.File]::WriteAllText((Join-Path $game 'RTV.pck'), 'untouched fixture archive')
    [IO.File]::WriteAllText($save, 'untouched synthetic save')
    $manifestPrefix = '"appid" "1963610"' + "`n" + '"installdir" "Road to Vostok"' + "`n"
    Check ((Get-FileHash -LiteralPath (Join-Path $fixtureKit 'metro/modloader.gd') -Algorithm SHA256).Hash -eq (Get-FileHash -LiteralPath (Join-Path $loader 'modloader.gd') -Algorithm SHA256).Hash) 'Bundled Metro differs from verified official fixture'
    $fixtureArgs = @{ GamePath = $game; NoIntegration = (-not $TestIntegration) }
    foreach ($otherBuild in @('25632875', '25799999')) {
        [IO.File]::WriteAllText($manifest, $manifestPrefix + '"buildid" "' + $otherBuild + '"')
        $expectedWarning = 'build ' + $otherBuild + ' differs from the reviewed build 25837777'
        $warning = & $runner @fixtureArgs -DryRun 3>&1 | Out-String
        Check ($warning -match [regex]::Escape($expectedWarning)) "Steam build $otherBuild did not warn in dry run"
        Check (-not (Test-Path -LiteralPath $radar) -and -not (Test-Path -LiteralPath $receipt)) 'Mismatched-build dry run wrote installation files'
        New-Item -ItemType Directory -Force -Path (Split-Path $cache) | Out-Null
        [IO.File]::WriteAllText($cache, 'authentic-looking but stale radar cache fixture')
        $staleHash = (Get-FileHash -LiteralPath $cache -Algorithm SHA256).Hash
        $warning = & $runner @fixtureArgs 3>&1 | Out-String
        Check ($warning -match [regex]::Escape($expectedWarning)) "Steam build $otherBuild did not warn on install"
        Check (-not (Test-Path -LiteralPath $cache)) 'Installer did not remove stale radar-only cache'
        $cacheBackups = @(Get-ChildItem -LiteralPath (Join-Path $env:LOCALAPPDATA 'Sobolyatnik-K\backups') -Filter 'RtVRadarLoot-cache-*.zip' -File)
        Check (@($cacheBackups | Where-Object { (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash -eq $staleHash }).Count -gt 0) 'Stale radar cache was not backed up with verified bytes'
        Copy-Item -LiteralPath $radar -Destination $cache
        & $runner @fixtureArgs | Out-Null
        Check ((Get-FileHash -LiteralPath $cache -Algorithm SHA256).Hash -eq $archiveHash) 'Idempotent setup altered a matching radar cache'
        [IO.File]::WriteAllText($cache, 'second stale cache fixture')
        & $runner @fixtureArgs | Out-Null
        Check (-not (Test-Path -LiteralPath $cache)) 'Idempotent setup did not reconcile stale cache'
        Check ((Get-FileHash -LiteralPath $radar -Algorithm SHA256).Hash -eq $archiveHash) 'Other valid Steam build did not install the verified radar'
        Check ((Get-FileHash -LiteralPath $tool -Algorithm SHA256).Hash -eq $exeHash) 'Other valid Steam build did not install the verified Toolkit'
        & $runner @fixtureArgs -Uninstall | Out-Null
        Check (-not (Test-Path -LiteralPath $radar) -and -not (Test-Path -LiteralPath $receipt)) 'Mismatch-install uninstall failed'
    }
    [IO.File]::WriteAllText($manifest, $manifestPrefix + '"buildid" "invalid"')
    Fails { & $runner @fixtureArgs -DryRun } 'build ID missing or invalid'
    [IO.File]::WriteAllText($manifest, '"appid" "999"' + "`n" + '"buildid" "25837777"')
    Fails { & $runner @fixtureArgs -DryRun } 'not for Road to Vostok'
    Remove-Item -LiteralPath $manifest -Force
    Fails { & $runner @fixtureArgs -DryRun } 'appmanifest not found'
    [IO.File]::WriteAllText($manifest, $manifestPrefix + '"buildid" "25837777"')
    Remove-Item -LiteralPath $vmz -Force
    Fails { & $runner @fixtureArgs -DryRun } 'incomplete'
    Copy-Item -LiteralPath (Join-Path $kit 'RtVRadarLoot.vmz') -Destination $vmz -Force
    [IO.File]::WriteAllText($exe, 'tampered exe')
    Fails { & $runner @fixtureArgs -DryRun } 'SHA-256 mismatch'
    [IO.File]::WriteAllBytes($exe, $originalExe)
    & $runner @fixtureArgs -DryRun | Out-Null
    Check (-not (Test-Path -LiteralPath $radar) -and -not (Test-Path -LiteralPath $receipt)) 'Dry run wrote installation files'
    & $runner @fixtureArgs | Out-Null
    Check ((Get-FileHash -LiteralPath $radar -Algorithm SHA256).Hash -eq $archiveHash) 'Installed wrong radar'
    Check ((Get-FileHash -LiteralPath $tool -Algorithm SHA256).Hash -eq $exeHash) 'Installed wrong Toolkit'
    $installed = [IO.File]::ReadAllText($receipt) | ConvertFrom-Json
    $helpText = & $tool --help | Out-String
    Check ($LASTEXITCODE -eq 0 -and $helpText -match ('rtv-toolkit ' + [regex]::Escape([string]$installed.Version))) 'Toolkit and installer versions differ'
    $menu = Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu\Programs\Sobolyatnik-K'
    $reg = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\Sobolyatnik-K'
    if ($TestIntegration) {
        Check ((Test-Path -LiteralPath (Join-Path $menu 'Sobolyatnik-K Toolkit.lnk')) -and (Test-Path -LiteralPath $reg)) 'Windows integration missing'
        $entry = Get-ItemProperty -LiteralPath $reg
        Check ($entry.DisplayName -eq 'Sobolyatnik-K' -and $entry.DisplayVersion -eq $installed.Version) 'Windows entry is not visibly labeled as this test version'
    }
    [IO.File]::WriteAllText($tool, 'tampered after install')
    Fails { & $runner @fixtureArgs -Uninstall } 'differs from the installed release'
    Check (Test-Path -LiteralPath $radar) 'Tamper caused partial uninstall'
    [IO.File]::WriteAllBytes($tool, $originalExe)
    & $runner @fixtureArgs -Uninstall | Out-Null
    Check (-not (Test-Path -LiteralPath $radar) -and -not (Test-Path -LiteralPath $tool) -and -not (Test-Path -LiteralPath $receipt)) 'Uninstall incomplete'
    if ($TestIntegration) { Check (-not (Test-Path -LiteralPath $reg)) 'Windows integration left behind' }
    Check ([IO.File]::ReadAllText($save) -eq 'untouched synthetic save') 'Save was modified'
    Check ([IO.File]::ReadAllText((Join-Path $game 'RTV.exe')) -eq 'untouched fixture binary') 'Game binary was modified'
    Check ([IO.File]::ReadAllText((Join-Path $game 'RTV.pck')) -eq 'untouched fixture archive') 'Game archive was modified'
    Check ((Get-FileHash -LiteralPath (Join-Path $game 'modloader.gd') -Algorithm SHA256).Hash -eq (Get-FileHash -LiteralPath (Join-Path $loader 'modloader.gd') -Algorithm SHA256).Hash) 'Uninstall changed Metro'
    Check ((Get-FileHash -LiteralPath (Join-Path $game 'override.cfg') -Algorithm SHA256).Hash -eq '9750A66FCF0CB1D9BF84284271F52E064F455CDD5A1CDC4981A007E4011A684B') 'Uninstall changed Metro override'
    if ($PreviousLoaderDirectory) {
        $previousLoader = Join-Path (Resolve-Path -LiteralPath $PreviousLoaderDirectory).ProviderPath 'modloader.gd'
        $knownHash = '60FCF7FEEC0A47C6472E3B7A190B46987B374618AE3D149C081B222542BC6135'
        Check ((Get-FileHash -LiteralPath $previousLoader -Algorithm SHA256).Hash -eq $knownHash) 'Previous official Metro fixture hash differs'
        $installedLoader = Join-Path $game 'modloader.gd' # The game here is a disposable synthetic directory.
        [IO.File]::WriteAllText($installedLoader, 'const MODLOADER_VERSION := "3.2.1" # unverified script')
        Fails { & $runner @fixtureArgs -DryRun } 'neither the verified 3.2.1 nor 3.4.2'
        Copy-Item -LiteralPath $previousLoader -Destination $installedLoader -Force
        & $runner @fixtureArgs -DryRun | Out-Null
        & $runner @fixtureArgs | Out-Null
        Check ((Get-FileHash -LiteralPath $installedLoader -Algorithm SHA256).Hash -eq $knownHash) 'Installing radar over Metro 3.2.1 changed the loader'
        Check ((Get-FileHash -LiteralPath $radar -Algorithm SHA256).Hash -eq $archiveHash) 'Radar install on Metro 3.2.1 failed'
        & $runner @fixtureArgs -Uninstall | Out-Null
        Check ((Get-FileHash -LiteralPath $installedLoader -Algorithm SHA256).Hash -eq $knownHash) 'Uninstall changed retained Metro 3.2.1'
    }
    Write-Host 'WINDOWS BUNDLE OK: valid other-build warning/install, malformed/missing manifest refusal, dry run, hashes, install, uninstall, Metro/save/binary preservation.'
} finally {
    $env:LOCALAPPDATA = $oldLocal
    $env:APPDATA = $oldApp
    if (Test-Path -LiteralPath $root) { Remove-Item -LiteralPath $root -Recurse -Force }
    if ($TestIntegration) {
        $reg = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\Sobolyatnik-K'
        if (Test-Path -LiteralPath $reg) { Remove-Item -LiteralPath $reg -Force }
    }
}
