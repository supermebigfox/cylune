# BlackHoleTrash 桌面黑洞安全导入设计

日期：2026-07-28

状态：用户已批准

目标平台：macOS 优先

取代范围：本文件取代 `2026-07-27-black-hole-pet-design.md` 中的黑洞外观、拖放反馈和吞噬动画设计；旧文档中的耗材业务、设置、ScreenCaptureKit 生命周期、多显示器、单实例、待结算与结算边界继续有效。

## 1. 已批准方向

桌面宠物的外观和完整吞噬反馈直接以
[`rrrjqy66/BlackHoleTrash`](https://github.com/rrrjqy66/BlackHoleTrash)
为规范参考。实现使用其当前默认 `Gargantua` 视觉参数、桌面透镜结构和成功后的
吸收喷流，不再把当前彩色圆环与 `blackhole-timer` 外观混合成第三种风格。

macOS 的窗口命中、文件卡片和坠落动画可以参考 BlackHoleTrash README 关联的
[`ZGhey/blackhole-mac`](https://github.com/ZGhey/blackhole-mac)，但只把它当作
macOS 行为和动画分段参考。最终黑洞的光学外观、默认参数和吸收喷流仍以
BlackHoleTrash 为准。

用户在 2026-07-28 对话中提供的目标截图是人工视觉验收参考，不是构建资产。
验收时必须能看到：黑色事件视界、穿过中心的暖白至金色吸积盘、上下透镜弧，
以及黑洞后方桌面文字和图标的连续弯折。截图与 BlackHoleTrash 当前默认效果
发生细微差异时，以 BlackHoleTrash `Gargantua` 默认效果为主。

### 1.1 固定参考版本

- BlackHoleTrash：commit
  `229d93213cd3e57364b4c6655cfb2c75b7ea4d18`
- blackhole-mac：commit
  `f719aa1139ecc49a728cbb8fac2e60fcfa51996e`

实现和视觉验收使用上述固定版本，避免上游更新改变本项目的完成标准。

## 2. 产品语义

BlackHoleTrash 原本在回收成功后播放吸收反馈。本应用把“删除/回收”完整替换为
“本地导入打印文件”：

- 用户把文件主动拖向黑洞中心并释放，应用读取第一个受支持的普通文件。
- Rust `PrintService` 完成解析、创建或重新打开待结算任务后，黑洞才完成吞噬。
- 源文件始终保留在原路径，不能删除、移入废纸篓、移动、重命名、覆盖或写回。
- 导入不会扣除耗材。只有现有结算事务可以修改耗材余额。
- 导入失败时不创建新任务、不扣料、不完成吞噬，文件卡片回退并显示错误反馈。
- 移动黑洞经过 Finder 文件不会读取文件，也不会生成导入事件。

本功能不是垃圾桶替代品。界面、通知、代码和文档中不得出现会让用户以为源文件
会被删除的文案或图标。

## 3. 范围与非目标

### 3.1 本次范围

- 修正视觉窗口、捕获区域、捕获纹理与命中圆的几何关系。
- 把固定版本 BlackHoleTrash 默认 `Gargantua` 光学效果移植到 Metal。
- 加入一张不暴露文件内容的通用文件卡片和完整吞噬动画。
- 加入有 generation 的原生拖放会话和 Rust 导入回执。
- 保留真实模式、轻量模式、120–360 px、自动/30/60 FPS、多显示器和
  Reduce Motion。
- 保留现有待结算轨道点、结算成功反馈和错误通知，但使其与新视觉一致。

### 3.2 非目标

- 不复制 BlackHoleTrash 的回收站、永久删除、废纸篓或恢复功能。
- 不复制鼠标引力、真实鼠标轨迹、自动漂移、吞噬数量成长、Kerr 自旋设置、
  预设选择器、屏保、更新检查或截图修补。
- 不接收文件夹、符号链接、设备文件、文本、URL、图片或剪贴板内容。
- 不一次导入多个文件。一次拖放只选择第一个受支持的普通文件。
- 不在文件卡片上显示文件名、模型缩略图或文件内容。
- 不改变 `.gcode.3mf`、已切片 `.3mf`、受限 `.gcode` 的现有解析和结算规则。
- 不在本次实现 Windows 原生渲染。

## 4. 几何、窗口和捕获

`pet_size` 继续表示透明视觉窗口的逻辑边长，范围保持 `120..=360` px；不新增
第二个尺寸概念。

对于边长 `S`：

- Shader 的黑洞阴影半径为 `0.075 × S`。
- 圆形鼠标/拖放命中半径为
  `max(22 px, 1.15 × 0.075 × S)`。
- 命中区域以视觉窗口中心为圆心。承载它的子窗口可以是正方形，但点必须通过
  圆距离测试，正方形四角不可接受点击或文件拖放。
- 吸积盘、透镜和喷流只负责绘制，命中区域不能扩展到这些装饰区域。
- 视觉窗口外的桌面必须保持可点击。

真实模式从当前显示器捕获以视觉窗口为中心、边长 `1.60 × S` 的区域。捕获区域
被显示器边界裁剪时，必须把“视觉窗口左上角在捕获纹理中的 UV”和“视觉窗口
占捕获纹理的 UV 尺寸”传给 Shader。Shader 不能假设视觉窗口永远位于捕获纹理
正中央。

精确映射为：

```text
capture_uv = capture_origin_uv + local_panel_uv × capture_extent_uv
```

其中 `local_panel_uv` 为视觉窗口内 `0..1` 坐标。透镜采样在这个基准上施加偏移，
然后限制到实际捕获纹理的 `0.002..0.998`。位于屏幕左、右、上、下边缘时，
未被裁剪的一侧仍必须与桌面像素对齐；不能拉伸整个捕获纹理来填满窗口。

多显示器继续按视觉窗口与各屏幕的最大交集选择捕获源。跨屏时先更新显示器、
backing scale、捕获映射和 Metal drawable，再提交新帧。显示器断开时回到主显示器
安全区域。拖放会话进行中不得自动把黑洞漂移到其他位置。

## 5. BlackHoleTrash 视觉移植

### 5.1 固定默认参数

首版不暴露预设或自旋设置。Metal uniform 使用 BlackHoleTrash `Gargantua`
默认值：

```text
temperature = 4500 K
inclination = 1.52 rad
roll = 0.10 rad
inner radius = 2.2 r_s
outer radius = 7.0 r_s
opacity = 0.85
doppler = 0.35
beaming = 2.0
gain = 1.4
contrast = 0.5
wind = 7.0
speed = 5.0
exposure = 1.20
stars = 0.0
spin = 0.0
```

`spin = 0.0` 固定走 Schwarzschild 快速路径，不移植或启用用户可见 Kerr 设置。

### 5.2 必须保留的 Shader 结构

WGSL 到 MSL 的移植必须保留下列行为和数值边界：

- `LENS_DEPTH = 13.0`
- `N_STEPS = 48`
- `B_CRIT = 2.5980762`
- 近场使用 kick-drift-kick 测地线积分。
- 光线进入 `r² < 1.0` 时被事件视界捕获。
- 薄吸积盘通过射线与倾斜盘面的符号交叉检测。
- 一条射线可发生多次盘面穿越，形成上下透镜弧。
- 远场使用 BlackHoleTrash 的有限相机弱偏折拟合，并与近场平滑衔接。
- 盘面使用其黑体温度、Doppler、beaming、噪声、透明度和曝光组合。
- 真实模式的背景来自 ScreenCaptureKit 捕获纹理。
- 轻量模式使用透明背景和同一套事件视界、吸积盘与动画，不读取桌面纹理。

不得继续使用当前 `spectral_ring` 的青色、紫色、玫红色圆环，也不得用一个
`1 / radius` 的径向 UV 偏移冒充测地线积分。当前彩色圆环只属于待替换实现，
不属于验收目标。

### 5.3 透明度和合成

- 事件视界内部为不透明黑色。
- 盘面和喷流使用预乘 alpha 输出。
- 真实模式只有受透镜影响和黑洞本体覆盖的区域不透明；视觉窗口四角 alpha
  必须为零。
- 轻量模式不能绘制不透明矩形背景。
- 捕获必须排除本应用，避免递归捕获。

## 6. 文件卡片与完整吞噬动画

文件卡片是程序生成的通用纸张轮廓：

- `.gcode.3mf` 和 `.3mf` 使用 `3MF` 类型色。
- `.gcode` 使用 `GCODE` 类型色。
- Shader 只收到文件类型枚举，不收到文件名、路径、缩略图或内容。
- 卡片使用 SDF/程序几何绘制，不能为每次拖放读取 Finder 缩略图。

普通动态效果下，成功吞噬使用 blackhole-mac 的 `Faller` 分段和
BlackHoleTrash 的成功喷流：

| 阶段 | 归一化时间 | 绝对时间（4.6 s） | 行为 |
|---|---:|---:|---|
| 等待导入 | 无固定时长 | Rust 回执前 | 卡片停在视界外侧，做幅度不超过 3° 的呼吸摆动；不能越过视界 |
| 接近 | `0.00..0.25` | `0..1.15 s` | 卡片沿收紧轨道接近 |
| 潮汐拉伸 | `0.20..0.55` | `0.92..2.53 s` | 径向拉长、切向压缩并逐渐倾斜 |
| 碎裂 | `0.45..0.72` | `2.07..3.31 s` | 最多 12 个程序碎片沿原轨道散开 |
| 汇入吸积盘 | `0.70..0.88` | `3.22..4.05 s` | 卡片和碎片透明度下降并并入盘面 |
| 越过/余辉 | `0.82..1.00` | `3.772..4.60 s` | `u = 0.82` 只触发一次盘面冲击、吸收喷流和任务状态交付 |

BlackHoleTrash 吸收喷流持续 `0.90 s`：

- `0.00..0.13` 为快速 attack。
- `0.00..0.24` 完成喷流伸展。
- `0.02..0.72` 推进冲击环。
- `0.45..1.00` 衰减喷流。
- 中心闪光在 `0.28` 前结束。
- 单文件能量固定为 `1.0`；本应用不实现批量能量成长。

盘面冲击使用 blackhole-mac 关联实现的时间常数：

- attack `0.06 s`
- decay `0.90 s`
- 单次冲击 lifetime `4.0 s`
- feed afterglow decay `3.2 s`
- feed afterglow lifetime `14 s`

这些余辉可以与后续空闲帧重叠，但不能改变用户设置的黑洞尺寸。

导入失败使用 `0.42 s` 回退：前 `0.18 s` 卡片沿原路径反向移动，随后在
`0.24 s` 内淡出；同时只显示一次红色错误脉冲。失败时不得触发碎裂、盘面交付、
吸收喷流或待结算点增加。

## 7. 精确状态机

原生视觉状态为：

```text
Hidden
Idle
PetDragging
ExternalHoverValid
ImportPending
Swallow
ImportRejected
SettlementPulse
```

### 7.1 状态转换

```text
Hidden --show--> Idle
Idle --hide--> Hidden
Idle --mouseDown(center)--> PetDragging
PetDragging --mouseUp--> Idle

Idle --valid external drag enters center--> ExternalHoverValid
ExternalHoverValid --drag leaves center/session changes--> Idle
ExternalHoverValid --drop + second validation succeeds--> ImportPending
ImportPending --matching Rust success acknowledgment--> Swallow
ImportPending --matching Rust failure acknowledgment--> ImportRejected
Swallow --4.672 s complete--> Idle
ImportRejected --0.42 s complete--> Idle

Idle --settlement success--> SettlementPulse
SettlementPulse --0.48 s complete--> Idle
any visible state --hide/sleep/destroy--> Hidden
```

`ExternalHoverValid` 使用 `0.15 s` ease-in，离开时使用 `0.15 s` ease-out。
只有这一状态向 macOS 返回 `NSDragOperationCopy`。无效扩展名、文件夹、符号链接、
非文件 URL、位于中心圆外或已有 `ImportPending/Swallow` 的拖放返回
`NSDragOperationNone`。

`ImportPending` 没有猜测性超时。它持续到 Rust 返回同一 generation 的成功或失败，
或窗口隐藏/应用退出。过期 generation 的回执必须被忽略，不能启动动画。

一次只允许一个导入会话。`ImportPending`、`Swallow` 或 `ImportRejected` 期间的新
拖放不排队并返回 `NSDragOperationNone`。待结算任务数量可以大于一，不受这个视觉
会话限制。

### 7.2 拖动黑洞与拖入文件的隔离

黑洞移动只由命中圆内的 `mouseDown/mouseDragged/mouseUp` 驱动。移动路径不能：

- 查询 Finder 或桌面文件；
-读取粘贴板；
- 触发 `draggingEntered`；
- 创建 drag generation；
- 调用 Rust 导入；
- 播放文件卡片。

外部文件拖放由 `NSDraggingDestination` 驱动。拖放进行时黑洞位置固定在指针下的
目标位置，不自动漂移。两个输入路径共享视觉窗口但不共享触发条件。

## 8. 安全拖放和 Rust 回执

### 8.1 原生第一次验证

`draggingEntered`/`draggingUpdated` 从 `NSPasteboard` 读取 file URL 列表，并按原顺序
选择第一个满足以下条件的候选：

- 绝对本地文件 URL；
- 扩展名为 `.gcode.3mf`、`.3mf` 或 `.gcode`；
- `lstat` 表明它是普通文件；
- 不是符号链接；
- 拖放位置位于命中圆内。

原生层为有效候选创建单调递增的 `u64 generation`，保存规范化路径字符串、文件
类型和放下点。此时只显示 hover，不读取文件内容。

### 8.2 放下时第二次验证

`performDragOperation` 必须重新读取 pasteboard、按相同规则重新选择候选，并确认：

- generation 仍是当前会话；
- 路径与 `draggingEntered` 保存的路径逐字节一致；
- 候选仍是普通文件且不是符号链接；
- 放下点仍位于命中圆内。

任一条件失败则取消会话并返回 `NO`。成功时进入 `ImportPending`，把
`{ generation, path }` 发送给 Rust，并返回 `YES`。不能在这个阶段删除、移动或
写入源文件。

### 8.3 Rust 最终验证与回执

Rust 收到事件后：

1. 使用 `symlink_metadata` 再次拒绝符号链接和非普通文件。
2. 记录只读稳定性指纹：规范路径、长度、修改时间。
3. 调用现有 `PrintService::import_print_file`；若需要，继续使用现有
   `confirm_new_print` 语义。
4. 在写入解析缓存和任务事务前再次读取稳定性指纹。长度或修改时间发生变化时
   返回现有稳定错误码 `file_not_stable`，不能写入或确认任务。
5. 成功持久化待结算任务并取得 `job_id` 后，向原生层回传
   `{ generation, accepted }`。
6. 任意错误向原生层回传 `{ generation, rejected }`，主界面/系统通知继续显示
   稳定错误码。

只有 accepted 回执可以从 `ImportPending` 进入 `Swallow`。`performDragOperation`
返回 `YES` 不等于导入成功。

源文件不变由测试中的内容 SHA-256、长度和修改时间共同证明。应用不能为了测试
而主动恢复源文件时间戳；如果它没有写入，三个值自然不变。

## 9. Reduce Motion、轻量模式和帧率

### 9.1 Reduce Motion

读取 macOS“减少动态效果”后：

- 黑洞透镜保持静态可见，冻结盘面风噪、轨道和空间漂移。
- hover 只在 `0.10 s` 内提高亮度，不缩放。
- `ImportPending` 使用静止文件卡片。
- 成功后文件卡片在 `0.15 s` 内淡入中心，不旋转、不拉伸、不碎裂。
- 成功只显示 `0.15 s` 暖白中心脉冲，不显示喷流和扩散冲击环。
- 失败只显示 `0.15 s` 红色颜色脉冲，不显示外扩波纹。
- 结算继续使用现有 `0.15 s` 绿色颜色脉冲。

### 9.2 轻量模式

轻量模式停用 ScreenCaptureKit，但必须保留：

- BlackHoleTrash `Gargantua` 事件视界和吸积盘外观；
- 文件 hover、ImportPending、完整吞噬、失败和结算状态；
- 待结算轨道点；
- 圆形命中和安全拖放。

轻量模式不绘制桌面纹理或假桌面截图。黑洞以透明背景合成。Metal 不可用时，
Core Animation 降级至少保留黑色中心、暖金盘面、通用文件卡片、成功/失败颜色
反馈；不要求 CA 复现测地线。

### 9.3 FPS

- `auto`：`Idle` 30 FPS；
  `ExternalHoverValid/ImportPending/Swallow/ImportRejected/SettlementPulse`
  60 FPS；`Hidden` 0 FPS。
- `30`：所有可见状态固定 30 FPS。
- `60`：所有可见状态固定 60 FPS。
- 睡眠、隐藏和销毁时停止 display link、ScreenCaptureKit 和动画时间推进。
- 唤醒后重新枚举显示器、权限和捕获映射；未完成的拖放会话被取消并回到 Idle。

## 10. 保留的现有业务行为

- `.gcode.3mf`、已切片 `.3mf` 与 `.gcode` 仍由当前 parser 最终判断。
- 未结算的重复文件继续打开已有待结算任务。
- 已结算文件再次导入时继续使用现有“确认新打印”逻辑。
- 导入只建立任务和解析缓存，不扣料。
- 成功、失败、取消及完成百分比继续由主程序结算。
- 待结算轨道点数量继续与未结算任务一一对应。
- 主窗口、菜单栏、设置、单实例和设备设置不因视觉重写而改变。

## 11. 隐私和日志

- 屏幕帧只在内存/GPU 中存在，不保存、不上传、不写数据库、不进入 JavaScript。
- 文件路径只在原生回调和 Rust 导入线程中存在，不写普通日志。
- 文件卡片不含文件名、缩略图或内容。
- 错误日志只记录稳定错误码。
- 不引入新的网络请求。

## 12. 第三方许可

直接移植 BlackHoleTrash WGSL、成功喷流或相关数学时，必须在
`THIRD_PARTY_NOTICES.md` 加入：

- 项目 URL 和固定 commit；
- MIT License 全文；
- `Copyright (c) 2026 GreenScreen410`；
- 本项目把 WGSL 移植到 MSL、把回收成功替换为导入成功的修改说明。

直接改编 blackhole-mac 的 `Faller`、`Impacts` 或窗口交互数学时，还必须加入：

- 项目 URL 和固定 commit；
- MIT License 全文；
- `Copyright (c) 2026 Jack Zhang`；
- 改编文件卡片与动画状态但不复制可选废纸篓功能的说明。

旧 `blackhole-timer`/`ghostty-blackhole` 声明可以保留历史说明，但新的
`shader.metal` 头注释和产品文档必须明确 BlackHoleTrash 是当前规范来源。不能再
声称新的光学实现以当前彩色圆环或混合方案为目标。

## 13. 测试和视觉验收

### 13.1 自动测试

- 中央、四个屏幕边缘和 Retina 2× 的捕获 UV 映射。
- 120/220/360 px 的阴影半径与圆形命中边界。
- 命中方形四角返回 false。
- 测地线近场改变棋盘格采样位置，远场衔接没有硬边。
- 事件视界黑色、暖色盘面、上下透镜弧和透明窗口四角。
- Shader 不再包含 `spectral_ring`。
- 4.6 s Faller 的阶段边界和 `u = 0.82` 单次交付。
- 0.90 s absorption jet 的开始、中点和完成。
- Reduce Motion 的 0.15 s 单次淡入，不产生碎片或喷流。
- stale generation、路径变化、拖放位置离开中心、符号链接和目录被拒绝。
- 多文件只选择第一个受支持普通文件。
- 成功和失败前后源文件 SHA-256、长度、修改时间不变。
- 成功回执前不越过视界；失败回执不播放吞噬。
- 移动黑洞不产生任何导入事件。
- 导入成功不修改耗材余额；结算仍是唯一扣料入口。
- Auto/30/60、隐藏、睡眠和唤醒状态的帧率与生命周期。

### 13.2 人工视觉验收

- 对照 BlackHoleTrash 固定 commit 的 `Gargantua` 默认效果和用户截图。
- 使用带文字和图标的桌面背景检查连续引力弯折，不能只看到圆形放大。
- 看到黑色事件视界、暖白金盘面、上下透镜弧和远近场无接缝。
- 看不到当前实现的青/紫/玫红彩虹环。
- 文件成功后完整经历接近、拉伸、碎裂、并盘、越界和 0.90 s 喷流。
- 解析较慢时文件停在视界外；失败时回退。
- 在 Finder 文件上拖动黑洞不读取、不选择、不导入。
- 将多个文件拖入时只导入第一个受支持普通文件，其余源文件保持不变。
- 在两块显示器、屏幕四边、1×/2× scale、30/60/Auto 和 Reduce Motion 下检查。
- 真实模式权限拒绝或 Metal 失败后，轻量模式仍能安全导入。

## 14. 完成标准

- 产品视觉以固定版本 BlackHoleTrash `Gargantua` 为基准，不再是彩色圆环或
  blackhole-timer 混合外观。
- 文件只在主动拖入中央圆并完成两次原生验证与一次 Rust 验证后导入。
- Rust 成功回执前不能完成吞噬；失败不能伪装成成功。
- 成功动画包含 4.6 s 完整 Faller 和 0.90 s absorption jet。
- 源文件在所有成功、失败、重复和多文件场景下保持不变。
- 多屏、边缘捕获、FPS、Reduce Motion、轻量模式和 Metal 降级通过自动与人工
  验收。
- 现有解析、待结算、结算和库存账本行为保持不变。
- MIT 版权和修改说明完整。
