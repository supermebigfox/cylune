# CYLUNE 发布影片剪辑与交付规范

## 剪辑项目

### 时间线

| 项目 | 设置 |
|---|---|
| 分辨率 | 3840×2160 |
| 帧率 | 30fps |
| 总时长 | 01:10:00，允许编码后误差不超过 1 帧 |
| 色彩 | SDR Rec.709，Gamma 2.4 |
| 音频 | 48kHz、24-bit、Stereo |
| 背景 | 深空黑或冰白，不使用透明画布作为最终底层 |

所有时间码以 [`script.md`](script.md) 为准。先建立 70 秒空时间线并放置 12 个标记，再导入素材。

### 轨道组织

```text
V6  Safety overlays / review notes（导出前关闭）
V5  CYLUNE master logo and end card
V4  English titles
V3  UI windows, masks, highlights, filament overlays
V2  Seedance shots and generated transitions
V1  Real desktop and CYLUNE screen recordings

A5  Final accents and logo shimmer
A4  UI clicks, glass, filament, confetti
A3  Black-hole spin, suction, jet, transition
A2  Atmosphere and low-frequency gravity
A1  Music
```

每条轨道只承担一种职责。不要把可读 UI 和生成画面预合成为不可修改的单一文件，直到 picture lock 通过。

## 素材解释与变速

- 60fps UI 素材放入 30fps 时间线时，正常速度镜头直接按 100% 播放，由剪辑软件每两帧取一帧。
- 需要慢动作时设为 50%，此时每个录制帧对应一个输出帧，不使用 Optical Flow。
- UI 画面禁止使用 Optical Flow、Frame Blend 或 AI 插帧，它们会扭曲文字和数字。
- Seedance 素材若为 24fps，可先按原时长进入 30fps 时间线；只有抽象光纤出现明显顿挫时，才使用高质量 Optical Flow，并逐帧检查线束边缘。
- 切片进度使用分段删减或线性时间重映射；确保数字单调从 0% 到 100%，不能倒退、跳回或伪造中间状态。
- 不通过冻结错误数字来延长界面停留；需要停留时使用同一真实状态的静帧。

## 12 镜头装配

| Shot | 时间线 | 主素材 | 主要处理 |
|---|---|---|---|
| 01 | 00:00–00:04 | `BH-01-idle-dark.mov` | 暗角、100→102% 缓推、8 帧淡入 |
| 02 | 00:04–00:08 | `BH-01-idle-dark.mov` | 延续同一录屏、102→104% 缓推 |
| 03 | 00:08–00:14 | `BH-02-ingest-dark.mov` | 保持真实鼠标动作和吸入，不改变黑洞大小 |
| 04 | 00:14–00:18 | 选定 `SD20-S04-*` | 与真实射流匹配，剪掉第五秒，尾端留冰白空间 |
| 05 | 00:18–00:24 | `UI-02-slice-entry.mov` / 静帧 | 真实窗口置入冰白空间，缩略图从轻虚到清晰 |
| 06 | 00:24–00:31 | `UI-03-slice-full.mov`、`UI-04-multiplate.mov` | 时间压缩进度，真实多盘卡片后期移动 |
| 07 | 00:31–00:38 | `UI-05-color-usage.mov` | 遮罩高亮真实色块与克数，后期绘制细耗材丝 |
| 08 | 00:38–00:46 | `UI-06-same-color.mov`、`UI-07-ams-map.mov` | 同色不同卷到 AMS 的匹配转场 |
| 09 | 00:46–00:53 | `UI-08-history.mov`、`UI-09-job-detail.mov` | 记录卡推进、信息逐项高光 |
| 10 | 00:53–01:01 | `UI-10-success.mov` | 使用真实扣减和庆祝，不改变时间顺序 |
| 11 | 01:01–01:06 | 选定 `SD20-S11-*` | 彩纸接光纤，尾 8–16 帧显现真实透明 Logo |
| 12 | 01:06–01:10 | 品牌母版 | 固定终章、光泽掠过、品牌句和发布状态 |

## UI 合成

### 窗口尺寸

