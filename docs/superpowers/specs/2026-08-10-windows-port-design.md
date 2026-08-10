# CYLUNE Windows 移植设计规格

## 1. 目标

在不改变已封板 macOS 版本外观、功能和黑洞表现的前提下，为 CYLUNE 增加 Windows 10 22H2 与 Windows 11 的原生版本。Windows 版需要覆盖主程序、耗材库、打印记录、3MF/G-code 解析、Bambu Studio 后台切片、系统托盘、单实例、文件拖放，以及与 macOS 版感知一致的桌面黑洞。

“一致”指造型、颜色、运动方向、动画节奏、吸入力度、交互结果和设置项一致；不要求 Metal 与 Direct3D 输出逐像素或逐比特相同。

## 2. 封板约束

- `src-tauri/native/mac/**` 为只读基线，不修改任何 Objective-C、C++、Metal 着色器、参数或资源。
- macOS 的 `.app`、`.dmg` 构建配置和现有运行行为不得改变。
- Windows 代码在 `src-tauri/native/windows/**` 中独立实现。
- 共用 Rust/React 代码只允许增加平台分发、能力描述和 Windows 路径处理；任何共用改动都必须通过现有 macOS 回归测试。
- Windows 开发位于 `/Users/robin/Desktop/耗材管理-Windows` 和 `codex/windows-port` 分支；不得覆盖 `/Users/robin/Desktop/耗材管理`。
- 不把构建缓存、临时切片输出、录屏素材或用户的 `result.json` 放入 Git。

## 3. 支持范围

### 3.1 首发范围

- Windows 10 22H2 x64。
- Windows 11 x64。
- 60 Hz 与高刷新率显示器。
- 单显示器和多显示器，包括不同 DPI、负坐标和显示器热插拔。
- Bambu Studio Windows 正式安装版，以及用户手动选择的 `BambuStudio.exe`。
- NSIS `.exe` 安装包；在发布链稳定后同时提供 `.msi`。

### 3.2 暂不纳入首发

- Windows on ARM 原生构建。
- Windows 10 22H2 以前版本。
- 远程桌面、虚拟机和受保护视频内容上的完整背景扭曲保证。
- 修改 macOS 黑洞来共用同一个跨平台着色器。

## 4. 总体架构

保留现有 React/Tauri 主程序和 Rust 业务服务。`PetNativeConfig`、回调编号、文件导入结果和动画信号继续作为稳定平台 ABI；macOS 后端保持原样，Windows 后端在相同 ABI 下提供实现。

Windows 后端分为五个单一职责单元：

1. `window`：创建透明、无边框、不抢焦点的 Win32 桌面窗口，处理移动、命中测试、DPI、显示器变化和可见性。
2. `capture`：用 DXGI Desktop Duplication 获取黑洞周围的实时桌面 GPU 纹理，处理设备丢失、锁屏、休眠和显示器切换。
3. `renderer`：用 Direct3D 11 与 DirectComposition 合成透明窗口，用 HLSL 复现现有 Metal 黑洞。
4. `drop_target`：用 Windows OLE `IDropTarget` 接收拖到黑洞上的文件，并保持“移动黑洞碰到桌面文件不会导入”的规则。
5. `pet_bridge`：导出与 macOS 后端相同的 C ABI，连接 Rust 运行时、设置、状态和动画事件。

选择 Direct3D 11 是因为桌面复制 API 原生返回 DXGI/D3D 纹理，首发实现的资源共享和兼容性更直接；此处不为黑洞引入浏览器 WebGL 或新的大体积跨平台渲染依赖。

## 5. 黑洞视觉迁移

### 5.1 着色器

将 `BlackHole.metal` 中已确认的数学公式、常量、时间曲线和颜色参数逐段移植为 `BlackHole.hlsl`，不重新设计外观。以下行为必须保持：

- 顺时针旋转并向中心吸入。
- 非规整、动态变化的面状空间扭曲。
- 动态中心亮光与可见的彩色吸积盘。
- 文件靠近时提高转速和吸入力度，但不改变黑洞尺寸。
- 支持文件吸入、拒绝吐出、成功射流和成功结算信号。
- 支持自动、30 FPS、60 FPS，以及现有尺寸范围。
- `prefers-reduced-motion` 对应的降动效行为不变。

### 5.2 实时背景

Windows 后端逐帧捕获黑洞所在显示器，并只裁剪渲染所需区域。捕获纹理直接留在 GPU 中，禁止以定时截图或 CPU 图片替代实时画面。

黑洞顶层窗口使用 `WDA_EXCLUDEFROMCAPTURE` 请求从系统捕获结果排除，避免捕获自身造成递归、延迟残影或黑边。若系统、显卡驱动或受保护内容不支持可靠排除，运行时进入明确的降级状态：保留黑洞粒子与吸积动画，但关闭背景采样扭曲，并在设置页显示原因；不得停在一帧旧画面。

### 5.3 视觉验收

在相同深色、浅色、浏览器、Finder/Explorer 风格背景上录制 macOS 与 Windows 对照素材。验收关注：轮廓消隐、旋转方向、吸入力度、中心亮光、粒子密度、拖动跟手性、吞噬/吐出/射流节奏。允许不同 GPU 和色彩管理造成轻微像素差异，不允许出现固定圆形透镜边界、单帧扭曲、拖动延迟黑边或漂浮异物。

## 6. Windows 交互

