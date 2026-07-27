# 耗材管理 · macOS 本地版

这是一个完全离线的 Mac 应用。运行已经构建好的 `.app` 不需要 Docker、Java、浏览器插件或树莓派。

## 从源码运行

准备 macOS 10.15 或更高版本、Xcode Command Line Tools、Node.js 20+ 与 Rust stable。进入项目根目录后运行：

```bash
npm install
npm run tauri dev
```

构建应用：

```bash
npm run tauri build -- --bundles app
```

应用位于 `src-tauri/target/release/bundle/macos/拓竹耗材管家.app`。若本机未对开发版签名，首次打开时可在 Finder 中按住 Control 点击应用并选择“打开”，或在“系统设置 → 隐私与安全性”中确认打开。此项目没有声称完成开发者签名或 Apple 公证。

## 使用

- 主窗口用于耗材库、AMS Lite 四槽、任务映射和成功/失败/取消结算。关闭主窗口只会隐藏它；菜单栏图标仍在运行。
- 点击菜单栏图标会弹出快捷拖放窗口。推荐导入 Bambu Studio 导出的 `.gcode.3mf`；已切片 `.3mf` 同样可读取。
- 独立 `.gcode` 可以拖入，但它通常缺少耗材密度与直径配置。应用会明确拒绝并保持库存不变，请改为导出 `.gcode.3mf`。
- 设置页可以选择一个非递归监测文件夹。应用只监测这个用户明确选择的文件夹。
- 设置页可导出或恢复 JSON 备份。备份包含库存、槽位、解析结果、任务、映射、结算和不可变流水，但不包含源 3MF/G-code、模型、缩略图或凭据。恢复前会在所选备份旁自动保存当前数据。

本地数据库默认位于 macOS 的应用数据目录（通常为 `~/Library/Application Support/com.local.bambuspools/inventory.sqlite`）。备份位置由用户每次自行选择。删除或覆盖数据库前请先退出应用并导出备份。

## 迁移到树莓派

G-code/3MF 解析、结算和 SQLite 账本是 Rust 核心逻辑，可以移植到 Raspberry Pi Zero 2 W。这个 macOS 的 Tauri 窗口、Dock 与菜单栏外壳不能原样运行在树莓派上；树莓派版本需要另做 Linux 服务或网页界面，再导入经过校验的 JSON 数据。不要直接在两台设备同时写同一个 SQLite 文件。
