<div align="center">
  <img src="src/assets/brand/cylune-mark.png" alt="CYLUNE Logo" width="156">
  <h1>CYLUNE</h1>
  <p><strong>面向 3D 打印工作流的本地耗材管理工具</strong></p>
  <p>把每一卷耗材、每一份切片数据和每一次打印记录放在同一个地方。</p>
</div>

## 下载 CYLUNE v1.0

| 平台 | 安装包 | 说明 |
| --- | --- | --- |
| macOS | [下载 CYLUNE for macOS](https://github.com/supermebigfox/cylune/releases/download/v1.0/CYLUNE.dmg) | Apple Silicon（M 系列芯片） |
| Windows | [下载 CYLUNE for Windows](https://github.com/supermebigfox/cylune/releases/download/v1.0/CYLUNE-Setup.exe) | Windows 安装程序 |

[查看 CYLUNE v1.0 的完整发布说明](https://github.com/supermebigfox/cylune/releases/tag/v1.0)

## 关于 CYLUNE

一卷耗材离开包装后，余量往往只能靠手感和记忆估算。同一种颜色如果有两卷或更多，每一卷的剩余量也不相同。

CYLUNE 为每卷耗材建立独立档案，并把耗材库存、切片数据与打印记录连接起来。你可以记录品牌、材质、系列、颜色和剩余重量；即使几卷耗材的参数完全相同，它们仍会保留各自的库存与使用记录。

将 `.3mf` 或 `.gcode.3mf` 文件交给 CYLUNE 后，软件会读取彩色模型缩略图、打印盘、层数、预计时间，以及每种颜色所需的耗材克数。未切片的项目文件可以调用本机 Bambu Studio 的切片能力完成分析，切片产生的临时输出会在读取数据后清理。

打印结束时，你可以选择成功、失败或取消。CYLUNE 会根据结果结算耗材：成功任务扣除完整用量；失败或取消的任务按停止层数计算消耗；需要重新打印时，可以沿用原任务继续安排。

## 核心功能

- **独立料卷库存**：同品牌、同材质、同颜色的不同料卷分别记录余量。
- **拓竹原厂耗材目录**：按材质、系列和官方颜色快速建立耗材档案。
- **切片与文件读取**：处理 `.3mf` 和 `.gcode.3mf`，记录缩略图、打印盘、时间、层数及分色用量。
- **耗材映射**：把切片文件中的颜色和用量映射到真实库存料卷。
- **打印结算**：根据成功、失败或取消结果扣除耗材，并支持重新打印流程。
- **打印记录**：查看任务录入时间、彩色预览、耗材克数、打印时长和层数。
- **桌面黑洞**：启用后可将文件拖入桌面黑洞，作为 CYLUNE 的动态导入入口。
- **多打印机配置**：保存不同打印机与切片配置，处理来自不同设备的项目文件。
- **多语言与主题**：支持简体中文、繁体中文、英语，以及浅色和深色模式。

## 使用流程

1. **登记耗材**：为现有料卷建立独立档案，填写材质、颜色和剩余重量。
2. **导入或切片**：拖入已切片文件，或让 CYLUNE 调用本机切片环境分析项目 3MF。
3. **结算打印**：选择打印结果，确认实际停止层数与耗材扣除，需要时继续安排重打。

## 当前支持范围

- CYLUNE 当前以电脑端导入、切片和打印记录为主。
- Bambu Handy 与 MakerWorld 发起的云打印任务不会自动同步到 CYLUNE。
- 项目 3MF 的切片依赖本机可用的 Bambu Studio 与对应打印机预设。
- 已切片的 `.gcode.3mf` 可以直接读取，不需要再次切片。
- 模型、库存和打印记录保存在本机应用数据中。

## 安装提示

### macOS

当前 `v1.0` 安装包支持 Apple Silicon。该版本尚未完成 Apple 公证；如果 macOS 阻止首次打开，请前往“系统设置 → 隐私与安全性”，确认打开 CYLUNE。

### Windows

首次运行安装程序时，Microsoft Defender SmartScreen 可能显示安全提示。请确认文件来自本仓库的官方 Release 页面后再继续安装。

## 项目链接

- [Releases 与安装包](https://github.com/supermebigfox/cylune/releases)
- [提交问题与建议](https://github.com/supermebigfox/cylune/issues)
- [第三方组件声明](THIRD_PARTY_NOTICES.md)

<div align="center">
  <sub><strong>Developer: Robin Lyu</strong></sub>
</div>
