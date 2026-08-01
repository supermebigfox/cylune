# CYLUNE 发布影片素材准备表

## 先准备什么

你只需要准备三类真实素材：

1. 一套干净、可公开展示的 CYLUNE 演示数据。
2. 按本表录制的软件与黑洞画面。
3. 一首有商用授权的音乐和一组有授权的音效。

Logo 和冰白海报已经在项目的 `brand/` 目录中，不需要重新生成。

## 目录结构

大型视频素材不要提交到 Git。录制前在 Finder 中建立：

```text
video/assets/
  capture/
    blackhole/
    ui/
  stills/
  generated/
    shot04/
    shot11/
  audio/
    music/
    sfx/
  licenses/
video/exports/
  review/
  master/
```

## 通用录制设置

| 项目 | 设置 |
|---|---|
| 首选分辨率 | 3840×2160 |
| 录制帧率 | 60fps |
| 色彩 | SDR / Rec.709，关闭 HDR |
| 编码 | ProRes 422 LT、HEVC 高码率或 OBS 无损近似 |
| 音频 | 系统音可关闭，后期统一设计 |
| 光标 | 按每条素材要求显示或隐藏 |
| 镜头余量 | 每次操作前后各保持 2 秒静止 |

如果 Mac 内建屏幕无法直接录到 3840×2160：优先录制屏幕原生 Retina 像素并保持 UI 清晰，OBS 画布可设为 3840×2160；不要为了凑 4K 先把界面缩得很小。录制结束后再统一高质量放大。

## 录制前系统清理

- 打开“专注模式”，关闭所有横幅、角标和信息预览。
- 隐藏桌面上的私人文件、磁盘名称和截图。
- 使用干净的深蓝灰壁纸，不用苹果默认动态壁纸或带版权图像。
- 退出微信、邮箱、日历、网盘和会改变菜单栏状态的应用。
- 菜单栏时间保持一致；若会入镜，录制全部镜头时不要跨分钟。
- 关闭夜览、原彩和 HDR，防止不同录屏之间颜色变化。
- 将 CYLUNE 窗口固定在同一尺寸和同一位置。
- 录制前退出重复的 CYLUNE 副本，确认活动监视器中只有一个进程。

## 演示文件

### 主模型文件

准备一个拥有公开展示权的 3MF：

- 文件名：`CYLUNE Demo.3mf`
- 类型：未切片 3MF，用于演示黑洞导入后进入快速切片。
- 颜色：3–5 种，必须有明显色差；推荐钴蓝、电紫、黄色、黑色和少量珊瑚红。
- 造型：轮廓简洁、缩略图识别度高，不使用电影、游戏或他人角色 IP。
- 盘数：准备单盘版和 2–4 盘的多盘版。
- 工艺：使用能在演示机型上正常完成的真实设置，不手填虚假重量、层数或时间。

建议自己建一个简单的抽象环形、涡旋或耗材丝模型。不要使用当前测试文件中带有私人项目名称或可能涉及他人版权的面具、武器和商业模型。

### 演示耗材

在 CYLUNE 中录入 6–8 卷拓竹原厂耗材：

| 卷 | 材质示例 | 颜色 | 建议剩余量 | 用途 |
|---|---|---|---:|---|
| 01 | PLA Basic | 钴蓝或蓝色 | 812g | 主模型颜色 |
| 02 | PLA Basic | 紫色 | 684g | 主模型颜色 |
| 03 | PLA Basic | 黄色 | 531g | 主模型颜色 |
| 04 | PLA Basic | 黑色 | 903g | 主模型颜色 |
| 05 | PLA Matte | 白色 | 746g | 丰富素材库 |
| 06 | PLA Silk | 红色或珊瑚色 | 428g | 丰富素材库 |
| 07 | PLA Basic | 与卷 01 相同 | 267g | 展示同色不同卷 |
| 08 | PETG Basic | 透明或灰色 | 615g | 展示不同材质 |

颜色名称必须从应用内官方中文目录选择。两卷同色耗材需要独立命名或显示不同录入时间、剩余量，确保观众一眼能看出它们不是同一卷。

### 打印机与记录

