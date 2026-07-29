# 耗材目录回归验证

验证日期：2026-07-29

验证分支：`feature/macos-local-prototype`

验证范围：离线拓竹耗材目录、卷元数据迁移与备份、跨预设匹配、原厂卷添加流程、创建失败时的模态内可访问反馈，以及真实 `.gcode.3mf` 解析与结算回归。

本记录独立于 `docs/qa-black-hole.md`。本次目录工作没有修改桌面黑洞的外观、动画、捕获、拖放、导入、待处理反馈或结算行为；黑洞既有 QA 状态也未改写。

## 1. 目录来源与结构

离线快照为 `src/catalog/bambu.json`，生成源为本机 Bambu Studio：

- Bambu Studio 源版本：`02.08.00.50`
- 源类型：45
- 颜色条目：306
- 快照长度：150,005 字节
- 快照 SHA-256：`9e99befb5a4603bf7fc4b28032487da7e6ad1dcdb0f8a3977bb773c76e099b82`

重新生成命令：

```bash
node scripts/catalog.mjs \
  '/Applications/BambuStudio.app/Contents/Resources/profiles/BBL/filament/filaments_color_codes.json' \
  '/Applications/BambuStudio.app/Contents/Resources/profiles/BBL/filament'
git diff --exit-code -- src/catalog/bambu.json
```

结果：两个命令均退出 0，重新生成后的跟踪文件无 diff，确认快照生成具有确定性。

目录驱动的建卷流程为“材料 → 系列 → 颜色”。保存时写入独立卷的目录 ID、品牌、材质、系列、无 ` @...` 后缀的预设基名、官方颜色名与代码、主颜色及完整多色数组；相同目录颜色仍创建不同的 `spool_id`。

## 2. 数据库、备份与匹配

`spools` 新增以下五个可空列：

- `catalog_id`
- `color_name`
- `color_code`
- `color_hexes`
- `preset_base`

业务 JSON 备份 schema version 为 2，包含五个字段；version 1 备份仍可恢复，缺失目录元数据保留为空，缺失、无效或空的多色数组回退到既有 `color_hex`。

导入切片后，候选卷按以下顺序匹配，并在前一层有结果时停止：

1. exact：`preset_id + material + series + color_hex`
2. base：当前 3MF 和已存值都去掉 ` @...` 后使用 `preset_base + material + color_hex`；因此早期 v2 备份恢复的 `preset_base = "... @base"` 仍在本层命中
3. legacy：仅 `preset_base IS NULL` 的卷使用 `material + series + color_hex`

唯一候选可建议；同层多个候选必须选择具体实体卷。

## 3. 完整自动化回归

### 前端

```bash
npm test -- --run \
  src/features/spools/Add.test.tsx \
  src/features/spools/Spools.test.tsx \
  src/App.test.tsx
```

结果：退出 0；3 个定向测试文件通过，54 个测试通过，0 失败。其中真实
装配测试从 `DesktopApp` 导航到 `Spools`，打开 portal 中的 `Add`，让
`create_spool` 失败，并确认：

- dialog 保持打开，官方颜色选择、自定义名称和克数均保留；
- 本地化 `role="alert"` 位于 dialog 内，不在任何 `[inert]` 祖先下；
- 再次保存时旧提示先清理，并在新失败到达后更新；
- 关闭后保留既有全局错误与重试入口，但重新打开 Add 不显示上一会话的
  旧创建错误；
- 请求仍在进行时关闭 dialog，晚到失败只更新全局错误，不会写回已失效的
  Add 会话或污染下一次打开。
- 请求仍在进行时通过关闭按钮、取消按钮或 Escape 关闭，材料与系列保留，
  但颜色、搜索、自定义名称和克数重置；晚到成功不能触碰新会话，重新打开
  后保存保持禁用，不能重复提交旧卷。

```bash
npm test -- --run
```

结果：退出 0；15 个测试文件通过，135 个测试通过，0 失败。

```bash
npm run build
```

结果：退出 0；TypeScript 编译和 Vite 生产构建成功，4,594 个模块完成转换。

### 原生

```bash
cd src-tauri
cargo fmt -- --check
```

结果：退出 0，无格式差异。

```bash
cd src-tauri
cargo test
```

结果：退出 0；库测试 150 个通过、0 失败、1 个 ignored；该 ignored 项为需要用户真实文件环境变量的 smoke test。主程序与文档测试均为 0 个测试、0 失败。

覆盖结果包括：五列目录迁移、目录元数据持久化、schema version 2 备份往返、version 1 兼容恢复、多色回退，以及 exact → base → legacy 匹配优先级。

## 4. 用户真实切片文件

命令：

```bash
cd src-tauri
BAMBU_SMOKE_3MF='/Users/robin/Desktop/叠色/萨莫面具-布莱克.gcode.3mf' \
  cargo test smoke_real_sliced_file_from_environment -- --ignored --nocapture
```

结果：退出 0；1 个 ignored smoke 被显式运行并通过，0 失败。解析得到 4 个工具，`real_file_layers=14`：

| 工具 | 槽位 | 颜色 | 预设 | 完整用量（克） |
| --- | ---: | --- | --- | ---: |
| 0 | 1 | `#FFFEFC` | `Bambu PLA Basic @BBL A1` | 44.900496 |
| 1 | 2 | `#FE3D36` | `Bambu PLA Basic @BBL A1` | 15.201414 |
| 2 | 3 | `#1C4EBB` | `Bambu PLA Basic @BBL A1` | 31.323133 |
| 3 | 4 | `#FFFD0D` | `Bambu PLA Basic @BBL A1` | 24.388558 |

导入前四卷余额均为正，只有四条建卷基线流水且待处理数为 0。导入后余额和流水保持原值，待处理数变为 1；四个工具均匹配到各自唯一实体卷。目录元数据迁移没有改变导入扣减边界或耗材换算。

结算摘要：

| 路径 | 选定层 | 置信度 | 四卷合计（克） | 结果 |
| --- | ---: | --- | ---: | --- |
| 成功 | 完整文件 | Exact | 115.813601 | 4 条消耗均为正；重复结算幂等；反向流水完整恢复；第二次反向不重复恢复 |
| 失败 | 6 | Exact | 76.669966 | 4 条消耗均为正并改变余额 |
| 取消 | 6 | Exact | 76.669966 | 4 条消耗均为正并改变余额 |
| 进度 50% | 7 | Estimated | 92.291482 | 4 条估算消耗均为正并改变余额 |

源文件只读完整性检查：

| 检查 | 运行前 | 运行后 |
| --- | --- | --- |
| 长度 | 4,662,275 字节 | 4,662,275 字节 |
| SHA-256 | `1f1614e6092de69de08cee99b7c45d9d59c37aead47a49e5754113c1433ee9d4` | `1f1614e6092de69de08cee99b7c45d9d59c37aead47a49e5754113c1433ee9d4` |

长度和哈希均未改变，真实文件没有被写入。
