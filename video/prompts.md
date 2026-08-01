# CYLUNE Seedance 2.0 Prompt 包

## 为什么只生成两个镜头

CYLUNE 的核心卖点建立在真实软件行为上。视频模型很容易重绘界面、写错中文、改变 Logo 或把黑洞变成普通圆形透镜，因此只将它用于两段不存在可读 UI 的抽象镜头：

- **Shot 04**：真实黑洞射流进入冰白空间。
- **Shot 11**：窗口内彩纸转化为光纤并聚合到 Logo。

Shot 01–03、05–10、12 均使用真实录屏、现有品牌资产和剪辑软件完成。

## 统一生成设置

| 设置 | 选择 |
|---|---|
| 主模型 | Seedance 2.0 |
| 模式 | 全能参考 / multimodal reference-to-video |
| 画幅 | 16:9 |
| 单次时长 | 5 秒 |
| 分辨率 | 选择当前入口提供的最高原生档位；成片阶段再统一到 4K |
| 生成音频 | 关闭；母版声音后期制作 |
| 初始版本 | 每个镜头 4 个 |
| 定向重试 | 选定方向后最多 2 个 |
| 交付帧率 | 不强制模型帧率；选片后转入 30fps 时间线 |

官方资料显示 Seedance 2.0 支持文字、图片、视频和音频混合参考，单次可输入最多 9 张图片、3 段视频与 3 段音频，并支持最长 15 秒的多镜头输出。本项目不使用它生成 70 秒长片，而是把每个动作限制在独立 5 秒镜头中，以获得更高稳定性。

## 平台操作模式

### 即梦 AI

1. 打开“视频生成”。
2. 模型选择 `Seedance 2.0`。
3. 选择“全能参考”，不要选择单纯“文生视频”。
4. 按下面每个镜头的素材映射上传文件。
5. 将上传素材按界面实际编号对应到 Prompt 中的 `@视频1`、`@图片1` 等引用。
6. 选择 16:9、5 秒、最高质量档，关闭自动配乐或原生音频。
7. 每个镜头先生成 4 个版本，不要同时改变 Prompt 和素材。

### 豆包

1. 进入 Seedance 2.0 视频创作入口。
2. 选择 2.0 模型与全能参考能力。
3. 上传相同的图片和视频参考。
4. 如果界面没有独立负面 Prompt，将“负面约束”整段追加在主 Prompt 末尾。

## 全局负面约束

下列内容追加在两个生成镜头的 Prompt 末尾；如果平台提供独立 Negative Prompt 输入框，则放入该输入框：

### 中文

```text
禁止人物、手、脸、文字、字母、数字、字幕、水印、品牌名和虚构界面。禁止重新设计或重绘 CYLUNE 标志，禁止生成第二个黑洞，禁止规整圆形透镜边界、同心圆水波、横向冲击波、爆炸、烟雾、火焰、闪电、赛博朋克城市、霓虹招牌和故障艺术。禁止摄像机抖动、手持感、突然变焦、无目的环绕、随机景别切换、时间跳跃、物体复制、粒子瞬移、闪烁、边缘撕裂、颜色漂移、过曝光晕和低清模糊。画面中不出现任何可读 UI。
```

### English

```text
No people, hands, faces, words, letters, numbers, captions, watermarks, brand names, or invented interfaces. Do not redesign or repaint the CYLUNE mark. No second black hole, hard circular lens boundary, concentric water ripples, lateral shockwave, explosion, smoke, fire, lightning, cyberpunk city, neon signage, or glitch art. No handheld shake, sudden zoom, purposeless orbit, random shot changes, time jumps, duplicated objects, teleporting particles, flicker, torn edges, color drift, excessive bloom, or low-resolution blur. No readable UI anywhere in the frame.
```

---

# Shot 04｜黑洞射流进入冰白空间

## 目标

生成 5 秒素材，剪辑时使用前 4 秒。首端必须无缝延续真实黑洞吸入后的射流，尾端进入干净冰白环境，为真实 CYLUNE 窗口出现提供背景。模型不生成软件窗口。

## 上传素材

按以下顺序上传，保持文件编号稳定：