- 打印机名称使用正式通用名称，例如 `Bambu Lab P2S`。
- 不使用“Robin 的打印机”等私人设备昵称。
- 演示打印记录放在同一天，时间间隔整齐。
- 不手改切片结果；时长、层数、逐色重量必须来自 CYLUNE 对演示文件的真实解析。
- 开始成功结算镜头前，记录各卷结算前余额，便于核对扣减。

## 黑洞录屏清单

保存到 `video/assets/capture/blackhole/`。

| 文件名 | 最短长度 | 光标 | 用于 | 录制内容与验收 |
|---|---:|---|---|---|
| `BH-01-idle-dark.mov` | 10s | 隐藏 | Shot 01–02 | 深色干净桌面；黑洞待机，顺时针流动，周围无硬圆边；前后各 2s 静止 |
| `BH-02-ingest-dark.mov` | 12s | 显示 | Shot 03 | 从右侧拖入 `CYLUNE Demo.3mf`；靠近后黑洞只增强转速与吸力、不变大；释放后完整吸入并射流 |
| `BH-03-idle-light.mov` | 8s | 隐藏 | 备用 | 浅灰或冰白背景上待机，用于检查亮带可见性和边缘透明度 |
| `BH-04-jet-ref.mov` | 1.5–2s | 隐藏 | Seedance Shot 04 | 从 `BH-02` 无重编码截取吸入后到射流结束；不包含私人文件名 |

从 `BH-04-jet-ref.mov` 导出最后一帧：

```text
video/assets/stills/BH-04-exit.png
```

黑洞录屏必须逐项确认：

- [ ] 黑洞动态连续，不是静态截图透镜。
- [ ] 外围扭曲是顺时针向中心吸入，不是水波向外扩散。
- [ ] 没有明显矩形捕获边界或一瞬间的黑边。
- [ ] 黑洞拖动经过其他桌面文件时没有误导入。
- [ ] 只有拖到黑洞中心并释放的演示文件被导入。
- [ ] 黑洞外观与当前确认版本一致，没有为视频临时改样式。

## UI 录屏清单

保存到 `video/assets/capture/ui/`。除明确需要点击的素材外，鼠标停在不遮挡内容的位置。

| 文件名 | 最短长度 | 光标 | 用于 | 录制内容与验收 |
|---|---:|---|---|---|
| `UI-01-home.mov` | 7s | 隐藏 | Shot 05 备用 | 主界面稳定展示，侧栏液态玻璃选择态清晰，无弹窗 |
| `UI-02-slice-entry.mov` | 7s | 显示 | Shot 05–06 | 黑洞导入未切片文件后，应用直接进入快速切片流程 |
| `UI-03-slice-full.mov` | 完整过程 | 显示 | Shot 06 | 目标打印机已选；从点击开始切片录到 100% 和结果出现，中途不停止录屏 |
| `UI-04-multiplate.mov` | 10s | 隐藏 | Shot 06 | 展示 2–4 盘真实结果、每盘彩色缩略图和盘数 |
| `UI-05-color-usage.mov` | 10s | 隐藏 | Shot 07 | 任务详情展示每种真实颜色和对应克数，所有数据至少停留 2s |
| `UI-06-same-color.mov` | 10s | 隐藏 | Shot 08 | 耗材库同时展示两卷相同官方颜色、不同剩余量的耗材 |
| `UI-07-ams-map.mov` | 10s | 显示 | Shot 08 | 从任务颜色到 AMS 槽位的真实匹配与确认，不演示匹配错误 |
| `UI-08-history.mov` | 10s | 显示 | Shot 09 | 进入打印记录，显示统一日期、彩色缩略图和任务卡片 |
| `UI-09-job-detail.mov` | 10s | 显示 | Shot 09 | 打开任务详情，稳定展示时间、时长、层数和逐色克数 |
| `UI-10-success.mov` | 12s | 显示 | Shot 10 | 点击打印成功，等待真实保存完成，拍到库存扣减和完整窗口内庆祝 |
| `UI-11-balance-after.mov` | 6s | 隐藏 | QA | 结算后的耗材库余额，用于证明扣减数字正确 |

从 `UI-10-success.mov` 无重编码截取最后 1.5–2 秒的彩纸动作：

```text
video/assets/capture/ui/UI-10-success-tail.mov
```

再导出最后一帧：

```text
video/assets/stills/UI-10-success-last.png
```

## 4K 静帧清单

