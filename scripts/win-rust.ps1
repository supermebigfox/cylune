$ErrorActionPreference = "Stop"

$logRoot = Join-Path $PWD ".ci-logs"
New-Item -ItemType Directory -Force $logRoot | Out-Null
$compileLog = Join-Path $logRoot "rust-compile.log"
$importsLog = Join-Path $logRoot "rust-imports.log"
$startupLog = Join-Path $logRoot "rust-startup.log"
$suiteLog = Join-Path $logRoot "rust.log"

npm run test:rust -- --no-run 2>&1 | Tee-Object -FilePath $compileLog
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

$targetRoot = if ([string]::IsNullOrWhiteSpace($env:CARGO_TARGET_DIR)) {
  Join-Path $env:LOCALAPPDATA "CYLUNE\Cache\rust"
} else {
  $env:CARGO_TARGET_DIR
}
$depsRoot = Join-Path $targetRoot "debug\deps"
$testExecutable = Get-ChildItem -LiteralPath $depsRoot -Filter "bambu_pools_lib-*.exe" |
  Sort-Object LastWriteTimeUtc -Descending |
  Select-Object -First 1
if ($null -eq $testExecutable) {
  throw "The compiled Rust library test executable was not found in $depsRoot."
}

& dumpbin.exe /DEPENDENTS $testExecutable.FullName 2>&1 |
  Tee-Object -FilePath $importsLog
if ($LASTEXITCODE -ne 0) { throw "dumpbin /DEPENDENTS failed." }
& dumpbin.exe /IMPORTS $testExecutable.FullName 2>&1 |
  Tee-Object -FilePath $importsLog -Append
if ($LASTEXITCODE -ne 0) { throw "dumpbin /IMPORTS failed." }

$localLibraries = @(Get-ChildItem -LiteralPath $depsRoot -Filter "*.dll")
foreach ($library in $localLibraries) {
  @(
    ""
    "Local DLL: $($library.FullName)"
    "SHA256: $((Get-FileHash -LiteralPath $library.FullName -Algorithm SHA256).Hash)"
  ) | Add-Content -Encoding utf8 $importsLog
  & dumpbin.exe /EXPORTS $library.FullName 2>&1 |
    Tee-Object -FilePath $importsLog -Append
}

& $testExecutable.FullName --list 2>&1 | Tee-Object -FilePath $startupLog
$startupStatus = $LASTEXITCODE
if ($startupStatus -ne 0) {
  Copy-Item -LiteralPath $testExecutable.FullName `
    -Destination (Join-Path $logRoot "failed-rust-test.exe")
  Get-WinEvent -FilterHashtable @{
    LogName = "Application"
    StartTime = (Get-Date).AddMinutes(-15)
  } -ErrorAction SilentlyContinue |
    Select-Object -First 40 TimeCreated, Id, LevelDisplayName, ProviderName, Message |
    Format-List |
    Out-String |
    Add-Content -Encoding utf8 (Join-Path $logRoot "rust-events.log")
  exit $startupStatus
}

npm run test:rust 2>&1 | Tee-Object -FilePath $suiteLog
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
