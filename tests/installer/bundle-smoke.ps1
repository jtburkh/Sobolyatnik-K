#requires -Version 5.1
param(
    [Parameter(Mandatory = $true)][string] $ArchivePath,
    [Parameter(Mandatory = $true)][string] $LoaderSourceDirectory,
    [string] $FakeToolkitPath,
    [string] $RenderedInstaller,
    [switch] $TestIntegration,
    [switch] $CheckToolkitHelp
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).ProviderPath
$archive = (Resolve-Path -LiteralPath $ArchivePath).ProviderPath
$loader = (Resolve-Path -LiteralPath $LoaderSourceDirectory).ProviderPath
$root = Join-Path ([IO.Path]::GetTempPath()) ('sobolyatnik-bundle-test-' + [guid]::NewGuid().ToString('N'))
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
    $gameApps = Join-Path $steam 'steamapps'
    $radar = Join-Path $game 'mods\RtVRadarLoot.vmz'
    New-Item -ItemType Directory -Force -Path $game, $env:LOCALAPPDATA, $env:APPDATA | Out-Null
    [IO.File]::WriteAllText((Join-Path $game 'RTV.exe'), 'fixture game binary')
    [IO.File]::WriteAllText((Join-Path $game 'RTV.pck'), 'fixture game archive')
    [IO.File]::WriteAllText((Join-Path $gameApps 'appmanifest_1963610.acf'), '"installdir" "Road to Vostok"' + "`n" + '"buildid" "25632875"')
    # A small synthetic PE header; only disposable file handling is tested here.
    # The real compiled Toolkit is built, executed and archived on Windows CI.
    $fakeExe = Join-Path $root 'rtv-toolkit.exe'
    $rendered = Join-Path $root 'setup-sobolyatnik.ps1'
    if ($FakeToolkitPath -and $RenderedInstaller) {
        Copy-Item -LiteralPath $FakeToolkitPath -Destination $fakeExe
        Copy-Item -LiteralPath $RenderedInstaller -Destination $rendered
    } elseif (-not $FakeToolkitPath -and -not $RenderedInstaller) {
        $bytes = New-Object byte[] 1024
        $bytes[0] = 0x4d; $bytes[1] = 0x5a; $bytes[0x3c] = 0x80
        $bytes[0x80] = 0x50; $bytes[0x81] = 0x45
        $bytes[0x84] = 0x64; $bytes[0x85] = 0x86
        [IO.File]::WriteAllBytes($fakeExe, $bytes)
        & python (Join-Path $repo 'tools\render_bundle_installer.py') --toolkit $fakeExe --output $rendered | Out-Null
        if ($LASTEXITCODE -ne 0) { throw 'Unable to render offline fixture installer' }
    } else { throw 'Pass both -FakeToolkitPath and -RenderedInstaller, or neither.' }
    $args = @{
        GamePath = $game; ArchivePath = $archive; LoaderSourceDirectory = $loader
        ToolkitPath = $fakeExe; RadarInstallerPath = (Join-Path $repo 'tools\install-sobolyatnik.ps1')
        UninstallerPath = (Join-Path $repo 'tools\uninstall-sobolyatnik.ps1')
    }
    if (-not $TestIntegration) { $args.NoIntegration = $true }
    & $rendered @args -DryRun | Out-Null
    Check (-not (Test-Path -LiteralPath $radar)) 'Dry run changed game files'
    & $rendered @args | Out-Null
    $installedExe = Join-Path $env:LOCALAPPDATA 'Programs\Sobolyatnik-K\rtv-toolkit.exe'
    $receipt = Join-Path $env:LOCALAPPDATA 'Sobolyatnik-K\installed.json'
    Check ((Get-FileHash $installedExe -Algorithm SHA256).Hash -eq (Get-FileHash $fakeExe -Algorithm SHA256).Hash) 'Toolkit binary differs from verified input'
    if ($CheckToolkitHelp) {
        $helpText = & $installedExe --help | Out-String
        Check ($LASTEXITCODE -eq 0 -and $helpText -match 'rtv-toolkit --check') 'Installed Toolkit does not run'
    }
    if ($TestIntegration) {
        $menu = Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu\Programs\Sobolyatnik-K'
        $reg = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\Sobolyatnik-K'
        Check ((Test-Path -LiteralPath (Join-Path $menu 'Sobolyatnik-K Toolkit.lnk')) -and
               (Test-Path -LiteralPath (Join-Path $menu 'Uninstall Sobolyatnik-K.lnk')) -and
               (Test-Path -LiteralPath $reg)) 'Start Menu/uninstall registration missing'
    }
    Check ((Get-FileHash $radar -Algorithm SHA256).Hash -eq '66FD97A4C1487BE6688EF94B59EE709A3BAA8C7886C490D2F03AF7B8C395EEFC') 'Radar hash changed'
    Check (Test-Path -LiteralPath $receipt -PathType Leaf) 'No ownership receipt written'
    & $rendered @args | Out-Null
    Check (@(Get-ChildItem -LiteralPath (Join-Path $game 'mods') -File).Count -eq 1) 'Idempotent install left temp files'
    # A tampered exe must block uninstall without removing the radar.
    [IO.File]::WriteAllText($installedExe, 'tampered')
    Fails { & $rendered -Uninstall -NoIntegration:(-not $TestIntegration) } 'differs from the installed release'
    Check (Test-Path -LiteralPath $radar) 'Tampered Toolkit caused partial uninstall'
    Copy-Item -LiteralPath $fakeExe -Destination $installedExe -Force
    function Get-Process { param($Name, $ErrorAction) return [pscustomobject]@{Name = 'RTV'} }
    Fails { & $rendered -Uninstall -NoIntegration:(-not $TestIntegration) } 'Close Road to Vostok'
    Remove-Item Function:\Get-Process -Force
    & $rendered -Uninstall -NoIntegration:(-not $TestIntegration) | Out-Null
    Check (-not (Test-Path -LiteralPath $radar) -and -not (Test-Path -LiteralPath $installedExe) -and
           -not (Test-Path -LiteralPath $receipt)) 'Uninstall left radar, Toolkit, or receipt'
    if ($TestIntegration) {
        Check (-not (Test-Path -LiteralPath (Join-Path $menu 'Sobolyatnik-K Toolkit.lnk')) -and
               -not (Test-Path -LiteralPath $reg)) 'Uninstall left Start Menu or Windows entry'
    }
    Check ((Get-FileHash (Join-Path $game 'modloader.gd') -Algorithm SHA256).Hash -eq '60FCF7FEEC0A47C6472E3B7A190B46987B374618AE3D149C081B222542BC6135') 'Uninstall changed Metro'
    Check ([IO.File]::ReadAllText((Join-Path $game 'RTV.exe')) -eq 'fixture game binary') 'Game binary changed'
    # Upgrade someone who already installed v0.1.8 on another computer. Metro's
    # override.cfg is rewritten by its two-pass startup; leave those bytes alone.
    & (Join-Path $repo 'tools\install-sobolyatnik.ps1') -GamePath $game -ArchivePath $archive | Out-Null
    $override = Join-Path $game 'override.cfg'
    [IO.File]::WriteAllText($override, '[autoload_prepend]' + "`n" + 'ModLoader="*res://modloader.gd"' + "`n" + '[autoload]')
    $overrideHash = (Get-FileHash $override -Algorithm SHA256).Hash
    & $rendered @args | Out-Null
    Check ((Get-FileHash $override -Algorithm SHA256).Hash -eq $overrideHash) 'Upgrading from v0.1.8 overwrote Metro state'
    Check ((Get-FileHash $radar -Algorithm SHA256).Hash -eq '66FD97A4C1487BE6688EF94B59EE709A3BAA8C7886C490D2F03AF7B8C395EEFC') 'Upgrade replaced the tested VMZ'
    & $rendered -Uninstall -NoIntegration:(-not $TestIntegration) | Out-Null
    Check ((Get-FileHash $override -Algorithm SHA256).Hash -eq $overrideHash) 'Uninstall overwrote Metro state'
    Fails { & $rendered -GamePath $game -ArchivePath $archive -ToolkitPath (Join-Path $root 'missing.exe') -UninstallerPath (Join-Path $repo 'tools\uninstall-sobolyatnik.ps1') -RadarInstallerPath (Join-Path $repo 'tools\install-sobolyatnik.ps1') -NoIntegration } 'Cannot find path'
    Check (-not (Test-Path -LiteralPath $radar)) 'Missing Toolkit source unexpectedly installed radar'
    Write-Host 'BUNDLE SMOKE OK: clean and v0.1.8 upgrades, dry run, hashes, receipt, idempotence, tamper refusal, running-game guard, uninstall retaining Metro.'
} finally {
    $env:LOCALAPPDATA = $oldLocal
    $env:APPDATA = $oldApp
    if (Test-Path -LiteralPath $root) { Remove-Item -LiteralPath $root -Recurse -Force }
    if ($TestIntegration) {
        $testReg = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\Sobolyatnik-K'
        if (Test-Path -LiteralPath $testReg) { Remove-Item -LiteralPath $testReg -Force }
    }
}