保存到 `video/assets/stills/`，PNG、无鼠标、无压缩重采样：

| 文件名 | 内容 |
|---|---|
| `ST-01-home.png` | 主界面与液态玻璃侧栏 |
| `ST-02-slice.png` | 选择打印机后的切片页 |
| `ST-03-multiplate.png` | 彩色多盘结果 |
| `ST-04-color-usage.png` | 逐色耗材克数 |
| `ST-05-spools.png` | 耗材库与同色不同卷 |
| `ST-06-ams.png` | AMS 槽位映射 |
| `ST-07-history.png` | 打印记录列表 |
| `ST-08-job.png` | 打印任务详情 |
| `ST-09-success.png` | 窗口内庆祝的视觉高峰 |

每张静帧检查中文、数字、模型颜色和窗口阴影。任何一个字段被鼠标、Tooltip 或系统通知遮挡都需要重新导出。

## Seedance 输入素材清单

### Shot 04

- `BH-04-jet-ref.mov`
- `BH-04-exit.png`
- `brand/poster-final-4k.png`
- `brand/cylune-mark.png`

### Shot 11

- `UI-10-success-tail.mov`
- `UI-10-success-last.png`
- `brand/cylune-mark.png`
- `brand/poster-final-4k.png`

生成文件按 [`prompts.md`](prompts.md) 的版本号保存到 `video/assets/generated/shot04/` 和 `video/assets/generated/shot11/`。

## 音乐素材

准备一首无歌词、可商用、能够覆盖 70 秒的音乐。原曲至少 80 秒，方便剪辑头尾。

**检索或音乐生成描述：**

```text
108 BPM premium technology product launch instrumental, restrained modern electronica, deep soft pulse, precise glass percussion, elegant evolving synth texture, quiet mysterious opening, clean optimistic middle, controlled celebratory peak around 55 seconds, percussion drops away near the ending, no vocals, no cinematic trailer brass, no aggressive dubstep, no corporate ukulele, no recognizable melody, spacious high-end mix
```

验收要求：

- [ ] 无歌词、无人声采样、无可辨识知名旋律。
- [ ] 允许商用发布，并保存授权截图、发票或许可证到 `video/assets/licenses/`。
- [ ] 00:53–01:01 有可用高潮，01:06 后能自然收束。
- [ ] 不依靠重低音轰炸制造“高级感”。
- [ ] 若使用 AI 音乐，确认所用套餐授予商业使用权。

## 音效素材

保存为 48kHz、24-bit WAV；每个类型至少准备 2 个可选版本：

| 文件前缀 | 内容 |
|---|---|
| `SFX-gravity-*` | 低频引力与轻微空间张力 |
| `SFX-spin-*` | 黑洞顺时针加速，避免机械马达感 |
| `SFX-suction-*` | 文件收缩吸入 |
| `SFX-jet-*` | 短促蓝紫射流 |
| `SFX-glass-*` | 磨砂玻璃展开与轻滑 |
| `SFX-ui-*` | 精密晶体点击与确认音 |
| `SFX-filament-*` | 耗材丝拉伸和光纤移动 |
| `SFX-confetti-*` | 窗口内彩纸爆发 |
| `SFX-logo-*` | Logo 光泽掠过 |

所有音效都要保存授权证明。不要直接从电影、游戏、Apple 发布会或公开视频中截取声音。

## 字体素材

- 安装 `Inter Display` 或 Inter Variable 字体。
- 保存字体的 OFL 授权文件到 `video/assets/licenses/`。
- `CYLUNE` 品牌字标不使用字体重打，直接使用现有品牌图。

## 最终素材交接检查

- [ ] 所有录屏可以正常播放，无掉帧、损坏或可变帧率异常。
- [ ] 黑洞素材包含真实动态、文件靠近加速、吸入和射流。
- [ ] UI 素材覆盖切片、多盘、逐色用量、耗材库、AMS、记录和成功结算。
- [ ] 成功结算画面中的余额扣减与真实切片用量一致。
- [ ] 成功庆祝只在 CYLUNE 窗口内。
- [ ] 所有文件名符合本表，没有“最终版2”“新的”“未命名”等名称。
- [ ] 没有私人路径、账号、通知、设备昵称或无权展示的模型。
- [ ] 音乐、音效和字体授权文件齐全。
