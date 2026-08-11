# Windows release QA gate

本表是 CYLUNE Windows 预览与正式发布的 required gate。当前记录创建于 macOS 实施宿主，无法执行 Windows 桌面、NSIS、Authenticode、D3D/DXGI 或 Bambu Studio 真机测试，因此下面所有真机行均为 **PENDING**，不是 PASS。任何 PENDING、BLOCKED 或 FAIL 都阻止 Windows 发布签核。

状态只允许 `PENDING`、`PASS`、`FAIL`、`BLOCKED`。执行者必须把“OS build”“GPU/driver”“DPI/topology”“Evidence”中的 `PENDING` 替换为真实值；证据路径必须指向同一次 run 的日志、截图、录屏、事件日志、输入 fixture 或哈希文件。不得复用另一台机器的元数据，也不得根据静态检查推断真机 PASS。

## Release identity

| Field | Value |
| --- | --- |
| Commit/tag | PENDING |
| CI run URL | PENDING |
| `CYLUNE-Setup.exe` SHA-256 | PENDING — must be produced on Windows |
| Authenticode signer/status/timestamp | PENDING — required for formal tag; preview must explicitly record Unsigned |
| Reviewer/date | PENDING |
| Mac seal reference | `d640d92` |

## OS, GPU, DPI, and display coverage

每行执行主窗口启动、托盘出现、黑洞显示/隐藏、移动与安全退出的 smoke test；“Discrete”必须填写真实 AMD 或 NVIDIA 型号。

| ID | Target | Required smoke | OS build | GPU/driver | DPI/topology | Status | Evidence |
| --- | --- | --- | --- | --- | --- | --- | --- |
| ENV-01 | Windows 10 22H2 x64 / Intel | Single display | PENDING | Intel integrated / PENDING | 100% | PENDING | PENDING |
| ENV-02 | Windows 10 22H2 x64 / Intel | Single display | PENDING | Intel integrated / PENDING | 125% | PENDING | PENDING |
| ENV-03 | Windows 10 22H2 x64 / Intel | Single display | PENDING | Intel integrated / PENDING | 150% | PENDING | PENDING |
| ENV-04 | Windows 10 22H2 x64 / Intel | Single display | PENDING | Intel integrated / PENDING | 200% | PENDING | PENDING |
| ENV-05 | Windows 10 22H2 x64 / Discrete | Single display | PENDING | AMD or NVIDIA / PENDING | 100% | PENDING | PENDING |
| ENV-06 | Windows 10 22H2 x64 / Discrete | Single display | PENDING | AMD or NVIDIA / PENDING | 125% | PENDING | PENDING |
| ENV-07 | Windows 10 22H2 x64 / Discrete | Single display | PENDING | AMD or NVIDIA / PENDING | 150% | PENDING | PENDING |
| ENV-08 | Windows 10 22H2 x64 / Discrete | Single display | PENDING | AMD or NVIDIA / PENDING | 200% | PENDING | PENDING |
| ENV-09 | Windows 11 x64 / Intel | Single display | PENDING | Intel integrated / PENDING | 100% | PENDING | PENDING |
| ENV-10 | Windows 11 x64 / Intel | Single display | PENDING | Intel integrated / PENDING | 125% | PENDING | PENDING |
| ENV-11 | Windows 11 x64 / Intel | Single display | PENDING | Intel integrated / PENDING | 150% | PENDING | PENDING |
| ENV-12 | Windows 11 x64 / Intel | Single display | PENDING | Intel integrated / PENDING | 200% | PENDING | PENDING |
| ENV-13 | Windows 11 x64 / Discrete | Single display | PENDING | AMD or NVIDIA / PENDING | 100% | PENDING | PENDING |
| ENV-14 | Windows 11 x64 / Discrete | Single display | PENDING | AMD or NVIDIA / PENDING | 125% | PENDING | PENDING |
| ENV-15 | Windows 11 x64 / Discrete | Single display | PENDING | AMD or NVIDIA / PENDING | 150% | PENDING | PENDING |
| ENV-16 | Windows 11 x64 / Discrete | Single display | PENDING | AMD or NVIDIA / PENDING | 200% | PENDING | PENDING |
| ENV-17 | Windows 10 22H2 x64 | Mixed-DPI dual display, negative origin, cross-display drag | PENDING | PENDING | 100% + 150% mixed | PENDING | PENDING |
| ENV-18 | Windows 11 x64 | Mixed-DPI dual display, cross-adapter if available | PENDING | PENDING | 125% + 200% mixed | PENDING | PENDING |

