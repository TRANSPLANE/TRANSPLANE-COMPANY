param(
    [ValidateSet('Debug','Release')][string]$BuildProfile = 'Release',
    [switch]$SkipWeb,
    [switch]$InstallDependencies,
    [switch]$Test
)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
$targetRoot = Join-Path $projectRoot 'target'
New-Item -ItemType Directory -Path $targetRoot -Force | Out-Null
$buildLock = [IO.File]::Open((Join-Path $targetRoot '.company-build.lock'), [IO.FileMode]::OpenOrCreate, [IO.FileAccess]::ReadWrite, [IO.FileShare]::None)
try {
    & (Join-Path $PSScriptRoot 'clean-build-artifacts.ps1') | Out-Host
    if (-not $SkipWeb) {
        Push-Location (Join-Path $projectRoot 'web')
        try {
            if ($InstallDependencies -or -not (Test-Path -LiteralPath 'node_modules')) {
                & npm.cmd ci
                if ($LASTEXITCODE -ne 0) { throw 'Frontend dependencies could not be installed.' }
            }
            & npm.cmd run build
            if ($LASTEXITCODE -ne 0) { throw 'Frontend build failed.' }
        } finally { Pop-Location }
    }
    Push-Location $projectRoot
    try {
        $arguments = @('build','--locked','--bins','--target-dir',$targetRoot)
        if ($BuildProfile -eq 'Release') { $arguments += '--release' }
        & cargo @arguments
        if ($LASTEXITCODE -ne 0) { throw 'Application build failed.' }
        if ($Test) {
            & cargo test --locked --workspace --target-dir $targetRoot
            if ($LASTEXITCODE -ne 0) { throw 'Workspace tests failed.' }
        }
    } finally { Pop-Location }
} finally { $buildLock.Dispose() }
