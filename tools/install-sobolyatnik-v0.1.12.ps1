#requires -Version 5.1
<#
.SYNOPSIS
    Install the version-pinned Shot Alerts VMZ for Road to Vostok Build 2.
.DESCRIPTION
    Downloads the exact official Metro 3.2.1 files and our VMZ, verifying all
    three SHA-256 hashes before writing anything. Installs Metro only when
    neither loader file exists; never overwrites an existing loader. Does not
    modify game binaries, saves, or user config. Run while the game is closed.
    -ArchivePath, -LoaderSourceDirectory, and -SteamRoot support offline tests.
#>
[CmdletBinding()]
param(
    [string] $GamePath,
    [string] $SteamRoot,
    [string] $ArchivePath,
    [string] $LoaderSourceDirectory,
    [switch] $DryRun
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$version = '0.1.12'
$expectedBuild = '25632875'
$expectedHash = '6d2d06e55831f174483595d44fde6ba5c3a50f7fec183d2c66a5953ecb98efdd'
$downloadUrl = 'https://github.com/jtburkh/Sobolyatnik-K/releases/download/v0.1.12-experimental/RtVRadarLoot.vmz'
$modFile = 'RtVRadarLoot.vmz'
$loaderInfo = 'https://modworkshop.net/mod/55623'
$loaderRelease = 'https://github.com/ametrocavich/vostok-mod-loader/releases/download/v3.2.1'
$loaderScriptHash = '60fcf7feec0a47c6472e3b7a190b46987b374618ae3d149c081b222542bc6135'
$loaderOverrideHash = '9750a66fcf0cb1d9bf84284271f52e064f455cdd5a1cdc4981a007e4011a684b'

function Assert-GameClosed {
    if (Get-Process -Name 'RTV' -ErrorAction SilentlyContinue) {
        throw 'Road to Vostok is running. Close the game before installing or updating the VMZ.'
    }
}

function Get-VdfField([string] $Text, [string] $Name) {
    $match = [regex]::Match($Text, '(?m)^\s*"' + [regex]::Escape($Name) + '"\s*"([^"\r\n]*)"')
    if (-not $match.Success) { return '' }
    return $match.Groups[1].Value.Replace('\\', '\')
}

function Get-Libraries([string] $Root) {
    $libraries = New-Object System.Collections.Generic.List[string]
    $libraries.Add($Root)
    $vdf = Join-Path $Root 'config\libraryfolders.vdf'
    if (Test-Path -LiteralPath $vdf -PathType Leaf) {
        $contents = [IO.File]::ReadAllText($vdf)
        foreach ($match in [regex]::Matches($contents, '(?m)^\s*"path"\s*"([^"\r\n]*)"')) {
            $libraries.Add($match.Groups[1].Value.Replace('\\', '\'))
        }
        foreach ($match in [regex]::Matches($contents, '(?m)^\s*"\d+"\s*"([A-Za-z]:\\[^"\r\n]*)"')) {
            $libraries.Add($match.Groups[1].Value.Replace('\\', '\'))
        }
    }
    return $libraries | Select-Object -Unique
}

function Resolve-Game {
    if ($GamePath) {
        $paths = @($GamePath)
    } else {
        $roots = New-Object System.Collections.Generic.List[string]
        if ($SteamRoot) {
            $roots.Add($SteamRoot)
        } else {
            foreach ($key in @('HKCU:\Software\Valve\Steam', 'HKLM:\SOFTWARE\WOW6432Node\Valve\Steam')) {
                $entry = Get-ItemProperty -Path $key -ErrorAction SilentlyContinue
                if ($null -ne $entry) {
                    foreach ($field in @('SteamPath', 'InstallPath')) {
                        if ($entry.PSObject.Properties[$field] -and $entry.PSObject.Properties[$field].Value) {
                            $roots.Add([string] $entry.PSObject.Properties[$field].Value)
                        }
                    }
                }
            }
            if (${env:ProgramFiles(x86)}) { $roots.Add((Join-Path ${env:ProgramFiles(x86)} 'Steam')) }
        }
        $paths = @(
            foreach ($root in ($roots | Select-Object -Unique)) {
                if (-not (Test-Path -LiteralPath $root -PathType Container)) { continue }
                foreach ($library in (Get-Libraries $root)) {
                    $manifest = Join-Path $library 'steamapps\appmanifest_1963610.acf'
                    if (-not (Test-Path -LiteralPath $manifest -PathType Leaf)) { continue }
                    $manifestText = [IO.File]::ReadAllText($manifest)
                    $installDir = Get-VdfField $manifestText 'installdir'
                    if (-not $installDir -or $installDir -match '[\\/:]' -or $installDir -in @('.', '..')) { continue }
                    Join-Path (Join-Path $library 'steamapps\common') $installDir
                }
            }
        ) | Select-Object -Unique
    }
    # Steam registry/config often disagree only in capitalization; Windows paths
    # are case-insensitive, so don't report a false "multiple installs" error.
    $paths = @($paths | Where-Object { $_ -and (Test-Path -LiteralPath $_ -PathType Container) } | Sort-Object -Unique)
    if ($paths.Count -eq 0) { throw 'Road to Vostok Steam installation not found. Supply -GamePath explicitly.' }
    if ($paths.Count -gt 1) { throw ('Multiple Road to Vostok installations found; choose one with -GamePath: ' + ($paths -join '; ')) }
    $path = (Resolve-Path -LiteralPath $paths[0]).ProviderPath
    if (-not (Test-Path -LiteralPath (Join-Path $path 'RTV.exe') -PathType Leaf) -or
        -not (Test-Path -LiteralPath (Join-Path $path 'RTV.pck') -PathType Leaf)) {
        throw "Not a Road to Vostok installation: $path"
    }
    $steamApps = Split-Path (Split-Path $path -Parent) -Parent
    $manifestPath = Join-Path $steamApps 'appmanifest_1963610.acf'
    if (-not (Test-Path -LiteralPath $manifestPath -PathType Leaf) -or
        (Get-VdfField ([IO.File]::ReadAllText($manifestPath)) 'buildid') -ne $expectedBuild) {
        throw "Steam build $expectedBuild is required. Check $manifestPath before installing onto a different game build."
    }
    return $path
}

function Get-MetroState([string] $Path) {
    $scriptPath = Join-Path $Path 'modloader.gd'
    $overridePath = Join-Path $Path 'override.cfg'
    $hasScript = Test-Path -LiteralPath $scriptPath -PathType Leaf
    $hasOverride = Test-Path -LiteralPath $overridePath -PathType Leaf
    if (-not $hasScript -and -not $hasOverride) { return 'Missing' }
    if (-not $hasScript -or -not $hasOverride) {
        throw "A partial Metro installation exists. Inspect $scriptPath and $overridePath or use the official instructions at $loaderInfo; no files were changed."
    }
    if ([IO.File]::ReadAllText($scriptPath) -notmatch 'const\s+MODLOADER_VERSION\s*:=\s*"3\.2\.1"') {
        throw 'Only Metro Mod Loader 3.2.1 has been tested with this VMZ. No files were changed.'
    }
    if ((Get-FileHash -LiteralPath $scriptPath -Algorithm SHA256).Hash -ine $loaderScriptHash) {
        throw 'Existing Metro 3.2.1 script differs from the tested official release. No loader files were overwritten.'
    }
    return 'Existing'
}

function Assert-Hash([string] $Path, [string] $Expected, [string] $Name) {
    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) { throw "$Name unavailable: $Path" }
    $actual = (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash
    if ($actual -ine $Expected) { throw "$Name SHA-256 mismatch ($actual); expected $Expected. No game files were changed." }
}

function Assert-Archive([string] $Path) {
    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) { throw "Archive unavailable: $Path" }
    $hash = (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash
    if ($hash -ine $expectedHash) { throw "VMZ SHA-256 mismatch ($hash); expected $expectedHash. No game files were changed." }
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $zip = [IO.Compression.ZipFile]::OpenRead($Path)
    try {
        $manifest = $zip.GetEntry('mod.txt')
        if ($null -eq $manifest) { throw 'VMZ is missing mod.txt.' }
        $reader = New-Object IO.StreamReader($manifest.Open())
        try { $text = $reader.ReadToEnd() } finally { $reader.Dispose() }
        if ($text -notmatch 'id="rtv_toolkit_radar_loot"' -or $text -notmatch ('version="' + [regex]::Escape($version) + '"')) {
            throw 'VMZ manifest has an unexpected mod ID or version.'
        }
    } finally { $zip.Dispose() }
}

Assert-GameClosed
$game = Resolve-Game
$metroState = Get-MetroState $game
$mods = Join-Path $game 'mods'
$destination = Join-Path $mods $modFile
if (Test-Path -LiteralPath $mods -PathType Container) {
    $otherMods = @(Get-ChildItem -LiteralPath $mods -Filter '*.vmz' -File | Where-Object { $_.Name -ne $modFile })
    if ($otherMods.Count -ne 0) { throw "Other VMZs are installed ($($otherMods.Name -join ', ')). Remove conflicting mods manually before this isolated test." }
}

$downloads = New-Object System.Collections.Generic.List[string]
$loaderAssets = @()
$stage = $null
$replacementBackup = $null
$modCommitted = $false
$loaderCreated = New-Object System.Collections.Generic.List[object]
try {
    if ($ArchivePath) {
        $archive = (Resolve-Path -LiteralPath $ArchivePath).ProviderPath
    } else {
        $archive = Join-Path ([IO.Path]::GetTempPath()) ('sobolyatnik-' + [guid]::NewGuid().ToString('N') + '.vmz')
        $downloads.Add($archive)
        Write-Host "Downloading Sobolyatnik-K $version from $downloadUrl"
        Invoke-WebRequest -Uri $downloadUrl -OutFile $archive -UseBasicParsing
    }
    Assert-Archive $archive
    Write-Host "Verified Sobolyatnik-K $version (SHA-256 $expectedHash)."

    if ($metroState -eq 'Missing') {
        $sourceDir = $null
        if ($LoaderSourceDirectory) { $sourceDir = (Resolve-Path -LiteralPath $LoaderSourceDirectory).ProviderPath }
        foreach ($asset in @(
            @{ Name = 'modloader.gd'; Hash = $loaderScriptHash },
            @{ Name = 'override.cfg'; Hash = $loaderOverrideHash }
        )) {
            if ($sourceDir) {
                $source = Join-Path $sourceDir $asset.Name
            } else {
                $source = Join-Path ([IO.Path]::GetTempPath()) ('metro-' + [guid]::NewGuid().ToString('N') + '-' + $asset.Name)
                $downloads.Add($source)
                Write-Host "Downloading official Metro 3.2.1 $($asset.Name) from $loaderRelease"
                Invoke-WebRequest -Uri ($loaderRelease + '/' + $asset.Name) -OutFile $source -UseBasicParsing
            }
            Assert-Hash $source $asset.Hash $asset.Name
            $loaderAssets += @{ Name = $asset.Name; Hash = $asset.Hash; Source = $source; Stage = $null }
        }
        Write-Host 'Verified both official Metro 3.2.1 release files.'
    }
    $needsMod = -not ((Test-Path -LiteralPath $destination -PathType Leaf) -and
        (Get-FileHash -LiteralPath $destination -Algorithm SHA256).Hash -ieq $expectedHash)
    if ($metroState -eq 'Existing' -and -not $needsMod) {
        Write-Host "Already installed: $destination (Metro 3.2.1 present)"
        return
    }
    if ($DryRun) {
        if ($metroState -eq 'Missing') { Write-Host "Dry run: would install verified Metro 3.2.1 into $game" }
        if ($needsMod) { Write-Host "Dry run: would install verified Sobolyatnik-K $version into $destination" }
        return
    }

    if ($needsMod) {
        if (-not (Test-Path -LiteralPath $mods -PathType Container)) {
            New-Item -ItemType Directory -Path $mods -ErrorAction Stop | Out-Null
        }
        $stage = Join-Path $mods ('RtVRadarLoot-' + [guid]::NewGuid().ToString('N') + '.rtvtmp')
        Copy-Item -LiteralPath $archive -Destination $stage -ErrorAction Stop
        Assert-Archive $stage
    }
    if ($metroState -eq 'Missing') {
        foreach ($asset in $loaderAssets) {
            $asset.Stage = Join-Path $game ($asset.Name + '.' + [guid]::NewGuid().ToString('N') + '.rtvtmp')
            Copy-Item -LiteralPath $asset.Source -Destination $asset.Stage -ErrorAction Stop
            Assert-Hash $asset.Stage $asset.Hash $asset.Name
        }
    }
    if ($needsMod -and (Test-Path -LiteralPath $destination -PathType Leaf)) {
        $backupDir = Join-Path $env:LOCALAPPDATA 'Sobolyatnik-K\backups'
        New-Item -ItemType Directory -Force -Path $backupDir | Out-Null
        $oldHash = (Get-FileHash -LiteralPath $destination -Algorithm SHA256).Hash.ToLowerInvariant()
        $backup = Join-Path $backupDir ('RtVRadarLoot-' + (Get-Date -Format 'yyyyMMdd-HHmmss') + '-' + $oldHash.Substring(0, 12) + '-' + [guid]::NewGuid().ToString('N').Substring(0, 8) + '.vmz')
        Copy-Item -LiteralPath $destination -Destination $backup -ErrorAction Stop
        Assert-Hash $backup $oldHash 'Rollback backup'
        Write-Host "Previous VMZ backed up to $backup"
    }

    Assert-GameClosed
    if ($metroState -eq 'Missing') {
        if ((Get-MetroState $game) -ne 'Missing') { throw 'Metro files appeared during setup; refusing to overwrite them.' }
        # Install the loader script first; override.cfg activates it on next game launch.
        foreach ($asset in $loaderAssets) {
            $target = Join-Path $game $asset.Name
            [IO.File]::Move($asset.Stage, $target)
            $loaderCreated.Add(@{ Path = $target; Hash = $asset.Hash })
            $asset.Stage = $null
        }
        Write-Host "Installed official Metro Mod Loader 3.2.1 into $game"
    }
    if ($needsMod) {
        Assert-GameClosed
        if (Test-Path -LiteralPath $destination -PathType Leaf) {
            $replacementBackup = Join-Path $mods ('RtVRadarLoot-' + [guid]::NewGuid().ToString('N') + '.rtvbak')
            [IO.File]::Replace($stage, $destination, $replacementBackup)
        } else {
            [IO.File]::Move($stage, $destination)
        }
        $stage = $null
        $modCommitted = $true
    }
    Assert-Archive $destination
    $modCommitted = $true
    if ($replacementBackup -and (Test-Path -LiteralPath $replacementBackup)) {
        Remove-Item -LiteralPath $replacementBackup -Force
        $replacementBackup = $null
    }
    Write-Host "Installed: $destination"
    Write-Host 'Launch Road to Vostok. F7 cycles radar layers; F8 hides/shows the overlay.'
} catch {
    if (-not $modCommitted) {
        # Roll back only pristine loader files created by this run, never pre-existing
        # loader files or files someone changed after the move.
        foreach ($created in $loaderCreated) {
            if ((Test-Path -LiteralPath $created.Path -PathType Leaf) -and
                (Get-FileHash -LiteralPath $created.Path -Algorithm SHA256).Hash -ieq $created.Hash) {
                Remove-Item -LiteralPath $created.Path -Force
            }
        }
    }
    throw
} finally {
    if ($stage -and (Test-Path -LiteralPath $stage)) { Remove-Item -LiteralPath $stage -Force }
    foreach ($asset in $loaderAssets) {
        if ($asset.Stage -and (Test-Path -LiteralPath $asset.Stage)) { Remove-Item -LiteralPath $asset.Stage -Force }
    }
    foreach ($download in $downloads) {
        if (Test-Path -LiteralPath $download) { Remove-Item -LiteralPath $download -Force }
    }
}