## Installer, update, lifecycle, and data

至少在一台 Windows 10 Intel 与一台 Windows 11 discrete 机器执行完整流程；其余环境执行相关 smoke。升级必须从前一预览版本保留真实 fixture 数据，卸载必须区分程序文件与用户数据。

| ID | Scenario / expected result | OS build | GPU/driver | DPI/topology | Status | Evidence |
| --- | --- | --- | --- | --- | --- | --- |
| PKG-01 | CI has exactly one `CYLUNE-Setup.exe`; SHA-256 recorded | PENDING | PENDING | PENDING | PENDING | PENDING |
| PKG-02 | Preview is explicitly Unsigned; formal tag has valid Authenticode signer and timestamp | PENDING | PENDING | PENDING | PENDING | PENDING |
| PKG-03 | SimpChinese and English selector; current-user install succeeds without elevation | PENDING | PENDING | PENDING | PENDING | PENDING |
| PKG-04 | Missing WebView2 invokes downloaded bootstrapper; existing runtime is not damaged | PENDING | PENDING | PENDING | PENDING | PENDING |
| PKG-05 | Clean first launch creates only expected current-user app data | PENDING | PENDING | PENDING | PENDING | PENDING |
| PKG-06 | Update from previous build preserves inventory, jobs, media, settings, and identifier | PENDING | PENDING | PENDING | PENDING | PENDING |
| PKG-07 | Downgrade behavior is recorded and does not silently corrupt the database | PENDING | PENDING | PENDING | PENDING | PENDING |
| PKG-08 | Uninstall removes app/shortcuts/processes; user-data retention is recorded | PENDING | PENDING | PENDING | PENDING | PENDING |
| LIFE-01 | Second launch activates/uses the single existing instance | PENDING | PENDING | PENDING | PENDING | PENDING |
| LIFE-02 | Close main window leaves intended tray/background behavior; tray reopen works | PENDING | PENDING | PENDING | PENDING | PENDING |
| LIFE-03 | Tray open, black-hole toggle, hide/show, reset, and exit actions work | PENDING | PENDING | PENDING | PENDING | PENDING |
| LIFE-04 | Reboot/login persistence matches saved enabled/visible state without duplicate process | PENDING | PENDING | PENDING | PENDING | PENDING |
| LIFE-05 | Sleep/wake rebuilds capture/render state and does not retain stale frame | PENDING | PENDING | PENDING | PENDING | PENDING |
| LIFE-06 | Lock/unlock resumes or explicitly degrades without crash/stale capture | PENDING | PENDING | PENDING | PENDING | PENDING |
| LIFE-07 | Display unplug/replug converges to a valid monitor and bounded resource ownership | PENDING | PENDING | PENDING | PENDING | PENDING |
| LIFE-08 | Primary-display switch preserves logical position, DPI, hit testing, and live capture | PENDING | PENDING | PENDING | PENDING | PENDING |

## Bambu Studio background slicing

Use a released Windows Bambu Studio plus a separately recorded manually selected `BambuStudio.exe`. Evidence includes the input fixture checksum, discovered executable/profile paths, command/log, extracted plate values, cancellation timing, child-process state, and temporary-directory cleanup. Never commit user output or `result.json`.

| ID | Scenario / expected result | OS build | GPU/driver | DPI/topology | Status | Evidence |
| --- | --- | --- | --- | --- | --- | --- |
| SLC-01 | Installed Bambu Studio discovery and validated profile layout | PENDING | PENDING | PENDING | PENDING | PENDING |
| SLC-02 | Manual executable selection takes priority and persists safely | PENDING | PENDING | PENDING | PENDING | PENDING |
| SLC-03 | Single-plate Bambu 3MF slices in background; values match Studio | PENDING | PENDING | PENDING | PENDING | PENDING |
| SLC-04 | Multi-plate Bambu 3MF preserves plate/material selections and values | PENDING | PENDING | PENDING | PENDING | PENDING |
| SLC-05 | Cancellation terminates the child, reports cancelled, and removes private temp output | PENDING | PENDING | PENDING | PENDING | PENDING |
| SLC-06 | Invalid executable/profile and failed slicing are actionable and preserve source/inventory | PENDING | PENDING | PENDING | PENDING | PENDING |