| 引用 | 文件 | 用途 |
|---|---|---|
| `@视频1` | `BH-04-jet-ref.mov` | 从真实 Shot 03 截取的最后 1.5–2 秒，包含完整射流；参考动作、方向和黑洞真实外观 |
| `@图片1` | `BH-04-exit.png` | `@视频1` 最后一帧；锁定光束出发位置、画面构图和颜色 |
| `@图片2` | `../brand/poster-final-4k.png` | 只参考冰白、雾蓝、玻璃与柔和虹彩，不参考其中的文字排版 |
| `@图片3` | `../brand/cylune-mark.png` | 只参考钴蓝、电紫和少量珊瑚红的能量色，不在画面中生成 Logo |

不要上传软件界面截图。尾端只需要空白冰白空间，真实窗口在后期加入。

## 中文主 Prompt

```text
创作一段 5 秒、16:9 的高级科技产品转场，单一连续镜头。严格延续 @视频1 中真实黑洞射流最后一刻的运动方向、速度和空间关系，以 @图片1 作为准确首帧构图。0–1.2 秒：一束由数十根细密光纤组成的钴蓝至电紫射流继续向画面纵深延展，射流窄而集中，保留少量珊瑚色高光，真实黑洞只短暂留在画面后方，不改变它的轮廓、旋转方向或外观。1.2–3.8 秒：摄像机沿光纤轴线平稳向前推进，深色桌面逐渐被受控、半透明的磨砂玻璃薄片遮蔽，环境从深空黑自然过渡为冰白与雾蓝，光线柔和，带克制的虹彩折射，材质精密、真实、安静。3.8–5 秒：摄像机进入一个宽阔、纯净的冰白空间，前方只出现一块正面的、无文字无界面的透明玻璃平面轮廓，光纤逐渐减速并在画面边缘消散。镜头运动始终稳定、轴向、连续，不爆炸，不切镜，不旋转镜头。高端消费电子产品影片质感，真实光学，清晰细节，克制留白。
```

## English Prompt

```text
Create a five-second, 16:9 premium technology product transition in one continuous shot. Continue precisely from the final real black-hole jet in @Video 1, preserving its direction, velocity, and spatial relationship, with @Image 1 as the exact opening composition. From 0 to 1.2 seconds, a narrow and controlled jet made of dozens of fine cobalt-blue to electric-violet optical filaments extends forward into depth, carrying only a tiny coral highlight. The real black hole remains briefly behind the jet and its shape, clockwise motion, and appearance do not change. From 1.2 to 3.8 seconds, the camera performs a smooth axial dolly forward along the filaments. The dark desktop is gradually occluded by controlled translucent frosted-glass sheets, and the environment naturally transforms from deep black into ice white and mist blue, with soft restrained iridescent refraction. From 3.8 to 5 seconds, the camera enters a spacious clean ice-white environment. Only the subtle outline of one front-facing transparent blank glass plane is visible; it contains no text and no interface. The filaments decelerate and dissolve near the frame edges. Keep the camera stable, axial, and continuous, with no explosion, cut, or camera roll. Premium consumer-electronics launch-film aesthetic, physically plausible optics, crisp detail, quiet negative space.
```

## Shot 04 专用负面约束

```text
不要生成应用窗口、电脑、手机、产品外壳、按钮、卡片、文字或 Logo。不要把射流变成激光炮、火焰、闪电或爆炸。不要产生巨大的圆形传送门、硬边界透镜、同心水波或横向扩散冲击波。不要新增、复制或放大黑洞，不要改变黑洞为逆时针。尾帧必须明亮、空旷、正面、可供后期放置真实软件窗口。
```

## 首轮选择标准

- 首 12 帧与真实射流方向一致，没有明显跳帧。
- 黑洞没有复制、变形或改变旋转方向。
- 光纤是受控的细束，不是爆炸和闪电。
- 暗到亮的过渡连续，没有突然白闪。
- 尾端有足够干净的冰白留白，不存在虚构 UI。

## 定向重试 A｜能量过强

保持原素材与主 Prompt，只在末尾追加：

