param(
    [ValidateSet('demo', 'simulation')][string]$Example = 'simulation',
    [string]$DataDir = '',
    [string]$Binary = '',
    [string]$DatabaseUrl = '',
    [ValidateSet('zh-CN', 'en')][string]$Lang = 'zh-CN'
)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
if (-not $DataDir) {
    if ($Lang -eq 'en') { throw 'Choose an independent example data directory explicitly.' }
    throw '请显式选择用于示例的独立数据目录。'
}
if (-not $Binary) {
    $packagedBinary = Join-Path $projectRoot 'company.exe'
    $Binary = if (Test-Path -LiteralPath $packagedBinary) { $packagedBinary } else { Join-Path $projectRoot 'target\release\company.exe' }
}
if (-not (Test-Path -LiteralPath $Binary)) {
    if ($Lang -eq 'en') { throw 'Build the application first or specify the company executable with -Binary.' }
    throw '请先构建应用，或通过 -Binary 指定 company 可执行文件。'
}
$commonArgs = @('--data-dir', $DataDir, '--lang', $Lang)
if ($DatabaseUrl) { $commonArgs += @('--database-url', $DatabaseUrl) }
$companyId = if ($Example -eq 'demo') { 'demo-company' } else { 'simulation-company' }
$exampleSuffix = if ($Lang -eq 'en' -and $Example -eq 'simulation') { '.en' } else { '' }
$companyInput = Join-Path $projectRoot "examples\$Example-company$exampleSuffix.json"
$operationInput = Join-Path $projectRoot "examples\$Example-operations$exampleSuffix.json"
& $Binary @commonArgs apply --operation company.create --input $companyInput --request-id "create-$companyId"
if ($LASTEXITCODE -ne 0) {
    if ($Lang -eq 'en') { throw 'Example company creation failed.' }
    throw '示例公司建立失败。'
}
& $Binary @commonArgs import --company $companyId --file $operationInput
if ($LASTEXITCODE -ne 0) {
    if ($Lang -eq 'en') { throw 'Example import failed.' }
    throw '示例导入失败。'
}
& $Binary @commonArgs metrics --company $companyId --as-of 2026-10-11
exit $LASTEXITCODE
