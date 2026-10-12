param(
    [string]$DataDir = (Join-Path $PSScriptRoot 'company-data'),
    [string]$Bind = '127.0.0.1:4177',
    [string]$DatabaseUrl = '',
    [ValidateSet('en', 'zh-CN')][string]$Lang = 'en',
    [switch]$Build,
    [switch]$NoBrowser
)
$ErrorActionPreference = 'Stop'
$messages = if ($Lang -eq 'zh-CN') {
    @{
        DependenciesFailed = '前端依赖安装失败。'
        FrontendFailed = '前端构建失败。'
        BackendFailed = '应用构建失败。'
        InitializationFailed = '部署初始化失败。'
        CredentialHint = '使用初始化时输出的主人凭据连接经营台。凭据只在首次创建时显示。'
        SourceRequired = '重新构建需要完整源码目录。已打包版本请不带 -Build 启动。'
        OpeningHint = '打开 {0} 登录，选择新建模拟或真实公司，再把开场任务发送给自己的 Agent。'
    }
} else {
    @{
        DependenciesFailed = 'Frontend dependency installation failed.'
        FrontendFailed = 'Frontend build failed.'
        BackendFailed = 'Application build failed.'
        InitializationFailed = 'Deployment initialization failed.'
        CredentialHint = 'Sign in with the owner token displayed during initialization. A new token is shown only once.'
        SourceRequired = 'Rebuilding requires the complete source directory. Start a packaged release without -Build.'
        OpeningHint = 'Open {0}, sign in, create a simulation or real company, then send its opening brief to your own Agent.'
    }
}
$binaryPath = Join-Path $PSScriptRoot 'target\release\company-server.exe'
$cliPath = Join-Path $PSScriptRoot 'target\release\company.exe'
$portableBinary = Join-Path $PSScriptRoot 'company-server.exe'
if (Test-Path -LiteralPath $portableBinary) {
    $binaryPath = $portableBinary
    $cliPath = Join-Path $PSScriptRoot 'company.exe'
}
$webPath = Join-Path $PSScriptRoot 'web\dist'
if ($Build -or -not (Test-Path -LiteralPath $binaryPath)) {
    if (-not (Test-Path -LiteralPath (Join-Path $PSScriptRoot 'Cargo.toml'))) { throw $messages.SourceRequired }
    & (Join-Path $PSScriptRoot 'scripts\build-company.ps1') -BuildProfile Release -InstallDependencies
}
$serverArgs = @('--data-dir', $DataDir, '--bind', $Bind)
if (Test-Path -LiteralPath (Join-Path $webPath 'index.html')) { $serverArgs += @('--web-dir', $webPath) }
if ($DatabaseUrl) { $serverArgs += @('--database-url', $DatabaseUrl) }
$initArgs = @('--data-dir', $DataDir, '--lang', $Lang)
if ($DatabaseUrl) { $initArgs += @('--database-url', $DatabaseUrl) }
& $cliPath @initArgs init
if ($LASTEXITCODE -ne 0) { throw $messages.InitializationFailed }
Write-Host $messages.CredentialHint
$browserUrl = "http://$Bind"
if ($Bind.StartsWith('0.0.0.0:')) { $browserUrl = 'http://127.0.0.1:' + $Bind.Substring(8) }
Write-Host ([string]::Format($messages.OpeningHint, $browserUrl))
if (-not $NoBrowser) {
    Start-Process $browserUrl -WindowStyle Hidden
}
& $binaryPath @serverArgs
exit $LASTEXITCODE