- Shot 05–10 的主窗口宽度约为画面宽度的 70%–76%。
- 需要阅读多列数据时可放大到 82%，但窗口不得触及 240px 水平安全边距。
- 保持窗口正面；最多使用 1.5° 的轻微透视，不为“高级感”过度倾斜。
- 截图和录屏均按整像素缩放，避免文字落在半像素坐标产生模糊。

### 阴影与玻璃

冰白背景上的窗口使用两层阴影：

```text
Shadow 1: Y 28px, blur 80px, opacity 12%, cool gray
Shadow 2: Y 4px, blur 18px, opacity 8%, neutral black
```

可在窗口边缘添加 1px、20% 不透明度的冰白描边。不要给窗口整体加高斯模糊；液态玻璃效果只来自应用真实 UI 或背景层。

### 高光与耗材丝

- 数据高光使用 12–18 帧的柔和亮度扫过，不改变原始文字颜色。
- 连接模型与克数的耗材丝宽度为 3–5px，颜色直接取自真实模型色块。
- 每根线只完成一次生长动作，持续 14–20 帧。
- 同时出现的活动线条不超过两根，避免变成流程图。

## Seedance 镜头合成

### Shot 04

1. 以 `BH-02-ingest-dark.mov` 的真实射流结束帧作为匹配点。
2. 生成素材从最接近该帧的位置开始；用 6–10 帧亮度遮罩融合，不使用普通长叠化。
3. 若生成素材中的黑洞发生变化，只裁取不含黑洞的光束和冰白尾段。
4. 冰白尾段保持至少 20 帧干净画面，让真实 UI 窗口在 Shot 05 出现。
5. 加入极轻的 0.25% 单色噪点统一生成画面，不对 UI 加噪点。

### Shot 11

1. 从 `UI-10-success.mov` 选择蓝紫彩纸最完整的一帧。
2. 使用 `UI-10-success-tail.mov` 与生成素材做 6–8 帧粒子位置匹配。
3. 在 01:04.8 左右开始用 Luma Matte 或线束遮罩显现 `brand/cylune-mark-transparent.png`。
4. 01:05.7 前必须完全切换到真实 Logo，生成 Logo 不得进入 Shot 12。
5. 真实 Logo 只允许整体透明度、缩放和光泽变化，不允许液化、重绘或改变开口方向。

## 字体与排版

### 字体

- 功能标题：Inter Display Medium。
- 品牌句与发布状态：Inter Display Regular 或 Medium。
- `CYLUNE`：直接使用 `brand/cylune-master.png` 或从同一母版提取的字标，不使用字体重打。

### 功能标题

| 项目 | 规格 |
|---|---|
| 字号 | 104px，4K 时间线 |
| 行高 | 1.05 |
| 字距 | -1% 到 0%，按字形光学校正 |
| 深色画面 | `#F6F8FF` |
| 冰白画面 | `#171B24` |
| 最大宽度 | 1600px |
| 水平安全边距 | 240px |
| 垂直安全边距 | 160px |

Shot 03 的标题位于左下安全区；Shot 05–10 位于上方留白或与 UI 不冲突的左上位置。每次只显示一条功能标题。

### 标题动画

- 入场：10 帧，透明度 0→100%，Y 从 +16px 到 0，Ease Out Cubic。
- 停留：至少 1.8 秒完全不动。
- 出场：8 帧，透明度 100→0%，Y 保持不动。
- 不使用逐字打字、弹跳、模糊旋转、故障或发光描边。

### 终章

- 图形标志宽度约 640px，位于中心轴上方。
- CYLUNE 字标使用现有母版，视觉宽度约 780px。
- `Every gram, in view.`：54px，深灰，位于字标下方 110px。
- `Available now.`：36px，深灰 70% 不透明度，位于品牌句下方 74px。
- 终章左右安全边距 320px，上下安全边距 200px。
- `Available now.` 保持句首大写，不改为全大写。

## 色彩

### 品牌参考色

后期能量色以现有 Logo 为准，以下值只用于新建图形和光效：

```text
Ice white       #F6F8FC
Mist blue       #E8EEFA
Deep space      #111722
Cobalt blue     #176BFF
Electric violet #6E35F2
Coral accent    #FF716C
Text charcoal   #171B24
```