```text
降低射流能量和画面曝光约三分之一。光纤更细、更平行、更精密，光晕紧贴线束，不出现大面积泛白。玻璃薄片只在侧逆光下可见，整体安静、克制、留白充足。
```

## 定向重试 B｜推进感不足

保持原素材与主 Prompt，只在末尾追加：

```text
增强摄像机沿射流轴线向前穿行的速度感，但保持稳定，不增加抖动、镜头旋转或黑洞尺寸。背景视差从弱到强，光纤从镜头两侧掠过，3.8 秒前必须完整进入冰白空间。
```

## 编辑回退

如果两次重试后模型仍改变真实黑洞：放弃模型生成的开头，只保留它最干净的冰白尾段。在 DaVinci Resolve 或 After Effects 中用真实射流末帧制作方向模糊、Glow 和 10–14 帧的光束擦除，直接连接生成尾段。不要接受一个外观错误的新黑洞。

---

# Shot 11｜彩纸化为光纤并聚合

## 目标

生成恰好 5 秒的蓝紫光纤聚合素材。起点连接真实窗口内庆祝动画，终点接近 CYLUNE 图形标志，但最终 8 帧必须由后期切换到现有透明 Logo 母版，保证标志准确。

## 上传素材

| 引用 | 文件 | 用途 |
|---|---|---|
| `@视频1` | `UI-10-success-tail.mov` | Shot 10 最后 1.5–2 秒，只保留窗口内部的彩纸动作；参考运动节奏 |
| `@图片1` | `UI-10-success-last.png` | 真实庆祝动画末帧；锁定起始彩纸位置和颜色 |
| `@图片2` | `../brand/cylune-mark.png` | 参考最终图形轮廓、线束方向与颜色，不参考白底以外的任何元素 |
| `@图片3` | `../brand/poster-final-4k.png` | 参考冰白环境、柔和虹彩和终章灯光 |

如果 `@视频1` 中界面文字过多，先在剪辑软件中将窗口外和 UI 内容遮成深蓝灰，仅保留彩纸粒子，再作为视频参考上传。

## 中文主 Prompt

```text
创作一段 5 秒、16:9 的高级科技品牌聚合动画，单一连续镜头。以 @图片1 的彩纸分布和 @视频1 的运动节奏作为准确起点，只参考 @图片2 中 CYLUNE 图形标志的整体轮廓、顺时针线束方向和钴蓝至电紫配色，背景与灯光参考 @图片3。0–1 秒：画面中的蓝色、紫色和少量珊瑚色彩纸自然减速，窗口和界面在柔和暗蓝中消失，彩纸位置连续、不跳变。1–3.6 秒：每一片彩纸平滑拉伸为一根极细、清晰、半透明的光纤，所有光纤顺时针旋转并被中心引力吸引，由散乱逐渐变得平行、有序，钴蓝在上方、电紫在下方，只有极少珊瑚色高光。3.6–5 秒：光纤在冰白背景中收紧为接近 @图片2 轮廓的开放式环形线束，运动逐渐减缓，在最后 0.4 秒稳定停留，为后期替换成真实 CYLUNE 标志预留画面。保持正面中心构图，摄像机仅有极轻的推进，不环绕、不摇晃、不切镜。材质像透明光纤和精密玻璃，不是烟雾，不是液体，不是火焰。画面无文字、无字母、无界面、无品牌名。
```

## English Prompt

```text
Create a five-second, 16:9 premium technology brand-convergence animation in one continuous shot. Use the confetti distribution in @Image 1 and the motion rhythm in @Video 1 as the exact starting point. From @Image 2, reference only the overall open orbital silhouette, clockwise filament direction, and cobalt-blue to electric-violet palette of the CYLUNE mark. Use @Image 3 for the ice-white environment and restrained iridescent lighting. From 0 to 1 second, the blue, violet, and tiny amount of coral confetti decelerates naturally while the application window and interface disappear into soft deep blue; particle positions remain temporally continuous with no jumps. From 1 to 3.6 seconds, every confetti piece stretches smoothly into one extremely fine, crisp, translucent optical filament. All filaments spiral clockwise and are drawn toward the center, evolving from scattered motion into parallel order, with cobalt blue above, electric violet below, and only minimal coral highlights. From 3.6 to 5 seconds, the filaments tighten against an ice-white background into an open orbital bundle approaching the silhouette of @Image 2. Motion gradually settles and holds during the final 0.4 seconds so the editor can replace it with the authentic CYLUNE mark. Keep a centered front-facing composition with only a subtle camera push, no orbit, shake, or cut. The material is transparent optical fiber and precision glass, not smoke, liquid, or flame. No words, letters, interface, or brand name.
```

