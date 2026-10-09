#requires -Version 5.1
param(
    [Parameter(Mandatory = $true)][string] $ArchivePath,
    [Parameter(Mandatory = $true)][string] $LoaderSourceDirectory,
    [string] $FakeToolkitPath,
    [string] $RenderedInstaller,
    [string] $RadarInstallerPath,
    [string] $UninstallerPath,
    [string] $InstallerTemplate,
    [string] $PreviousBundleInstallerPath,
    [string] $PreviousBundleToolkitPath,
    [string] $PreviousBundleUninstallerPath,
    [string] $PreviousBundleRadarInstallerPath,
    [string] $PreviousBundleArchivePath,
    [string] $PreviousBundleVersion = '0.1.9-experimental',
    [string] $PreviousBundleGameBuild,
    [string] $ExpectedBundleVersion = '0.1.11-experimental',
    [string] $ExpectedGameBuild = '25632875',
    [string] $ExpectedToolkitVersion,
    [string] $ExpectedRadarHash = '66FD97A4C1487BE6688EF94B59EE709A3BAA8C7886C490D2F03AF7B8C395EEFC',
    [switch] $TestIntegration,
    [switch] $CheckToolkitHelp
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).ProviderPath
$archive = (Resolve-Path -LiteralPath $ArchivePath).ProviderPath
$loader = (Resolve-Path -LiteralPath $LoaderSourceDirectory).ProviderPath
$legacy = if ($RadarInstallerPath) { (Resolve-Path -LiteralPath $RadarInstallerPath).ProviderPath }
    else { (Resolve-Path -LiteralPath (Join-Path $repo 'tools\install-sobolyatnik.ps1')).ProviderPath }
$uninstaller = if ($UninstallerPath) { (Resolve-Path -LiteralPath $UninstallerPath).ProviderPath }
    else { (Resolve-Path -LiteralPath (Join-Path $repo 'tools\uninstall-sobolyatnik.ps1')).ProviderPath }
