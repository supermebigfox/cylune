# 耗材管理 · macOS 本地版

这是一个完全离线的 Mac 应用。运行已构建的 `.app` 不需要 Docker、Java、浏览器插件或树莓派。

## 系统要求

- Task 9 当前交付只包含 `aarch64`，适用于 Apple Silicon 和 macOS 11.0 或更高版本；本次 DMG 不包含 Intel/x86_64 或 universal 二进制。源码与 bundle 配置中的 macOS 10.15 只为未来 Intel/x86_64 构建保留，不能理解为当前 arm64 DMG 可运行在 macOS 10.15。
- 黑洞的实时扭曲使用 ScreenCaptureKit，仅在 macOS 12.3 或更高版本可用；旧系统开启黑洞后会自动使用兼容背景。
- 从源码构建需要 Xcode Command Line Tools、Node.js 20+ 与 Rust stable；普通用户运行 `.app` 或从 `.dmg` 安装时不需要这些开发工具。

## 安装或升级

1. 在旧版本的菜单栏菜单中选择“退出”，并确认活动监视器中没有仍在运行的“CYLUNE”。
2. 若曾从临时 `.dmg` 直接运行旧版本，先退出应用并在 Finder 中推出该磁盘映像。
3. 打开新的 `.dmg`，将“CYLUNE”拖到“应用程序”；不要同时运行下载目录、旧 `.dmg` 和“应用程序”里的多个副本。
4. 首次打开未公证的本地构建时，可在 Finder 中按住 Control 点击应用并选择“打开”，或在“系统设置 → 隐私与安全性”中确认打开。

本项目当前没有声称完成 Apple Developer ID 签名或公证。构建产物使用本机临时签名，只适合本机测试。

## 屏幕录制权限

黑洞只截取附近的一小块桌面来生成局部光学扭曲。全新安装默认关闭黑洞；用户第一次在设置中选择“开启黑洞”时，macOS 可能请求屏幕录制权限：

1. 打开“系统设置 → 隐私与安全性 → 屏幕录制”。
2. 为“CYLUNE”打开权限。
3. 若应用显示“需要重新启动”，从菜单栏彻底退出后只启动“应用程序”中的一个副本。

在较旧的 macOS 中，此入口位于“系统偏好设置 → 安全性与隐私 → 隐私 → 屏幕录制”。拒绝、撤销权限或捕获/Metal 初始化失败时，导入和结算仍可使用，黑洞会改用兼容背景。

如果不想授予权限或希望停止 GPU 消耗，可在主窗口“设置 → 桌面黑洞”中选择“关闭黑洞”。开启或关闭状态会在退出后保留；开启时同一处可调整尺寸、帧率、显示/隐藏和重置位置。

## 隐私边界

- 捕获明确排除本应用，不采集音频、麦克风或鼠标光标。
- 屏幕帧只在内存与 GPU 中用于即时渲染，不保存为文件、不写入 SQLite、不进入 JavaScript、不记录到日志，也不上传网络。
- 应用只读取用户主动拖入黑洞或通过文件选择/监测文件夹明确授权的打印文件。移动黑洞经过 Finder 文件不会读取或导入该文件。
- 每次拖放只处理第一个受支持的普通文件；最终仍由本地 `PrintService` 验证 `.gcode.3mf`、含 G-code 的 `.3mf` 或受限 `.gcode`。

## 使用

- 主窗口用于耗材库、AMS Lite 四槽、任务映射和成功/失败/取消结算。关闭主窗口只会隐藏它；菜单栏图标与桌面黑洞仍可继续运行。
- 将 Bambu Studio 导出的 `.gcode.3mf` 或已切片 `.3mf` 拖入桌面黑洞即可创建待确认任务。导入本身不会扣减耗材，只有明确结算才会修改余额。
- 短按黑洞：有待处理任务时打开最新任务；没有待处理任务时打开主窗口。右键菜单可打开主程序、重置位置、隐藏黑洞或退出。
- 独立 `.gcode` 通常缺少耗材密度与直径配置。应用会明确拒绝并保持库存不变，请优先导出 `.gcode.3mf`。
- 设置页可以选择一个非递归监测文件夹；应用只监测这个用户明确选择的文件夹。

本地数据库默认位于 macOS 的应用数据目录（通常为 `~/Library/Application Support/com.local.bambuspools/inventory.sqlite`）。删除或覆盖数据库前请先退出应用并导出备份。

## 备份边界

业务 JSON 备份包含库存、槽位、解析结果、任务、映射、结算和不可变流水，不包含源 3MF/G-code、模型、缩略图、凭据或屏幕帧。桌面黑洞的 `pet_*` 模式、尺寸、帧率、可见性、坐标和显示器编号是本机设备设置，也不会进入业务备份；在另一台设备恢复业务数据不会移动或改写那台设备的黑洞设置。

恢复业务备份前，应用会在所选备份旁保存当前业务数据。业务备份与设备设置应分别看待，不要通过复制正在使用的 SQLite 文件在两台设备间同步。

## 从源码运行

进入项目根目录后：

```bash
npm install
npm run tauri dev
```

构建 `.app` 和 `.dmg`：

```bash
npm run tauri build
```

构建成功后：

- macOS 的 Rust 编译缓存位于 `$HOME/Library/Caches/CYLUNE/rust`，不会再堆积在项目文件夹中；
- `.app` 位于 `$HOME/Library/Caches/CYLUNE/rust/release/bundle/macos/CYLUNE.app`；
- `.dmg` 位于 `$HOME/Library/Caches/CYLUNE/rust/release/bundle/dmg/CYLUNE_0.1.0_aarch64.dmg`。

若本机的 Finder AppleScript 长时间停在 `Running bundle_dmg.sh`，先确认 release `.app` 已成功生成并中止卡住的 DMG 美化步骤，再使用同一个 Tauri 生成脚本的无 GUI 模式：

```bash
cd "$HOME/Library/Caches/CYLUNE/rust/release/bundle/macos"
../dmg/bundle_dmg.sh \
  --skip-jenkins \
  --volname 'CYLUNE' \
  --icon 'CYLUNE.app' 180 170 \
  --app-drop-link 480 170 \
  --window-size 660 400 \
  --hide-extension 'CYLUNE.app' \
  --volicon '../dmg/icon.icns' \
  '../dmg/CYLUNE_0.1.0_aarch64.dmg' \
  'CYLUNE.app'
```

若目标 DMG 已存在，应先移走旧产物再执行。`--skip-jenkins` 只跳过 Finder 的图标位置/背景美化；应用内容、`Applications` 链接、卷图标、压缩和校验仍会生成。

## 迁移到树莓派

G-code/3MF 解析、结算和 SQLite 账本是 Rust 核心逻辑，可以移植到 Raspberry Pi Zero 2 W。macOS 的 Tauri 窗口、菜单栏、ScreenCaptureKit 与 Metal 桌宠不能原样运行在树莓派上；树莓派版本需要另做 Linux 服务或网页界面，再导入经过校验的 JSON 数据。不要直接在两台设备同时写同一个 SQLite 文件。