- 不改变软件真实色标和模型颜色。
- 录屏只做白平衡、曝光和对比度统一，不套电影 LUT。
- 深色桌面保留黑位细节，不压成纯黑色块。
- 冰白背景最高亮度保持有纹理，不让蓝紫光晕大面积过曝。
- 生成镜头与录屏的色彩匹配以 Shot 03 射流和现有冰白海报为准。

## 音乐剪辑

使用约 108 BPM 的无歌词现代电子音乐。70 秒时间线可按下列段落重组原曲：

| 时间 | 音乐状态 |
|---|---|
| 00:00–00:08 | 无鼓点，低频脉冲与空气感纹理 |
| 00:08–00:14 | 引入轻微节拍与上升音 |
| 00:14–00:18 | 过渡打开，加入第一组玻璃打击乐 |
| 00:18–00:31 | 温暖中频和弦与精密脉冲 |
| 00:31–00:46 | 节奏完整但克制，颜色逐项对应音阶 |
| 00:46–00:53 | 稍微降低旋律密度，为信息阅读留空间 |
| 00:53–01:01 | 全片高潮，成功后才进入完整声场 |
| 01:01–01:06 | 逐步抽走鼓点，只保留聚合上升音 |
| 01:06–01:10 | 和弦尾音与最后一个高频点，安静结束 |

如果音乐原曲无法在 01:06 自然收束，优先在整小节边界重剪，不用生硬音量淡出掩盖不合拍结构。

## 音效时间表

| 时间点 | 音效 | 混音说明 |
|---:|---|---|
| 00:01.0 | `SFX-gravity` | 低于音乐 8–12dB，只建立空间感 |
| 00:04.0 | `SFX-spin` | 跟随黑洞顺时针运动，避免持续轰鸣 |
| 00:10.0 | `SFX-spin` 加速层 | 音高和速度上升，音量增长不超过 4dB |
| 00:11.5 | `SFX-suction` | 短促向心收缩，低频中心化 |
| 00:12.2 | `SFX-jet` | 从中心扩到立体声两侧，峰值清晰 |
| 00:15.5 | `SFX-glass` | 射流进入冰白空间的材质转变 |
| 00:18.2–00:22 | `SFX-ui` ×4 | 窗口和缩略图组装，音高递增 |
| 00:25.0–00:28 | 精密脉冲 | 跟随真实进度，不做每 1% 一次点击 |
| 00:28.2–00:31 | `SFX-glass` ×盘数 | 多盘卡片依次展开 |
| 00:31–00:38 | `SFX-filament`、`SFX-ui` | 拉丝与克数亮起分别处理 |
| 00:38–00:46 | 玻璃滑动、AMS 确认音 | 同色卷切换要柔和，槽位确认要明确 |
| 00:47.5 | 记录展开 | 低音量、短尾音 |
| 00:54.0 | 真实按钮点击 | 不提前加入成功声 |
| 00:55.4 | 结算确认 | 只有真实持久化成功后出现 |
| 00:56.0 | `SFX-confetti` | 窗口内庆祝，宽立体声但不过量 |
| 01:01–01:05 | 反向彩纸、光纤吸入 | 从宽声场逐步收向中心 |
| 01:06.5 | `SFX-logo` | 极轻玻璃闪光，尾音至结束 |

## 混音标准

- 成片综合响度目标：约 -14 LUFS-I。
- True Peak：不高于 -1.0 dBTP。
- 对白轨不存在，不为“听清旁白”压低全部音乐。
- 低频黑洞效果在手机和笔记本扬声器上仍需可感知，但不能只靠 40Hz 以下次声。
- UI 点击比音乐瞬时高 1–3dB 即可，不要像游戏菜单。
- 庆祝声是唯一明显打开立体声宽度的段落。
- 终章尾音不能被编码器直接截断，最后保留至少 8 帧稳定画面和自然音频尾巴。

## 导出规格

### 4K 母版

```text
Container: MOV
Video: Apple ProRes 422 HQ
Resolution: 3840×2160
Frame rate: 30fps constant
Color: Rec.709 Gamma 2.4
Audio: Linear PCM, 48kHz, 24-bit, stereo
Filename: CYLUNE-Launch-Master-4K.mov
```