## Shot 11 专用负面约束

```text
禁止生成 CYLUNE 字样或任何可读文字。禁止把参考标志闭合成普通圆环、Chrome 图标、眼睛、唱片、光盘或行星。禁止修改线束开口方向、增加对称叶片、复制中心圆、改变为逆时针或添加额外 Logo。禁止彩纸爆炸、随机乱飞、液体飞溅、烟雾、火焰、星云和大面积光晕。禁止背景残留软件界面、窗口边框、按钮或中文。
```

## 首轮选择标准

- 前 12 帧能够和真实彩纸末帧自然衔接。
- 彩纸先减速再拉伸，没有突然消失或瞬移。
- 光纤明确顺时针向中心聚合，不是向外爆发。
- 背景逐渐清理为冰白，没有残留虚构 UI。
- 末端接近 Logo 轮廓但没有错误字标，便于后期替换。

## 定向重试 A｜标志变形

保持主 Prompt，但删除“接近 @图片2 轮廓”的要求，改为追加：

```text
不要尝试生成任何完整标志。所有光纤只需顺时针收束到画面中心附近，形成一个仍未完成的开放式弧形线束，并在最后 0.6 秒停住。最终准确标志由后期加入。
```

## 定向重试 B｜粒子过乱

保持原素材与主 Prompt，只在末尾追加：

```text
将粒子数量减少一半，所有轨迹平滑且可追踪。每根光纤只完成一次顺时针弧线运动，避免交叉、反弹、瞬移和向外扩散。运动从自由到有序，幅度逐渐减小。
```

## 编辑回退

如果模型连续两次无法保持 Logo 形状，不再让模型生成终态。只使用它的彩纸到光纤转换，在 01:04.4 截断；后期制作光纤向中心收束的方向模糊，并以 12–16 帧 Luma Matte 显现 `cylune-mark-transparent.png`。这是正式方案，不是质量降级，因为最终 Logo 必须来自品牌母版。

---

# 备用模型适配

只有在当前账户无法使用 Seedance 2.0 时才启用备用模型。

## Runway

- 选择当前最高质量的 image-to-video 或 references 工作流。
- Shot 04 使用 `BH-04-exit.png` 作为首帧；Shot 11 使用 `UI-10-success-last.png` 作为首帧。
- 只粘贴对应英文 Prompt；把素材引用改为“the input image”或“the reference mark”。
- 生成 5 秒、16:9；关闭任何自动字幕。
- Runway 不能稳定参考最终 Logo 时，直接执行编辑回退，不继续烧生成次数。

## Kling

- 选择当前最高质量的图生视频或多图参考模式，不使用仅追求速度的极速档。
- Shot 04 以真实射流末帧为首帧，冰白海报作为风格参考。
- Shot 11 以真实彩纸末帧为首帧，CYLUNE 图形作为主体参考。
- 动态强度选择中等；镜头运动由 Prompt 控制，不开启随机运镜模板。
- 生成 5 秒、16:9；两次失败后执行编辑回退。

# 生成文件命名

每个生成结果保留原始版本，不覆盖：

```text
SD20-S04-v01.mp4
SD20-S04-v02.mp4
SD20-S04-v03.mp4
SD20-S04-v04.mp4
SD20-S04-retry-energy.mp4
SD20-S04-retry-motion.mp4

SD20-S11-v01.mp4
SD20-S11-v02.mp4
SD20-S11-v03.mp4
SD20-S11-v04.mp4
SD20-S11-retry-logo.mp4
SD20-S11-retry-particles.mp4
```

选中的版本不改名，在剪辑项目中使用颜色标签或评分标记。这样可以始终追溯具体 Prompt 和生成结果。
