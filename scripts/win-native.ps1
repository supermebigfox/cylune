$ErrorActionPreference = "Stop"

$nativeRoot = (Resolve-Path "src-tauri/native/windows").Path
$outputRoot = Join-Path $PWD ".ci-logs/native"
New-Item -ItemType Directory -Force $outputRoot | Out-Null

if ($null -eq (Get-Command cl.exe -ErrorAction SilentlyContinue)) {
  throw "MSVC cl.exe is required for the Windows native release gate."
}

$common = @(
  "/nologo",
  "/std:c++17",
  "/EHsc",
  "/W4",
  "/WX",
  "/DUNICODE",
  "/D_UNICODE",
  "/DNOMINMAX",
  "/I$nativeRoot"
)

function Invoke-NativeTest {
  param(
    [Parameter(Mandatory = $true)][string]$Name,
    [Parameter(Mandatory = $true)][string[]]$Sources,
    [string[]]$Libraries = @(),
    [string[]]$Arguments = @()
  )

  $executable = Join-Path $outputRoot "$Name.exe"
  $log = Join-Path $outputRoot "$Name.log"
  $sourcePaths = $Sources | ForEach-Object { Join-Path $nativeRoot $_ }
  $compileArguments = @($common) + $sourcePaths + @("/Fe:$executable") + $Libraries

  & cl.exe @compileArguments 2>&1 | Tee-Object -FilePath $log
  if ($LASTEXITCODE -ne 0) {
    throw "Native test compilation failed: $Name"
  }

  & $executable @Arguments 2>&1 | Tee-Object -FilePath $log -Append
  if ($LASTEXITCODE -ne 0) {
    throw "Native test execution failed: $Name"
  }
}

Invoke-NativeTest -Name "callback-guard" -Sources @("callback_guard_test.cc")
Invoke-NativeTest -Name "capture-state" -Sources @("capture_state_test.cc")
Invoke-NativeTest -Name "drop-state" -Sources @("drop_state_test.cc")
Invoke-NativeTest -Name "render-state" -Sources @("render_state_test.cc")
Invoke-NativeTest -Name "window-state" -Sources @("window_state_test.cc")
Invoke-NativeTest -Name "animation" -Sources @("animation_test.cc")
Invoke-NativeTest `
  -Name "drop-target" `
  -Sources @("drop_target_test.cc", "drop_target.cpp") `
  -Libraries @("ole32.lib", "shell32.lib", "user32.lib", "uuid.lib")
Invoke-NativeTest `
  -Name "hlsl" `
  -Sources @("hlsl_compile_test.cc") `
  -Libraries @("d3dcompiler.lib") `
  -Arguments @((Join-Path $nativeRoot "BlackHole.hlsl"))