## Desktop black-hole and visual parity

Run this section together with `docs/qa-windows-black-hole.md`. Paired clips are 3840×2160, 60 FPS, same size/style/brightness/source timing, and compared with the approved `d640d92` Mac recording. Record the capture tool, codec, exact source/checkerboard asset, motion path and Finder-equivalent Explorer icon scene. The missing Windows file-icon spiral overlay is a known difference; all shared uniforms, timings and live-background behavior remain gates.

| ID | Scenario / rejection boundary | OS build | GPU/driver | DPI/topology | Status | Evidence |
| --- | --- | --- | --- | --- | --- | --- |
| VIS-01 | Dark navy idle + hover; clockwise flow, dynamic center, unchanged diameter | PENDING | PENDING | PENDING | PENDING | PENDING |
| VIS-02 | White scrolling browser; live distortion, no circular lens edge/frozen frame | PENDING | PENDING | PENDING | PENDING | PENDING |
| VIS-03 | Pinned moving checkerboard; no rectangular boundary or discontinuity | PENDING | PENDING | PENDING | PENDING | PENDING |
| VIS-04 | Pinned Explorer icon scene equivalent to approved Finder scene | PENDING | PENDING | PENDING | PENDING | PENDING |
| VIS-05 | Mixed-DPI multi-monitor drag; current monitor/crop/rotation stay correct | PENDING | PENDING | PENDING | PENDING | PENDING |
| VIS-06 | Supported ingest: 0.74 s swallow, live center/background, unchanged diameter | PENDING | PENDING | PENDING | PENDING | PENDING |
| VIS-07 | Unsupported eject starts after swallow and lasts 0.62 s; source untouched | PENDING | PENDING | PENDING | PENDING | PENDING |
| VIS-08 | Success jet lasts 0.50 s with correct alignment/fade and live background | PENDING | PENDING | PENDING | PENDING | PENDING |
| VIS-09 | Moving black hole over Explorer files never imports by proximity | PENDING | PENDING | PENDING | PENDING | PENDING |
| VIS-10 | Hide/show and enable/disable preserve state without stale frames | PENDING | PENDING | PENDING | PENDING | PENDING |
| VIS-11 | Size presets/range 120–900 do not change during hover or animation | PENDING | PENDING | PENDING | PENDING | PENDING |
| VIS-12 | Auto/30/60 FPS remain responsive; high-refresh display pacing recorded | PENDING | PENDING | PENDING | PENDING | PENDING |
| VIS-13 | WDA exclusion prevents self-capture; unsupported path visibly degrades | PENDING | PENDING | PENDING | PENDING | PENDING |
| VIS-14 | GPU/device-loss recovery and bounded shutdown show no leak/hung process | PENDING | PENDING | PENDING | PENDING | PENDING |

## Automated and final sign-off

Attach fresh logs for the exact release commit:

```powershell
npm ci
npm test -- --run
npm run test:rust
npm run check:mac-seal
npm run tauri build -- --bundles nsis
npm run release:windows
Get-AuthenticodeSignature .\发布-Windows\CYLUNE-Setup.exe
Get-FileHash .\发布-Windows\CYLUNE-Setup.exe -Algorithm SHA256
```

| Gate | Status | Evidence |
| --- | --- | --- |
| Full-history `d640d92` → HEAD/index/worktree/untracked/ignored Mac seal | PENDING | PENDING |
| Frontend/release tests | PENDING | PENDING |
| Rust and Windows native tests on Windows | PENDING | PENDING |
| NSIS install/update/uninstall matrix | PENDING | PENDING |
| Bambu real-slice matrix | PENDING | PENDING |
| Paired black-hole perceptual matrix | PENDING | PENDING |
| Setup signature and SHA-256 | PENDING | PENDING |
| Final reviewer sign-off | PENDING | PENDING |

Release decision: **BLOCKED while any row remains PENDING, BLOCKED, or FAIL.**
