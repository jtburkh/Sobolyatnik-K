#requires -Version 5.1
<# Synthetic Windows fixture only. Never run against a real game or save. #>
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string] $KitPath,
    [Parameter(Mandatory = $true)][string] $LoaderSourceDirectory,
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
    $fixtureKit = Join-Path $root 'TestKit'
    New-Item -ItemType Directory -Force -Path $game, (Split-Path $save), $fixtureKit | Out-Null
    Copy-Item -Path (Join-Path $kit '*') -Destination $fixtureKit -Recurse -Force
    $runner = Join-Path $fixtureKit 'run-test-install.ps1'
    $vmz = Join-Path $fixtureKit 'RtVRadarLoot.vmz'
    $exe = Join-Path $fixtureKit 'rtv-toolkit.exe'
    $originalExe = [IO.File]::ReadAllBytes($exe)
    $archiveHash = (Get-FileHash -LiteralPath $vmz -Algorithm SHA256).Hash
    $exeHash = (Get-FileHash -LiteralPath $exe -Algorithm SHA256).Hash
    [IO.File]::WriteAllText((Join-Path $game 'RTV.exe'), 'untouched fixture binary')
    [IO.File]::WriteAllText((Join-Path $game 'RTV.pck'), 'untouched fixture archive')
    [IO.File]::WriteAllText($save, 'untouched synthetic save')
    [IO.File]::WriteAllText($manifest, '"installdir" "Road to Vostok"' + "`n" + '"buildid" "25632875"')
    Check ((Get-FileHash -LiteralPath (Join-Path $fixtureKit 'metro/modloader.gd') -Algorithm SHA256).Hash -eq (Get-FileHash -LiteralPath (Join-Path $loader 'modloader.gd') -Algorithm SHA256).Hash) 'Bundled Metro differs from verified official fixture'
    $fixtureArgs = @{ GamePath = $game; NoIntegration = (-not $TestIntegration) }
    Fails { & $runner @fixtureArgs -DryRun } 'Steam build 25710663 is required'
    Fails { & $runner @fixtureArgs } 'Steam build 25710663 is required'
    Check (-not (Test-Path -LiteralPath $radar) -and -not (Test-Path -LiteralPath $receipt)) 'Unsupported build wrote installation files'
    [IO.File]::WriteAllText($manifest, '"installdir" "Road to Vostok"' + "`n" + '"buildid" "25710663"')
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
        Check ((Test-Path -LiteralPath (Join-Path $menu 'Sobolyatnik-K Toolkit (TEST).lnk')) -and (Test-Path -LiteralPath $reg)) 'Windows integration missing'
        $entry = Get-ItemProperty -LiteralPath $reg
        Check ($entry.DisplayName -eq "Sobolyatnik-K ($($installed.Version) TEST)" -and $entry.DisplayVersion -eq $installed.Version) 'Windows entry is not visibly labeled as this test version'
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
    Check ((Get-FileHash -LiteralPath (Join-Path $game 'modloader.gd') -Algorithm SHA256).Hash -eq '60FCF7FEEC0A47C6472E3B7A190B46987B374618AE3D149C081B222542BC6135') 'Uninstall changed Metro'
    Check ((Get-FileHash -LiteralPath (Join-Path $game 'override.cfg') -Algorithm SHA256).Hash -eq '9750A66FCF0CB1D9BF84284271F52E064F455CDD5A1CDC4981A007E4011A684B') 'Uninstall changed Metro override'
    Write-Host 'HOTFIX TEST KIT OK: wrong build, missing/tampered assets, dry run, install, EXE/version, uninstall, Metro/save/binary preservation.'
} finally {
    $env:LOCALAPPDATA = $oldLocal
    $env:APPDATA = $oldApp
    if (Test-Path -LiteralPath $root) { Remove-Item -LiteralPath $root -Recurse -Force }
    if ($TestIntegration) {
        $reg = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\Sobolyatnik-K'
        if (Test-Path -LiteralPath $reg) { Remove-Item -LiteralPath $reg -Force }
    }
}
