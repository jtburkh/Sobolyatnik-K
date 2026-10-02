#requires -Version 5.1
param(
    [Parameter(Mandatory = $true)][string] $ArchivePath,
    [Parameter(Mandatory = $true)][string] $LoaderSourceDirectory,
    [switch] $TestLoaderDownload
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$installer = (Resolve-Path (Join-Path $PSScriptRoot '..\..\tools\install-sobolyatnik.ps1')).ProviderPath
$archive = (Resolve-Path -LiteralPath $ArchivePath).ProviderPath
$loaderSource = (Resolve-Path -LiteralPath $LoaderSourceDirectory).ProviderPath
$root = Join-Path ([IO.Path]::GetTempPath()) ('sobolyatnik-installer-test-' + [guid]::NewGuid().ToString('N'))
$steam = Join-Path $root 'Steam'
$library = Join-Path $root 'Library 2'
$steamApps = Join-Path $library 'steamapps'
$game = Join-Path $steamApps 'common\Road to Vostok'
$modDir = Join-Path $game 'mods'
$target = Join-Path $modDir 'RtVRadarLoot.vmz'
$backups = Join-Path $env:LOCALAPPDATA 'Sobolyatnik-K\backups'
$before = @(Get-ChildItem -LiteralPath $backups -Filter 'RtVRadarLoot-*.vmz' -ErrorAction SilentlyContinue | ForEach-Object FullName)
$created = @()

function Check([bool] $Condition, [string] $Message) {
    if (-not $Condition) { throw "FAIL: $Message" }
}
function Fails([scriptblock] $Action, [string] $Expected) {
    $message = ''
    try { & $Action | Out-Null } catch { $message = $_.Exception.Message }
    Check ($message -like "*$Expected*") "Expected failure containing '$Expected', got '$message'"
}
try {
    New-Item -ItemType Directory -Force -Path (Join-Path $steam 'config'), $game | Out-Null
    $escaped = $library.Replace('\', '\\')
    [IO.File]::WriteAllText((Join-Path $steam 'config\libraryfolders.vdf'), "`"libraryfolders`"`n{`n  `"1`"`n  {`n    `"path`" `"$escaped`"`n  }`n}")
    [IO.File]::WriteAllText((Join-Path $steamApps 'appmanifest_1963610.acf'), "`"AppState`"`n{`n `"appid`" `"1963610`"`n `"installdir`" `"Road to Vostok`"`n `"buildid`" `"25632875`"`n}")
    [IO.File]::WriteAllText((Join-Path $game 'RTV.exe'), 'offline test fixture only')
    [IO.File]::WriteAllText((Join-Path $game 'RTV.pck'), 'offline test fixture only')
    & $installer -SteamRoot $steam -ArchivePath $archive -LoaderSourceDirectory $loaderSource -DryRun | Out-Null
    Check (-not (Test-Path -LiteralPath $target) -and -not (Test-Path -LiteralPath (Join-Path $game 'override.cfg'))) 'DryRun changed the game directory'
    & $installer -SteamRoot $steam -ArchivePath $archive -LoaderSourceDirectory $loaderSource | Out-Null
    Check ((Get-FileHash (Join-Path $game 'modloader.gd') -Algorithm SHA256).Hash -eq '60FCF7FEEC0A47C6472E3B7A190B46987B374618AE3D149C081B222542BC6135') 'Metro script not installed from pinned upstream asset'
    Check ((Get-FileHash (Join-Path $game 'override.cfg') -Algorithm SHA256).Hash -eq '9750A66FCF0CB1D9BF84284271F52E064F455CDD5A1CDC4981A007E4011A684B') 'Metro override not installed from pinned upstream asset'
    Check ((Get-FileHash $target -Algorithm SHA256).Hash -eq '66FD97A4C1487BE6688EF94B59EE709A3BAA8C7886C490D2F03AF7B8C395EEFC') 'Installed VMZ differs from release archive'
    $extraLibrary = Join-Path $root 'Library 3'
    $extraApps = Join-Path $extraLibrary 'steamapps'
    $extraGame = Join-Path $extraApps 'common\Road to Vostok'
    New-Item -ItemType Directory -Force -Path $extraGame | Out-Null
    [IO.File]::WriteAllText((Join-Path $extraApps 'appmanifest_1963610.acf'), '"installdir" "Road to Vostok"')
    Add-Content -LiteralPath (Join-Path $steam 'config\libraryfolders.vdf') -Value ("`n`"path`" `"" + $extraLibrary.Replace('\', '\\') + '"')
    Fails { & $installer -SteamRoot $steam -ArchivePath $archive } 'Multiple Road to Vostok installations'
    & $installer -GamePath $game -ArchivePath $archive | Out-Null
    Remove-Item -LiteralPath $extraApps -Recurse -Force
    & $installer -SteamRoot $steam -ArchivePath $archive | Out-Null
    Check (@(Get-ChildItem -LiteralPath $modDir -File).Count -eq 1) 'Idempotent install left another mod or temp file'
    [IO.File]::WriteAllText($target, 'old-version-fixture')
    & $installer -SteamRoot $steam -ArchivePath $archive | Out-Null
    Check ((Get-FileHash $target -Algorithm SHA256).Hash -eq '66FD97A4C1487BE6688EF94B59EE709A3BAA8C7886C490D2F03AF7B8C395EEFC') 'Upgrade failed'
    $after = @(Get-ChildItem -LiteralPath $backups -Filter 'RtVRadarLoot-*.vmz' | ForEach-Object FullName)
    $created = @($after | Where-Object { $before -notcontains $_ })
    Check ($created.Count -eq 1) 'Upgrade did not create exactly one rollback copy'
    Check (([IO.File]::ReadAllText($created[0])) -eq 'old-version-fixture') 'Rollback bytes differ from old archive'
    $bad = Join-Path $root 'tampered.vmz'
    [IO.File]::WriteAllText($bad, 'untrusted download')
    Fails { & $installer -GamePath $game -ArchivePath $bad } 'SHA-256 mismatch'
    Check ((Get-FileHash $target -Algorithm SHA256).Hash -eq '66FD97A4C1487BE6688EF94B59EE709A3BAA8C7886C490D2F03AF7B8C395EEFC') 'Tampered download modified installed mod'
    Remove-Item -LiteralPath (Join-Path $game 'override.cfg') -Force
    Fails { & $installer -GamePath $game -ArchivePath $archive } 'partial Metro installation exists'
    [IO.File]::WriteAllText((Join-Path $game 'override.cfg'), 'offline test fixture only')
    [IO.File]::WriteAllText((Join-Path $game 'modloader.gd'), 'const MODLOADER_VERSION := "3.1.0"')
    Fails { & $installer -GamePath $game -ArchivePath $archive } 'Only Metro Mod Loader 3.2.1'
    [IO.File]::WriteAllText((Join-Path $game 'modloader.gd'), 'const MODLOADER_VERSION := "3.2.1" # locally patched')
    Fails { & $installer -GamePath $game -ArchivePath $archive } 'differs from the tested official release'
    Copy-Item -LiteralPath (Join-Path $loaderSource 'modloader.gd') -Destination (Join-Path $game 'modloader.gd') -Force
    [IO.File]::WriteAllText((Join-Path $steamApps 'appmanifest_1963610.acf'), '"buildid" "00000000"')
    Fails { & $installer -GamePath $game -ArchivePath $archive } 'Steam build 25632875 is required'
    [IO.File]::WriteAllText((Join-Path $steamApps 'appmanifest_1963610.acf'), '"buildid" "25632875"')
    [IO.File]::WriteAllText((Join-Path $modDir 'OtherMod.vmz'), 'not ours')
    Fails { & $installer -GamePath $game -ArchivePath $archive } 'Other VMZs are installed'
    Remove-Item -LiteralPath (Join-Path $modDir 'OtherMod.vmz') -Force
    function Get-Process { param($Name, $ErrorAction) return [pscustomobject]@{Name = 'RTV'} }
    Fails { & $installer -GamePath $game -ArchivePath $archive } 'Road to Vostok is running'
    Remove-Item Function:\Get-Process -Force
    Remove-Item -LiteralPath $target, (Join-Path $game 'modloader.gd'), (Join-Path $game 'override.cfg') -Force
    $tamperedLoader = Join-Path $root 'bad-loader'
    New-Item -ItemType Directory -Path $tamperedLoader | Out-Null
    [IO.File]::WriteAllText((Join-Path $tamperedLoader 'modloader.gd'), 'const MODLOADER_VERSION := "3.2.1" # changed')
    Copy-Item -LiteralPath (Join-Path $loaderSource 'override.cfg') -Destination $tamperedLoader
    Fails { & $installer -GamePath $game -ArchivePath $archive -LoaderSourceDirectory $tamperedLoader } 'modloader.gd SHA-256 mismatch'
    Check (-not (Test-Path -LiteralPath (Join-Path $game 'modloader.gd'))) 'Unverified loader modified the game'
    $global:rtvCalls = 0
    function Get-Process {
        param($Name, $ErrorAction)
        $global:rtvCalls++
        if ($global:rtvCalls -ge 3) { return [pscustomobject]@{Name = 'RTV'} }
    }
    Fails { & $installer -GamePath $game -ArchivePath $archive -LoaderSourceDirectory $loaderSource } 'Road to Vostok is running'
    Remove-Item Function:\Get-Process -Force
    Check (-not (Test-Path -LiteralPath (Join-Path $game 'modloader.gd')) -and
           -not (Test-Path -LiteralPath (Join-Path $game 'override.cfg')) -and
           -not (Test-Path -LiteralPath $target)) 'Failed mod install left a half-installed loader or VMZ'
    Check (@(Get-ChildItem -LiteralPath $game -Recurse -Filter '*.rtvtmp' -File).Count -eq 0) 'Temporary staging files were not cleaned'
    if ($TestLoaderDownload) {
        & $installer -GamePath $game -ArchivePath $archive | Out-Null
        Check ((Get-FileHash (Join-Path $game 'modloader.gd') -Algorithm SHA256).Hash -eq '60FCF7FEEC0A47C6472E3B7A190B46987B374618AE3D149C081B222542BC6135') 'Official loader download/install failed'
        Check (Test-Path -LiteralPath $target -PathType Leaf) 'Radar missing after official download'
        Write-Host 'SMOKE OK: direct pinned upstream Metro downloads for new user'
    }
    Write-Host 'SMOKE OK: Windows PowerShell fixture fresh official Metro+radar install, multiple-library ambiguity, idempotence, DryRun, rollback, tamper/build/loader/running-game/other-mod rejection'
} finally {
    Remove-Item -LiteralPath $root -Recurse -Force -ErrorAction SilentlyContinue
    foreach ($backup in $created) {
        if ($backup -and (Test-Path -LiteralPath $backup)) { Remove-Item -LiteralPath $backup -Force }
    }
}