$template = if ($InstallerTemplate) { (Resolve-Path -LiteralPath $InstallerTemplate).ProviderPath }
    else { (Resolve-Path -LiteralPath (Join-Path $repo 'tools\setup-sobolyatnik.ps1.in')).ProviderPath }
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
    [IO.File]::WriteAllText((Join-Path $gameApps 'appmanifest_1963610.acf'), '"appid" "1963610"' + "`n" + '"installdir" "Road to Vostok"' + "`n" + '"buildid" "' + $ExpectedGameBuild + '"')
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
        & python (Join-Path $repo 'tools\render_bundle_installer.py') --toolkit $fakeExe --output $rendered --template $template --uninstaller $uninstaller | Out-Null
        if ($LASTEXITCODE -ne 0) { throw 'Unable to render offline fixture installer' }
    } else { throw 'Pass both -FakeToolkitPath and -RenderedInstaller, or neither.' }
    $args = @{
        GamePath = $game; ArchivePath = $archive; LoaderSourceDirectory = $loader
        ToolkitPath = $fakeExe; RadarInstallerPath = $legacy
        UninstallerPath = $uninstaller
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
        if ($ExpectedToolkitVersion) {
            Check ($helpText -match ('rtv-toolkit ' + [regex]::Escape($ExpectedToolkitVersion))) 'Toolkit does not report the test kit version'
        }
    }
    if ($TestIntegration) {
        $menu = Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu\Programs\Sobolyatnik-K'
        $reg = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\Sobolyatnik-K'
        Check ((Test-Path -LiteralPath (Join-Path $menu 'Sobolyatnik-K Toolkit.lnk')) -and
               (Test-Path -LiteralPath (Join-Path $menu 'Uninstall Sobolyatnik-K.lnk')) -and
               (Test-Path -LiteralPath $reg)) 'Start Menu/uninstall registration missing'
    }
    Check ((Get-FileHash $radar -Algorithm SHA256).Hash -eq $ExpectedRadarHash) 'Radar hash changed'
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
    Check ((Get-FileHash (Join-Path $game 'modloader.gd') -Algorithm SHA256).Hash -eq (Get-FileHash (Join-Path $loader 'modloader.gd') -Algorithm SHA256).Hash) 'Uninstall changed Metro'
    Check ([IO.File]::ReadAllText((Join-Path $game 'RTV.exe')) -eq 'fixture game binary') 'Game binary changed'
    # Upgrade someone who already installed v0.1.8 on another computer. Metro's
    # override.cfg is rewritten by its two-pass startup; leave those bytes alone.
    & $legacy -GamePath $game -ArchivePath $archive | Out-Null
    $override = Join-Path $game 'override.cfg'
    [IO.File]::WriteAllText($override, '[autoload_prepend]' + "`n" + 'ModLoader="*res://modloader.gd"' + "`n" + '[autoload]')
    $overrideHash = (Get-FileHash $override -Algorithm SHA256).Hash
    & $rendered @args | Out-Null
    Check ((Get-FileHash $override -Algorithm SHA256).Hash -eq $overrideHash) 'Upgrading from v0.1.8 overwrote Metro state'
    Check ((Get-FileHash $radar -Algorithm SHA256).Hash -eq $ExpectedRadarHash) 'Upgrade replaced the tested VMZ'
    & $rendered -Uninstall -NoIntegration:(-not $TestIntegration) | Out-Null
    Check ((Get-FileHash $override -Algorithm SHA256).Hash -eq $overrideHash) 'Uninstall overwrote Metro state'
    Fails { & $rendered -GamePath $game -ArchivePath $archive -ToolkitPath (Join-Path $root 'missing.exe') -UninstallerPath $uninstaller -RadarInstallerPath $legacy -LoaderSourceDirectory $loader -NoIntegration } 'Cannot find path'
    Check (-not (Test-Path -LiteralPath $radar)) 'Missing Toolkit source unexpectedly installed radar'
    if ($PreviousBundleInstallerPath -or $PreviousBundleToolkitPath -or $PreviousBundleUninstallerPath) {
        Check ([bool]($PreviousBundleInstallerPath -and $PreviousBundleToolkitPath -and $PreviousBundleUninstallerPath)) 'Pass all three previous bundle assets together'
        $oldBundle = (Resolve-Path -LiteralPath $PreviousBundleInstallerPath).ProviderPath
        $oldExe = (Resolve-Path -LiteralPath $PreviousBundleToolkitPath).ProviderPath
        $oldUninstaller = (Resolve-Path -LiteralPath $PreviousBundleUninstallerPath).ProviderPath
        $oldRadarInstaller = if ($PreviousBundleRadarInstallerPath) { (Resolve-Path -LiteralPath $PreviousBundleRadarInstallerPath).ProviderPath } else { $legacy }
        $oldArchive = if ($PreviousBundleArchivePath) { (Resolve-Path -LiteralPath $PreviousBundleArchivePath).ProviderPath } else { $archive }
        $saveDir = Join-Path $env:APPDATA 'Road to Vostok'
        New-Item -ItemType Directory -Force -Path $saveDir | Out-Null
        $save = Join-Path $saveDir 'Character.tres'
        [IO.File]::WriteAllText($save, 'fixture save: keep')
        $oldArgs = @{
            GamePath = $game; ArchivePath = $oldArchive; LoaderSourceDirectory = $loader
            ToolkitPath = $oldExe; RadarInstallerPath = $oldRadarInstaller; UninstallerPath = $oldUninstaller
        }
        if (-not $TestIntegration) { $oldArgs.NoIntegration = $true }
        if ($PreviousBundleGameBuild) {
            [IO.File]::WriteAllText((Join-Path $gameApps 'appmanifest_1963610.acf'), '"appid" "1963610"' + "`n" + '"installdir" "Road to Vostok"' + "`n" + '"buildid" "' + $PreviousBundleGameBuild + '"')
        }
        & $oldBundle @oldArgs | Out-Null
        if ($PreviousBundleGameBuild) {
            [IO.File]::WriteAllText((Join-Path $gameApps 'appmanifest_1963610.acf'), '"appid" "1963610"' + "`n" + '"installdir" "Road to Vostok"' + "`n" + '"buildid" "' + $ExpectedGameBuild + '"')
        }
        Check (([IO.File]::ReadAllText($receipt) | ConvertFrom-Json).Version -eq $PreviousBundleVersion) 'Previous bundle receipt missing'
        $previousHash = (Get-FileHash $installedExe -Algorithm SHA256).Hash
        $bundleArgs = $args # $args is automatic inside scriptblocks; capture with a different name.
        Fails { & $rendered @bundleArgs } ("v$(($PreviousBundleVersion -split '-')[0]) is installed")
        Check ((Get-FileHash $installedExe -Algorithm SHA256).Hash -eq $previousHash) 'Collision overwrote v0.1.9 Toolkit'
        Check (([IO.File]::ReadAllText($receipt) | ConvertFrom-Json).Version -eq $PreviousBundleVersion) 'Collision rewrote previous receipt'
        Check ((Get-FileHash $radar -Algorithm SHA256).Hash -eq (Get-FileHash $oldArchive -Algorithm SHA256).Hash) 'Collision changed radar'
        & $oldBundle -Uninstall -NoIntegration:(-not $TestIntegration) | Out-Null
        Check (-not (Test-Path -LiteralPath $radar) -and -not (Test-Path -LiteralPath $receipt)) 'v0.1.9 uninstall incomplete'
        Check ([IO.File]::ReadAllText($save) -eq 'fixture save: keep') 'Previous uninstall changed save'
        & $rendered @args | Out-Null
        Check (([IO.File]::ReadAllText($receipt) | ConvertFrom-Json).Version -eq $ExpectedBundleVersion) 'New version receipt missing'
        if ($CheckToolkitHelp) {
            $helpText = & $installedExe --help | Out-String
            Check ($LASTEXITCODE -eq 0 -and $helpText -match 'rtv-toolkit --check') 'Upgraded Toolkit does not launch'
        }
        & $rendered -Uninstall -NoIntegration:(-not $TestIntegration) | Out-Null
        Check ((Get-FileHash $override -Algorithm SHA256).Hash -eq $overrideHash) 'Upgrade flow changed Metro'
        Check ([IO.File]::ReadAllText($save) -eq 'fixture save: keep') 'Upgrade flow changed save'
        Check ((Test-Path -LiteralPath (Join-Path $game 'RTV.pck')) -and (Test-Path -LiteralPath (Join-Path $game 'RTV.exe'))) 'Upgrade flow changed game binaries'
    }
    Write-Host "BUNDLE SMOKE OK: clean, radar-only install, optional $PreviousBundleVersion uninstall/reinstall, dry run, hashes, receipt, idempotence, tamper/running-game refusal, Metro and save retention."
} catch {
    # Surface the actual fixture error as a public CI annotation; generic job
    # failures are otherwise difficult to diagnose without Actions log access.
    Write-Host "::error title=Windows bundle fixture::$($_.Exception.Message)"
    throw
} finally {
    $env:LOCALAPPDATA = $oldLocal
    $env:APPDATA = $oldApp
    if (Test-Path -LiteralPath $root) { Remove-Item -LiteralPath $root -Recurse -Force }
    if ($TestIntegration) {
        $testReg = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\Sobolyatnik-K'
        if (Test-Path -LiteralPath $testReg) { Remove-Item -LiteralPath $testReg -Force }
    }
}
