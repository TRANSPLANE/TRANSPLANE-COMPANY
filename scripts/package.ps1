param(
    [switch]$SkipBuild,
    [string]$Destination = '',
    [ValidateSet('windows-x86_64','windows-aarch64')][string]$Platform = 'windows-x86_64',
    [string]$BinaryDir = '',
    [string]$Version = ''
)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
if (-not $Version) {
    $manifest = Get-Content -Raw -LiteralPath (Join-Path $projectRoot 'Cargo.toml')
    $Version = [regex]::Match($manifest, '(?m)^version\s*=\s*"([^"]+)"').Groups[1].Value
    if (-not $Version) { throw 'Release version was not found in Cargo.toml.' }
}
if (-not $Destination) { $Destination = Join-Path $projectRoot "target\package-TRANSPLANE-COMPANY-v$Version-$Platform" }
if (-not $BinaryDir) { $BinaryDir = Join-Path $projectRoot 'target\release' }
if (-not $SkipBuild) {
    & (Join-Path $PSScriptRoot 'build-company.ps1') -BuildProfile Release -InstallDependencies
}
& python (Join-Path $PSScriptRoot 'package.py') --platform $Platform --version $Version --binary-dir $BinaryDir --destination $Destination
if ($LASTEXITCODE -ne 0) { throw 'Release packaging failed.' }
& (Join-Path $PSScriptRoot 'clean-build-artifacts.ps1') -PackagesOnly -KeepPackage $Destination | Out-Host