### 4K 发布版

```text
Container: MP4
Video: H.265 / HEVC Main10
Resolution: 3840×2160
Frame rate: 30fps constant
Video bitrate: 35–55 Mbps VBR
Audio: AAC-LC, 48kHz, 320kbps, stereo
Fast start: enabled
Filename: CYLUNE-Launch-4K.mp4
```

### 1080p 审片版

```text
Container: MP4
Video: H.264 High Profile
Resolution: 1920×1080
Frame rate: 30fps constant
Video bitrate: 12–20 Mbps VBR
Audio: AAC-LC, 48kHz, 320kbps, stereo
Fast start: enabled
Filename: CYLUNE-Launch-Review-1080p.mp4
```

## 技术校验

安装 FFmpeg 后，对三个文件分别执行：

```bash
ffprobe -v error \
  -show_entries format=filename,duration,format_name \
  -show_entries stream=index,codec_type,codec_name,profile,width,height,r_frame_rate,avg_frame_rate,pix_fmt,sample_rate,channels \
  -of json \
  "video/exports/master/CYLUNE-Launch-Master-4K.mov"
```

母版预期：

- `width=3840`、`height=2160`
- `r_frame_rate=30/1`、`avg_frame_rate=30/1`
- `duration` 接近 70.000 秒，误差不超过 0.034 秒
- 视频编码为 ProRes 422 HQ 对应的 `prores` / HQ profile
- 音频 `sample_rate=48000`、`channels=2`

对发布版和审片版替换文件路径后重复检查。发布版还需要确认 HEVC Main10 和 10-bit 像素格式；审片版确认 H.264、1920×1080。

检查响度：

```bash
ffmpeg -i "video/exports/master/CYLUNE-Launch-Master-4K.mov" \
  -filter_complex ebur128=peak=true \
  -f null -
```

验收 Integrated Loudness 约 -14 LUFS，True Peak 不高于 -1.0 dBTP。

## 画面 QA

### 内容准确性

- [ ] 未切片 3MF 从黑洞进入快速切片，不出现“尚未切片”死路。
- [ ] 使用的软件首选打印机与演示数据一致。
- [ ] 多盘数量、缩略图、打印时长、层数和耗材重量来自真实结果。
- [ ] 官方颜色中文名称与耗材卷匹配。
- [ ] 两卷相同颜色仍显示不同余额和独立身份。
- [ ] 成功结算后扣减数字等于真实任务用量。
- [ ] 失败、取消和重打流程不在本片中虚构展示。

### 视觉准确性

- [ ] 黑洞始终顺时针、动态向中心吸入，没有水波或硬圆边。
- [ ] 黑洞靠近文件时增强转速和力度，但不改变大小。
- [ ] Shot 04 没有生成第二个黑洞、传送门、虚构 UI 或文字。
- [ ] Shot 11 光纤顺时针收束，最终 Logo 来自真实品牌母版。
- [ ] 软件界面中文字与数字逐帧清晰，没有插帧变形。
- [ ] 庆祝动画严格限制在 CYLUNE 窗口内。
- [ ] 所有英文文案拼写和标点与 [`script.md`](script.md) 一致。
- [ ] `Available now.` 使用正确大小写并处于安全区。

### 隐私与版权

- [ ] 没有私人路径、账号、通知、联系人、设备昵称或文件列表。
- [ ] 演示模型拥有公开展示权。
- [ ] 音乐、音效和字体商业授权已归档。
- [ ] 没有截取 Apple 发布会的画面、音乐或音效。
- [ ] 影片只是采用克制的产品发布语言，没有复制某一支既有广告的具体镜头排列。

### 播放测试

- [ ] 在 MacBook 内建屏幕全屏播放。
- [ ] 在普通 1080p 显示器播放。
- [ ] 使用笔记本扬声器、耳机和手机外放分别试听。
- [ ] 上传一个未公开版本到目标平台，检查平台二次压缩后的文字和暗部。
- [ ] 从头无声播放一次，确认没有旁白也能理解核心流程。
- [ ] 只听声音不看画面一次，确认吸入、切片、结算和终章节奏连续。