- 黑洞窗口默认不激活主程序、不出现在任务栏和 Alt+Tab 中，能够跨虚拟桌面显示。
- 用户在黑洞本体上按下并拖动时移动黑洞；松手后持久化逻辑坐标和显示器身份。
- 透明区域尽量鼠标穿透，黑洞可交互区域保持可拖动和可接收文件。
- 拖入任意文件都播放吸入：支持的 `.3mf`、`.gcode`、`.gcode.3mf` 进入现有导入/切片流程；其他文件播放吐出并返回拒绝结果。
- 多文件一次拖入时按照现有队列逐个处理，不改变任务数据库语义。
- 托盘提供打开主程序、开启/关闭黑洞、隐藏/显示黑洞和退出；用户退出后保留启用状态。

## 7. Bambu Studio 与切片

把安装发现从 macOS `.app` 结构中抽离为平台策略：

- macOS 策略保持现有路径和验证逻辑。
- Windows 策略依次检查用户手动选择、注册表卸载信息、标准 Program Files 位置和可执行文件所在资源目录。
- 只接受真实普通文件和真实 profiles 目录；失败时给出可操作错误，不静默套默认工艺。
- 启动 `BambuStudio.exe` 的后台 CLI 行为与 macOS 一致，不打开 Studio 主界面。
- 继续完整读取 3MF 内的打印机、打印板、耗材和自定义工艺；只在用户确认后覆盖目标打印机。
- 切片产物继续位于私有临时目录，提取重量、时间、层数、缩略图和多盘信息后清理。
- 支持取消切片，并确保子进程和临时目录都被终止、回收。

## 8. 构建与发布

- Tauri 通用配置保留产品名 `CYLUNE`、标识符 `com.robin.cylune` 和现有图标。
- Windows 平台配置单独声明 NSIS 目标、安装图标、卸载信息和 WebView2 策略。
- Windows 原生代码只在 `target_os = "windows"` 时编译并链接 `d3d11`、`dxgi`、`dcomp`、`dwmapi`、`user32`、`ole32` 等系统库。
- macOS 构建仍只编译 `native/mac`，不得因为 Windows 依赖改变链接结果。
- 正式分发前配置 Windows 代码签名；未签名测试包必须明确标记为测试版。

## 9. 错误处理

- 捕获设备丢失、显示模式变化和睡眠唤醒后自动重建 D3D 设备与捕获会话。
- 捕获暂时无帧时继续动画但不重复使用过期桌面纹理超过一个短帧窗口。
- GPU 初始化失败时主程序仍可运行，黑洞状态显示为不可用，并允许用户重试。
- 拖放、切片或导入失败继续复用现有业务错误与吐出动画，不因原生窗口故障重复扣耗材。
- 程序退出时有界等待捕获线程和渲染线程，超时后记录错误并退出，不长期挂后台进程。

## 10. 测试策略

### 自动测试

- 保留全部 React、Rust 和 macOS 原生测试。
- 增加平台选择、ABI、Windows 路径发现、DPI 坐标、拖放状态机、捕获降级和设备重建测试。
- 用可注入接口测试窗口、捕获和渲染生命周期，不依赖真实桌面即可覆盖错误状态。
- Windows CI 至少执行 TypeScript 构建、Rust 测试、C++ 单元测试、HLSL 编译和 Tauri debug bundle。
- macOS CI 在合并前执行现有完整测试和打包烟雾测试，证明封板版本没有回归。

### Windows 真机验收

- Intel 与 AMD/NVIDIA 至少各一台可用配置。
- 100%、125%、150%、200% DPI，以及两个不同 DPI 的显示器。
- 睡眠/唤醒、锁屏/解锁、切换主显示器、拔插显示器和 GPU 设备重建。
- 深色与浅色背景、浏览器、资源管理器、视频窗口和高刷新率。
- 文件靠近、吸入、拒绝吐出、成功射流、移动黑洞不误导入、退出后单实例。
- 单盘与多盘 3MF 的切片值与 Bambu Studio 对照。

## 11. 实施阶段

1. 平台骨架：Windows 构建配置、原生 ABI、主程序、数据库、托盘和安装发现。
2. 切片闭环：Windows Bambu Studio 发现、后台子进程、取消与临时文件清理。
3. 黑洞窗口：透明窗口、多屏/DPI、拖动和 OLE 文件拖放。
4. 实时渲染：桌面捕获、DirectComposition、HLSL 黑洞和自排除。
5. 动画对齐：吸入、吐出、射流、成功信号、FPS 和尺寸。
6. 真机验收与发布：GPU/DPI 矩阵、安装/卸载、签名和最终安装包。

每个阶段必须形成独立可测试的提交。阶段 1 至 3 可以在 macOS 上完成平台无关测试和静态准备，但阶段 4 至 6 的“完成”只能由 Windows 真机运行结果确认。

## 12. 完成标准

- Windows 安装后无需开发环境即可运行主程序。
- 主程序核心功能与 macOS 版数据语义一致。
- Bambu Studio 后台切片不打开主界面，数值和用户 3MF 设置保持一致。
- 黑洞没有固定透镜边界、单帧背景、拖动黑边或位置限制问题。
- 黑洞所有已确认动画和拖放规则在 Windows 上通过真机验收。
- Mac 封板目录和黑洞资源没有改动，macOS 全量回归通过。
- 产出可安装的 Windows 测试包，完成签名后产出正式发布包。

## 13. 技术依据

- Microsoft Desktop Duplication API：<https://learn.microsoft.com/en-us/windows/win32/direct3ddxgi/desktop-dup-api>
- Microsoft DirectComposition：<https://learn.microsoft.com/en-us/windows/win32/directcomp/initialize-directcomposition>
- `SetWindowDisplayAffinity` / `WDA_EXCLUDEFROMCAPTURE`：<https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setwindowdisplayaffinity>
