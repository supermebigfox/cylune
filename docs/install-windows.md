# CYLUNE Windows 安装与发布

## 支持范围

- Windows 10 22H2 x64 与 Windows 11 x64。
- 当前首发格式为 NSIS `CYLUNE-Setup.exe`；不提供 MSI 或 Windows on ARM 原生包。
- 安装范围为当前用户，不需要管理员权限。安装器提供简体中文与 English，并在启动时显示语言选择器。
- 安装器在系统缺少 Microsoft Edge WebView2 Runtime 时下载官方 bootstrapper，因此首次安装可能需要网络；CYLUNE 的库存、解析与切片业务本身保持本地运行。

## 预览版与正式版

普通分支和拉取请求的 CI 产物是未签名预览版，只可用于内部 QA。Windows 可能显示 SmartScreen 警告；不要把未签名预览包描述为正式发布，也不要要求用户绕过组织安全策略。

只有 `v*` tag 的 CI 会读取 GitHub Actions secrets 中的证书与密码，在 runner 临时目录导入证书并重建签名包。证书、密码、PFX 和临时签名配置不得写入仓库或上传为 artifact；正式发布前必须在 QA 记录中保存 Authenticode 检查与 SHA-256 证据。

```powershell
Get-AuthenticodeSignature .\CYLUNE-Setup.exe | Format-List Status,StatusMessage,SignerCertificate
Get-FileHash .\CYLUNE-Setup.exe -Algorithm SHA256
```

摘要必须与同一次 CI run 的发布记录一致。本仓库不预填或伪造尚未在 Windows runner 生成的摘要。

## 安装或升级

1. 在旧版本托盘菜单中选择退出，并在任务管理器确认没有 `CYLUNE.exe` 或仍在运行的 Bambu Studio 后台切片子进程。
2. 从对应 CI run 或正式 release 下载唯一的 `CYLUNE-Setup.exe`，先验证签名状态与 SHA-256。
3. 运行安装器，选择简体中文或 English，保持“当前用户”安装范围。
4. 若需要 WebView2，允许安装器完成 Microsoft bootstrapper 下载；离线环境应先由管理员部署受支持的 WebView2 Runtime。
5. 升级时运行更高版本的同一安装器。不得通过复制应用目录覆盖正在运行的版本，也不要并行运行预览版和正式版。
6. 首次启动后确认主窗口、托盘、单实例、桌面黑洞开关和 Bambu Studio 路径；全新安装默认不应读取未由用户选择的打印文件。

升级前从设置页导出业务 JSON 备份。备份包含库存、槽位、打印任务、映射和结算，不包含源 3MF/G-code、切片临时目录、屏幕帧、凭据或本机黑洞位置设置。

## 卸载与用户数据

从“设置 → 应用 → 已安装的应用”卸载 CYLUNE。卸载后必须在发布 QA 中确认应用文件、开始菜单入口、托盘进程与开机持久化项已清理，并单独记录用户数据是否保留。

业务数据库 `inventory.sqlite` 与媒体位于 Tauri 的当前用户应用数据目录（通常在 `%APPDATA%\com.robin.cylune`）。如需彻底删除数据，先退出应用并导出备份，再由用户明确删除该目录；安装器或卸载器不得静默删除未备份的业务数据。

## Bambu Studio 与桌面黑洞

- 后台切片优先使用用户手动选择的 `BambuStudio.exe`，其次检查 Windows 卸载信息和标准安装位置。发现失败时应提示用户选择真实可执行文件，不能套用默认工艺。
- 单盘、多盘、取消与临时文件清理必须在真实 Windows/Bambu Studio 环境通过；未运行该矩阵前不得声称 Windows 切片发布就绪。
- 桌面黑洞使用 Direct3D 11、DXGI Desktop Duplication 与 DirectComposition。锁屏、休眠、显示器拔插或 GPU 重建设备时允许进入明确降级，但不得保留过期桌面帧。
- Windows 当前没有 macOS 文件图标螺旋覆盖层；这是已记录的平台差异，不得通过修改封板 Mac 实现来“对齐”。

## 从源码构建预览包

在 Windows 10/11 x64 的 PowerShell 中运行：

```powershell
npm ci
npm test -- --run
npm run test:rust
npm run check:mac-seal
npm run release:windows
Get-AuthenticodeSignature .\发布-Windows\CYLUNE-Setup.exe
Get-FileHash .\发布-Windows\CYLUNE-Setup.exe -Algorithm SHA256
```

Rust/Tauri 构建缓存位于 `%LOCALAPPDATA%\CYLUNE\Cache\rust`。`npm run release:windows` 是 fail-fast 的完整本地发布入口：它只执行一次 Tauri NSIS build，且只有该次构建退出码为 0 时才继续调用 publish-only 步骤。内部的 `npm run publish:windows` 仅发布已经成功生成的产物，不应代替完整发布入口单独使用。发布器从 `release\bundle\nsis` 接受唯一的普通 `*-setup.exe`，并以 no-clobber 方式发布到 `发布-Windows\CYLUNE-Setup.exe`。若目标已存在，应先人工核对并移走旧产物；发布命令不会覆盖它。

上述命令在 macOS 上不能生成或验证真实 Windows 安装包、签名或 SHA-256 发布证据；请使用 `docs/qa-windows-release.md` 完成 required gate。
