param(
    [switch]$ResetDebug,
    [switch]$PackagesOnly,
    [string]$KeepPackage = ''
)
$ErrorActionPreference = 'Stop'
$projectRoot = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$targetRoot = Join-Path $projectRoot 'target'
$qaRoot = Join-Path $projectRoot '.qa'
$activePrograms = @(Get-CimInstance Win32_Process -ErrorAction SilentlyContinue | Where-Object ExecutablePath | Select-Object -ExpandProperty ExecutablePath)
$removedBytes = 0L
foreach ($root in @($targetRoot, $qaRoot)) {
    if ((Test-Path -LiteralPath $root) -and ((Get-Item -LiteralPath $root -Force).Attributes -band [IO.FileAttributes]::ReparsePoint)) {
        throw "Generated-artifact root is redirected: $root"
    }
}

function Remove-GeneratedArtifact([string]$Path, [string]$AllowedRoot) {
    if (-not (Test-Path -LiteralPath $Path)) { return }
    $absolute = [IO.Path]::GetFullPath((Get-Item -LiteralPath $Path).FullName)
    $allowed = [IO.Path]::GetFullPath($AllowedRoot).TrimEnd('\') + '\'
    if (-not $absolute.StartsWith($allowed, [StringComparison]::OrdinalIgnoreCase)) {
        throw "Cleanup target is outside its generated-artifact root: $absolute"
    }
    $item = Get-Item -LiteralPath $absolute -Force
    if ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) {
        throw "Refusing to clean a redirected artifact path: $absolute"
    }
    $ancestor = $item.Directory
    if ($item.PSIsContainer) { $ancestor = $item.Parent }
    while ($ancestor -and $ancestor.FullName.StartsWith($allowed, [StringComparison]::OrdinalIgnoreCase)) {
        if ($ancestor.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw "Cleanup ancestor is redirected: $($ancestor.FullName)" }
        $ancestor = $ancestor.Parent
    }
    $prefix = $absolute.TrimEnd('\') + '\'
    if ($activePrograms | Where-Object { $_ -eq $absolute -or $_.StartsWith($prefix, [StringComparison]::OrdinalIgnoreCase) }) { return }
    if ($item.PSIsContainer) {
        # Never traverse a junction or symbolic link while deleting generated data.
        if (Get-ChildItem -LiteralPath $absolute -Recurse -Force -ErrorAction Stop | Where-Object { $_.Attributes -band [IO.FileAttributes]::ReparsePoint }) {
            throw "Refusing to clean artifacts containing redirected paths: $absolute"
        }
        $bytes = (Get-ChildItem -LiteralPath $absolute -Recurse -File -Force | Measure-Object Length -Sum).Sum
        Remove-Item -LiteralPath $absolute -Recurse -Force
    } else {
        $bytes = $item.Length
        Remove-Item -LiteralPath $absolute -Force
    }
    $script:removedBytes += [long]$bytes
}

$knownPackageFiles = @(
    'LICENSE',
    'NOTICE',
    'README.en.md',
    'README.md',
    'README.zh-CN.md',
    'Start-Company.ps1',
    'Start-Company.sh',
    'THIRD_PARTY_NOTICES.md',
    'company',
    'company-launch',
    'company-launch.exe',
    'company-server',
    'company-server.exe',
    'company.exe',
    'docs/agent-interface.en.md',
    'docs/agent-interface.md',
    'docs/getting-started.md',
    'docs/getting-started.zh-CN.md',
    'docs/hosted-skills.en.md',
    'docs/hosted-skills.md',
    'docs/licenses/fusion-pixel-OFL-1.1.txt',
    'docs/licenses/markdown-runtime-notices.txt',
    'docs/licenses/pixelarticons-MIT.txt',
    'docs/licenses/playwright-Apache-2.0.txt',
    'docs/media/company-icon-dark.ico',
    'docs/media/company-icon-dark.png',
    'docs/media/company-icon-dark.svg',
    'docs/media/company-icon-light.ico',
    'docs/media/company-icon-light.png',
    'docs/media/company-icon-light.svg',
    'docs/media/company-icon.ico',
    'docs/media/company-icon.png',
    'docs/media/company-icon.svg',
    'docs/media/company-mark-dark.png',
    'docs/media/company-mark-dark.svg',
    'docs/media/company-mark-light.png',
    'docs/media/company-mark-light.svg',
    'docs/media/company-mark.png',
    'docs/media/company-mark.svg',
    'docs/media/demo-en.png',
    'docs/media/demo-zh-CN.png',
    'docs/media/hero-dark.svg',
    'docs/media/hero.svg',
    'docs/operations.md',
    'docs/plugins.en.md',
    'docs/plugins.md',
    'examples/demo-company.json',
    'examples/demo-operations.json',
    'examples/plugins/backup-config.json',
    'examples/plugins/backup_destination.py',
    'examples/plugins/business-summary.json',
    'examples/plugins/business_summary.py',
    'examples/plugins/local-backup.json',
    'examples/plugins/operating-skills.json',
    'examples/plugins/process-backup.json',
    'examples/simulation-company.en.json',
    'examples/simulation-company.json',
    'examples/simulation-operations.en.json',
    'examples/simulation-operations.json',
    'scripts/capture-company.mjs',
    'scripts/load-example.ps1',
    'scripts/renderer/capture-company.mjs',
    'scripts/renderer/package-lock.json',
    'scripts/renderer/package.json',
    'skills/company-operating/cashflow-review/SKILL.en.md',
    'skills/company-operating/cashflow-review/SKILL.md',
    'skills/company-operating/customer-interviews/SKILL.en.md',
    'skills/company-operating/customer-interviews/SKILL.md',
    'skills/company-operating/simulation-gm/SKILL.en.md',
    'skills/company-operating/simulation-gm/SKILL.md',
    'skills/company-operating/weekly-retrospective/SKILL.en.md',
    'skills/company-operating/weekly-retrospective/SKILL.md',
    'skills/transplane-company/SKILL.md',
    'skills/transplane-company/agents/openai.yaml',
    'skills/transplane-company/references/entry.en.md'
)

function Test-GeneratedPackage([IO.DirectoryInfo]$Package) {
    $manifestPath = Join-Path $Package.FullName 'release-manifest.json'
    if (-not (Test-Path -LiteralPath $manifestPath -PathType Leaf)) { return $false }
    if ($Package.Attributes -band [IO.FileAttributes]::ReparsePoint) { return $false }
    $entries = @(Get-ChildItem -LiteralPath $Package.FullName -Recurse -Force)
    if ($entries | Where-Object { $_.Attributes -band [IO.FileAttributes]::ReparsePoint }) { return $false }
    try {
        $manifest = Get-Content -Raw -LiteralPath $manifestPath | ConvertFrom-Json
        if ($manifest.format -ne 'transplane-company-release' -or -not $manifest.files) { return $false }
        $declared = @($manifest.files.PSObject.Properties)
        $files = @($entries | Where-Object { -not $_.PSIsContainer })
        if ($files.Count -ne $declared.Count + 1) { return $false }
        foreach ($entry in $declared) {
            if ($entry.Name -notin $knownPackageFiles -or $entry.Value -notmatch '^[a-fA-F0-9]{64}$') { return $false }
            $file = Join-Path $Package.FullName ($entry.Name.Replace('/', [IO.Path]::DirectorySeparatorChar))
            if (-not (Test-Path -LiteralPath $file -PathType Leaf)) { return $false }
            if ((Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash -ne $entry.Value) { return $false }
        }
        # Empty or extra directories are never part of a generated release manifest.
        foreach ($directory in @($entries | Where-Object PSIsContainer)) {
            $relative = $directory.FullName.Substring($Package.FullName.Length + 1).Replace('\', '/') + '/'
            if (-not ($declared | Where-Object { $_.Name.StartsWith($relative, [StringComparison]::OrdinalIgnoreCase) })) { return $false }
        }
        return $true
    } catch { return $false }
}

if (-not $PackagesOnly) {
    if ($ResetDebug) {
        Remove-GeneratedArtifact (Join-Path $targetRoot 'debug') $targetRoot
    } else {
        Remove-GeneratedArtifact (Join-Path $targetRoot 'debug\incremental') $targetRoot
        # Keep dependency libraries; discard old workspace/test executables.
        foreach ($buildMode in @('debug', 'release')) {
            $dependencies = Join-Path $targetRoot "$buildMode\deps"
            if (Test-Path -LiteralPath $dependencies) {
                foreach ($file in Get-ChildItem -LiteralPath $dependencies -File -Filter '*.exe') {
                    Remove-GeneratedArtifact $file.FullName $targetRoot
                }
            }
        }
    }
    if (Test-Path -LiteralPath $qaRoot) {
        # Only first-level frozen test-program folders; retain all QA records,
        # snapshots, company files and the live preview deployment.
        foreach ($directory in Get-ChildItem -LiteralPath $qaRoot -Directory) {
            if ($directory.Name -notlike 'game-preview*') {
                $binPath = Join-Path $directory.FullName 'bin'
                if (Test-Path -LiteralPath $binPath) {
                    $entries = @(Get-ChildItem -LiteralPath $binPath -Force)
                    $hasCompanyBinary = $entries | Where-Object { $_.Name -in @('company.exe','company-server.exe','company','company-server') }
                    $unknown = $entries | Where-Object { $_.PSIsContainer -or ($_.Name -notin @('company','company-server') -and $_.Extension -notin @('.exe','.dll','.pdb','.so','.dylib')) }
                    if ($hasCompanyBinary -and -not $unknown) { Remove-GeneratedArtifact $binPath $qaRoot }
                }
            }
        }
    }
}

if (Test-Path -LiteralPath $targetRoot) {
    $packageCandidates = @(Get-ChildItem -LiteralPath $targetRoot -Directory -Filter 'package-*')
    $nestedPackages = Join-Path $targetRoot 'packages'
    if (Test-Path -LiteralPath $nestedPackages) {
        if ((Get-Item -LiteralPath $nestedPackages -Force).Attributes -band [IO.FileAttributes]::ReparsePoint) {
            throw "Release-package root is redirected: $nestedPackages"
        }
        $packageCandidates += @(Get-ChildItem -LiteralPath $nestedPackages -Directory | Where-Object { $_.Name -match '^TRANSPLANE-COMPANY-v[0-9]+\.[0-9]+\.[0-9]+-' })
    }
    $packages = @($packageCandidates | Sort-Object LastWriteTimeUtc -Descending)
    $keep = if ($KeepPackage) { [IO.Path]::GetFullPath($KeepPackage) } elseif ($packages.Count) { $packages[0].FullName } else { '' }
    foreach ($package in $packages) {
        if ($package.FullName -ne $keep) {
            if (-not (Test-GeneratedPackage $package)) { continue }
            Remove-GeneratedArtifact $package.FullName $targetRoot
            foreach ($archiveExtension in @('.zip','.tar.gz')) {
                $archivePath = $package.FullName + $archiveExtension
                $checksumPath = $archivePath + '.sha256'
                if (-not (Test-Path -LiteralPath $archivePath -PathType Leaf) -or -not (Test-Path -LiteralPath $checksumPath -PathType Leaf)) { continue }
                if (((Get-Item -LiteralPath $archivePath).Attributes -band [IO.FileAttributes]::ReparsePoint) -or ((Get-Item -LiteralPath $checksumPath).Attributes -band [IO.FileAttributes]::ReparsePoint)) { continue }
                $checksumLine = (Get-Content -Raw -LiteralPath $checksumPath).Trim()
                $archiveHash = (Get-FileHash -LiteralPath $archivePath -Algorithm SHA256).Hash.ToLowerInvariant()
                if ($checksumLine -ne ($archiveHash + '  ' + [IO.Path]::GetFileName($archivePath))) { continue }
                Remove-GeneratedArtifact $archivePath $targetRoot
                Remove-GeneratedArtifact $checksumPath $targetRoot
            }
        }
    }
}
[pscustomobject]@{RemovedBytes=$removedBytes; RemovedGiB=[math]::Round($removedBytes/1GB,3)}
