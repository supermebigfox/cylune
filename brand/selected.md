# CYLUNE 品牌选择记录

## 最终选择

- 品牌名：**CYLUNE**
- 状态：用户最终选定，停止继续探索其他 Logo 造型。
- 原始生成文件：`/Users/robin/.codex/generated_images/019fa6da-96dc-7b30-938e-b376063c7a6b/call_K75xQpLRVjtibpqMIVQ9C05X.png`
- 原图 SHA-256：`8e6c9d34f7b6d21a6074fa2ff857778a5b9235b14d881f706f4e95662532e500`

## 文件

- `cylune-master.png`：原始选定文件的逐字节副本，包含 CYLUNE 字标；没有重绘或修改。
- `cylune-mark.png`：从同一原图裁出的纯图形版本，保留白底，去掉下方 CYLUNE 文字；没有重绘符号。
- `cylune-mark-transparent.png`：由 `cylune-mark.png` 做白底反合成得到的透明背景版本；保留原图颜色与线束结构。
- `cylune-menu-template.png`：由透明版本直接派生的 64×64 单色 alpha 轮廓预览，供 macOS 菜单栏适配评估。

## 菜单栏限制

`cylune-menu-template.png` 是忠实的栅格轮廓派生文件，不是重新绘制的矢量 Logo。64px 下开口、中心轴和总体轮廓清晰；压到约 16px 时，原图的多条细线会自然合并成较厚轮廓，但整体符号仍可识别。若后续要求在 16px 仍逐条保留线束，需要基于选定图形另做人工光学校正的 SVG / macOS Template Image，不能从当前栅格无损恢复。
