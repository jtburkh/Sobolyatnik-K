#requires -Version 5.1
<#
Owner-controlled @KIT_VERSION@ TEST prerelease kit. Keep every asset
in this extracted folder; do not use the pinned v0.1.12 setup with these files.
The verified setup owns all safety checks and will not overwrite a foreign
receipt, loader, VMZ, executable, save or game binary.
#>
[CmdletBinding()]
param(
    [string] $GamePath,
    [string] $SteamRoot,
    [string] $LoaderSourceDirectory,
    [switch] $DryRun,
    [switch] $NoIntegration,
    [switch] $Uninstall
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$setup = Join-Path $PSScriptRoot 'setup-sobolyatnik.ps1'
$radarInstaller = Join-Path $PSScriptRoot 'install-sobolyatnik.ps1'
$uninstaller = Join-Path $PSScriptRoot 'uninstall-sobolyatnik.ps1'
$vmz = Join-Path $PSScriptRoot 'RtVRadarLoot.vmz'
$toolkit = Join-Path $PSScriptRoot 'rtv-toolkit.exe'
$bundledMetro = Join-Path $PSScriptRoot 'metro'
if (-not $LoaderSourceDirectory) { $LoaderSourceDirectory = $bundledMetro }
foreach ($asset in @($setup, $radarInstaller, $uninstaller, $vmz, $toolkit,
                   (Join-Path $LoaderSourceDirectory 'modloader.gd'),
                   (Join-Path $LoaderSourceDirectory 'override.cfg'))) {
    if (-not (Test-Path -LiteralPath $asset -PathType Leaf)) {
        throw "The extracted test kit is incomplete: $asset. Nothing was installed."
    }
}
if ($Uninstall) {
    if ($DryRun) { throw 'Uninstall cannot be combined with DryRun.' }
    & $setup -Uninstall -NoIntegration:$NoIntegration
    return
}
$parameters = @{
    ArchivePath = $vmz
    ToolkitPath = $toolkit
    RadarInstallerPath = $radarInstaller
    UninstallerPath = $uninstaller
    DryRun = [bool]$DryRun
    NoIntegration = [bool]$NoIntegration
}
if ($GamePath) { $parameters.GamePath = $GamePath }
if ($SteamRoot) { $parameters.SteamRoot = $SteamRoot }
$parameters.LoaderSourceDirectory = $LoaderSourceDirectory
& $setup @parameters
