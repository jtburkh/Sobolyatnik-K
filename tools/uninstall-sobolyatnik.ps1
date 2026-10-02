#requires -Version 5.1
<#
.SYNOPSIS
    Remove the hash-verified Sobolyatnik-K radar and Toolkit from this user account.
.DESCRIPTION
    Does not remove Metro Mod Loader, other mods, saves, or rollback backups.
    Refuses changed installation files; run with Road to Vostok closed.
#>
[CmdletBinding()]
param([switch] $NoIntegration)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$version = '0.1.9-experimental'
$radarHash = '66fd97a4c1487be6688ef94b59ee709a3baa8c7886c490d2f03af7b8c395eefc'
$stateDir = Join-Path $env:LOCALAPPDATA 'Sobolyatnik-K'
$receiptPath = Join-Path $stateDir 'installed.json'
$toolDir = Join-Path $env:LOCALAPPDATA 'Programs\Sobolyatnik-K'
$toolExe = Join-Path $toolDir 'rtv-toolkit.exe'
$installedUninstaller = Join-Path $toolDir 'uninstall-sobolyatnik.ps1'
$shortcuts = Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu\Programs\Sobolyatnik-K'
$registry = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\Sobolyatnik-K'

if (Get-Process -Name RTV -ErrorAction SilentlyContinue) { throw 'Close Road to Vostok before uninstalling.' }
if (-not (Test-Path -LiteralPath $receiptPath -PathType Leaf)) {
    throw "No Sobolyatnik-K bundle installation receipt found at $receiptPath. The v0.1.8 radar-only installer did not create a receipt; close the game and move RtVRadarLoot.vmz out of mods manually."
}
$receipt = [IO.File]::ReadAllText($receiptPath) | ConvertFrom-Json
if ($receipt.Version -ne $version -or $receipt.RadarSha256 -ine $radarHash -or
    [string]$receipt.ToolkitSha256 -notmatch '^[a-fA-F0-9]{64}$') {
    throw 'Installation receipt is not from this bundle; no files were changed.'
}
$game = [IO.Path]::GetFullPath([string]$receipt.GamePath).TrimEnd('\')
if ($game -notmatch '(?i)\\steamapps\\common\\[^\\]+$' -or
    -not (Test-Path -LiteralPath (Join-Path $game 'RTV.exe') -PathType Leaf) -or
    -not (Test-Path -LiteralPath (Join-Path $game 'RTV.pck') -PathType Leaf)) {
    throw 'Stored game path is not a Road to Vostok Steam installation; nothing was deleted.'
}
$radar = Join-Path $game 'mods\RtVRadarLoot.vmz'
foreach ($asset in @(
    @{ Path = $radar; Hash = $radarHash; Name = 'Radar VMZ' },
    @{ Path = $toolExe; Hash = [string]$receipt.ToolkitSha256; Name = 'Toolkit executable' }
)) {
    if ((Test-Path -LiteralPath $asset.Path -PathType Leaf) -and
        (Get-FileHash -LiteralPath $asset.Path -Algorithm SHA256).Hash -ine $asset.Hash) {
        throw "$($asset.Name) differs from the installed release; refusing to delete it: $($asset.Path)"
    }
}
if (Test-Path -LiteralPath $installedUninstaller -PathType Leaf) {
    if ((Get-FileHash -LiteralPath $installedUninstaller -Algorithm SHA256).Hash -ine [string]$receipt.UninstallerSha256) {
        throw 'The installed uninstaller has changed; refusing to delete it.'
    }
}
if (-not $NoIntegration) {
    if (Test-Path -LiteralPath $registry) {
        $entry = Get-ItemProperty -LiteralPath $registry
        if ($entry.DisplayName -ne 'Sobolyatnik-K (Experimental)' -or
            $entry.UninstallString -notlike ('*' + $installedUninstaller + '*')) {
            throw 'The Windows uninstall entry has been changed; refusing to remove it.'
        }
    }
    if (Test-Path -LiteralPath $shortcuts -PathType Container) {
        $shell = New-Object -ComObject WScript.Shell
        foreach ($name in @('Sobolyatnik-K Toolkit.lnk', 'Uninstall Sobolyatnik-K.lnk')) {
            $link = Join-Path $shortcuts $name
            if (Test-Path -LiteralPath $link -PathType Leaf) {
                $shortcut = $shell.CreateShortcut($link)
                if ($shortcut.TargetPath -ine (Join-Path $env:SystemRoot 'System32\cmd.exe') -and
                    $shortcut.TargetPath -ine (Join-Path $env:SystemRoot 'System32\WindowsPowerShell\v1.0\powershell.exe')) {
                    throw "Shortcut was replaced; refusing to remove it: $link"
                }
            }
        }
    }
}
# Recheck immediately before the first removal; keep receipts/backups if a file is locked.
if (Get-Process -Name RTV -ErrorAction SilentlyContinue) { throw 'Road to Vostok started; uninstall stopped.' }
foreach ($asset in @($radar, $toolExe, $installedUninstaller)) {
    if (Test-Path -LiteralPath $asset -PathType Leaf) { Remove-Item -LiteralPath $asset -Force -ErrorAction Stop }
}
if (-not $NoIntegration) {
    if (Test-Path -LiteralPath $shortcuts -PathType Container) {
        foreach ($name in @('Sobolyatnik-K Toolkit.lnk', 'Uninstall Sobolyatnik-K.lnk')) {
            $link = Join-Path $shortcuts $name
            if (Test-Path -LiteralPath $link -PathType Leaf) { Remove-Item -LiteralPath $link -Force }
        }
        if (@(Get-ChildItem -LiteralPath $shortcuts -Force).Count -eq 0) { Remove-Item -LiteralPath $shortcuts -Force }
    }
    if (Test-Path -LiteralPath $registry) { Remove-Item -LiteralPath $registry -Force }
}
Remove-Item -LiteralPath $receiptPath -Force
if ((Test-Path -LiteralPath $toolDir -PathType Container) -and
    @(Get-ChildItem -LiteralPath $toolDir -Force).Count -eq 0) {
    Remove-Item -LiteralPath $toolDir -Force
}
Write-Host 'Removed the Sobolyatnik-K radar and Toolkit. Metro, saves, and rollback backups were left untouched.'
