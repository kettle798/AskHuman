# 开发计划：钉钉文本类附件可靠预览（内联 / 转 docx）

> 关联需求：`docs/specs/dingtalk-attachment-preview.md`（含全部已确认决策 D1–D18）
> 计划描述方案与技术/规则细节，具体代码以实现为准。

## 0. 方案总览

```
DingTalkSession::send_message_prompt  (channels/dingding.rs)
  对 message.files 中每个文件：
    is_image ────────────────────────────────▶ 现状：sampleImageMsg
    非图片 ─▶ route_text_attachment(file, cfg)
                │
                ├─ 扩展名∉文本清单  或  内容非UTF-8  或  开关全关 ─▶ 现状：send_attachment(原样 sampleFile)
                │
                ├─ 文本 且 字符数≤3000 且 开关①(inline)开 ─▶ send_inline(sampleMarkdown: header+hr+内容)  [不发文件]
                │
                └─ 文本 且 (长 或 开关①关) ─▶ 开关②(convert)开 ? send_docx(转docx) : send_attachment(原样)
                                                                            │
                                                                    失败兜底:静默退回 send_attachment(原样)
```

要点：判定与分支逻辑集中在新模块；docx 生成（OOXML+zip、markdown→OOXML、代码→OOXML）独立成模块；钉钉端不内置字体、不做语法高亮。

---

## 1. 触发与判定（D2/D3）

### 1.1 文本类扩展名清单（命中→进入文本处理）
小写化扩展名匹配。建议清单（实现时以常量集合维护，可后续增补）：

```
md markdown txt text log csv tsv
json json5 yaml yml toml ini conf cfg env properties
xml html htm css scss less svg(按文本)
js jsx ts tsx mjs cjs vue svelte
py rb go rs java kt kts scala
c h cpp cc cxx hpp hh cs m mm swift
php pl lua dart r sql graphql gql proto
sh bash zsh fish ps1 bat
gradle dockerfile makefile mk cmake
```

无扩展名文件：不进入文本处理（按现状发送），保持简单可预期。

### 1.2 排除（不处理，按现状发送）
- 钉钉白名单可预览类型：`pdf doc docx xls xlsx ppt pptx zip rar`。
- 图片：沿用 `is_image` 分支。
- 命中文本清单但**内容非合法 UTF-8**：判为非文本，按现状发送（也是 D16 的一种兜底）。

### 1.3 短/长判定（D5）
读取文件为字符串后，按 **Unicode 字符数**（`chars().count()`）与阈值 **3000** 比较：`≤3000` 短，`>3000` 长。

---

## 2. 分支与开关（D4/D6/D7/D9/D16）

两个开关来自钉钉配置（默认均 true）：`inlineSmallText`、`convertTextToDocx`。

| 文件 | inline① | convert② | 行为 |
|---|---|---|---|
| 文本·短 | 开 | * | 内联（不发文件） |
| 文本·短 | 关 | 开 | 转 docx |
| 文本·短 | 关 | 关 | 发源文件 |
| 文本·长 | * | 开 | 转 docx |
| 文本·长 | * | 关 | 发源文件 |
| 非文本 | * | * | 发源文件（现状） |

兜底（D16）：读取/转换/上传任一步失败 → 退回 `send_attachment` 发送源文件；**不打印任何警告**。

---

## 3. 内联渲染（D8/D9）

经 `client.send_oto_markdown(title, text)`（`sampleMarkdown`）单条消息，每个短文本文件一条。

`text` 结构：

```
**{文件名} · {大小} · {行数} 行**

---

{内容区}
```

- **header 行**：加粗。大小格式：`<1024B` 用 `N B`，`<1MB` 用 `N.N KB`，否则 `N.N MB`；行数 = 换行数 + 1。
- **分割线**：`---`（钉钉渲染为水平分隔线）。
- **内容区**：
  - `.md/.markdown`：原始 markdown 原文（钉钉已实测可渲染标题/粗体/列表/表格/带语言代码块+高亮）。
  - 非 md：围栏代码块，带**语言标识**（按 §6 扩展名→语言映射；未知映射用扩展名原文）：

    ```
    ```{lang}
    {文件原文}
    ```
    ```

- `title` 参数填文件名即可（钉钉消息标题，非正文）。

---

## 4. docx 生成（D10–D15）

### 4.1 打包结构（zip / OOXML）
最小 docx = zip，固定包含：

```
[Content_Types].xml
_rels/.rels
word/document.xml
word/_rels/document.xml.rels
word/styles.xml
word/numbering.xml
```

`styles.xml` / `numbering.xml` 为固定模板（见 §5）；`document.xml` 由内容动态拼装。命名（D10）：上传与 `sampleFile` 的 `fileName` 用 **源文件名 + ".docx"**，`fileType` 固定 `docx`。

### 4.2 两种渲染模式
- **Markdown 模式**（`.md/.markdown`，D11）：`pulldown-cmark` 解析为事件流 → 映射为 OOXML 段落/表格/代码框；**不加**文件名标题。
- **PlainCode 模式**（非 md，D12）：第一段为 **H1 = 文件名**；其后整文件放入**一个等宽代码框**（每行一个 Code 段落，保留缩进；`xml:space="preserve"`）。

### 4.3 Markdown→OOXML 元素映射（D15）
| Markdown | OOXML 处理 |
|---|---|
| H1–H6 | 普通段落 + run 直接 加粗+字号（见 §5；**不用命名标题样式**）；H4–H6 退化到 H3 |
| 段落 | Normal 段落，inline 富文本（粗/斜/行内代码）作为不同 run |
| 粗体/斜体 | run 上 `<w:b/>`/`<w:i/>` |
| 行内代码 | run 用 Courier New + 浅灰底（`shd fill EFF1F3`） |
| 无序/有序列表（含嵌套） | `numbering.xml` 的 bullet/decimal 定义；`numPr` + `ilvl` 表示层级（至少支持 2–3 级缩进） |
| 代码块 | 等宽代码框（单元格表格，底色 `F6F8FA`，无高亮），每行一个 Code 段落 |
| 引用 | Quote 样式（左竖条 `D0D7DE` + 灰字 `656D76`） |
| 表格 | 表格：细边框 `D0D7DE`、表头加粗、偶数行斑马底色 `F6F8FA`、单元格内边距 |
| 分隔线 | 段落底边框（`pBdr/bottom`）或空段 + 横线，呈现为分隔 |
| 链接 | 仅渲染链接**文字**（不渲染为可点击超链接） |
| 图片 | **跳过**；若有 alt 文字则以普通文字呈现 |

读取文件用 UTF-8（非法字节判为非文本，见 §1.2）。

---

## 5. docx 字体与样式（D13/D14，实测锁定值）

> 关键：钉钉渲染器**只认命名样式里的字体**、忽略 `docDefaults`；故所有字体写进命名样式，并让其它样式 `basedOn=Normal` 继承。

- **Normal**：`rFonts ascii/hAnsi/cs="Arial" eastAsia="SimHei"`；`color #1F2328`；`sz 24`（12pt）；段后 `after 240`、行距 `line 360 auto`（1.5）。
- **标题（H1–H6）**：⚠️**不用命名样式**。钉钉会把命名标题样式/带 `keepNext` 段落的字号抹平、相近字号倒置（详见 spec §7 2026-06-06 反馈）。改为「**普通段落 + run 上直接 加粗 + 字号**」：
  - 字号（half-point）：**H1=56(28pt)、H2=32(16pt)、H3 及更深=28(14pt)**（均落在钉钉"干净单调带"，H1 取高位大字）；H4–H6 退化到 H3。
  - 段落属性：段前/后距 `H1 before640/after240`、`H2 before600/after200`、`H3 before480/after160`；**H1/H2** 加下边框 `D8DEE4`（GitHub 风格下划线），H3 不加。
- **ListParagraph**：`basedOn Normal`；`ind left 640 hanging 360`；`after 60`。
- **Code**：`basedOn Normal`；`rFonts ascii/hAnsi/cs="Courier New" eastAsia="SimHei"`；`b`（提升分量，解决细/浅问题）；`color #1A1A1A`；`sz 21`（10.5pt）；`line 288`、`before/after 0`。
- **Quote**：`basedOn Normal`；左边框 `single sz24 color D0D7DE`；`color #656D76`；`ind left 480`。
- **代码框（表格）**：单元格底色 `F6F8FA`、无表格边框、单元格内边距（上下 120 / 左右 200 twips）；内部段落用 Code 样式，run 不带显式 rFonts（继承 Code 样式，已验证生效）。
- **数据表格**：`tblBorders` 全 `single sz4 color D0D7DE`；表头行加粗 + `tblHeader`；偶数数据行 `shd fill F6F8FA`（斑马）；单元格内边距（上下 80 / 左右 160）。

`numbering.xml`：abstractNum 0 = bullet（`•`），abstractNum 1 = decimal（`%1.`）；对应 `num 1`/`num 2`，缩进 `ind left 640 hanging 360`。

> 备注：以上数值已在 `scripts/dingtalk-docx-test.py` 验证过钉钉渲染效果（中英无衬线、代码等宽、标题层级与 GitHub 比例一致）；实现时以该脚本生成的 XML 为蓝本固化为 Rust 模板。

---

## 6. 扩展名 → 代码语言映射（内联代码块用）

用于内联围栏代码块的语言标识（钉钉据此高亮）。建议映射（未列出的用扩展名原文）：

```
rs→rust  py→python  js/mjs/cjs→javascript  ts→typescript  jsx→jsx  tsx→tsx
go→go  java→java  kt/kts→kotlin  c/h→c  cpp/cc/cxx/hpp/hh→cpp  cs→csharp
rb→ruby  php→php  swift→swift  m/mm→objectivec  sh/bash/zsh/fish→bash
sql→sql  yaml/yml→yaml  json/json5→json  toml→toml  ini/conf/cfg→ini
html/htm→html  css→css  scss→scss  less→less  xml/svg→xml  vue→vue  svelte→svelte
md/markdown→markdown  lua→lua  dart→dart  r→r  graphql/gql→graphql  proto→protobuf
dockerfile→dockerfile  makefile/mk→makefile  gradle→gradle  bat→bat  ps1→powershell
```

---

## 7. 配置与设置 UI（D6）

### 7.1 后端 `config.rs`
`DingTalkChannelConfig` 新增两布尔字段（`#[serde(default)]` 已在结构体上）：

- `inline_small_text: bool`
- `convert_text_to_docx: bool`

并在 `Default` 实现里**默认设为 `true`**（保证旧配置缺字段时反序列化得到 true）。同步更新 `config.rs` 的默认值测试。

### 7.2 前端类型与设置页
- `src/lib/types.ts` 的 `DingTalkChannelConfig` 增加 `inlineSmallText: boolean`、`convertTextToDocx: boolean`。
- `src/views/SettingsView.vue` 钉钉区（`v-if="...dingding.enabled"`）内，仿 `enabled` 复选框新增两个开关，`@change="persist"`。
- i18n（`src/i18n/zh.ts` / `en.ts`）在 `settings.channels` 下新增文案：
  - `inlineSmallText`：中「小文件内联到正文」/ 英「Inline small text files」
  - `convertTextToDocx`：中「文本文件转 docx 发送」/ 英「Send text files as docx」
  - 可各配一行说明（hint）。

---

## 8. 代码改动点（落地位置）

- 新增 `src-tauri/src/dingtalk/docx.rs`：
  - `build_markdown_docx(content: &str) -> Vec<u8>`（Markdown 模式）
  - `build_plaincode_docx(file_name: &str, content: &str) -> Vec<u8>`（PlainCode 模式）
  - 内部：OOXML 拼装助手 + 固定 `styles.xml`/`numbering.xml` 常量 + zip 打包。
- 新增 `src-tauri/src/dingtalk/textfile.rs`（或并入 dingding.rs）：
  - 扩展名清单/白名单/图片判定、UTF-8 读取、字符数阈值、扩展名→语言映射、大小/行数格式化、内联 `text` 拼装。
  - `route_text_attachment(client, cfg, path, name) -> RouteOutcome`：返回“已内联 / 已发 docx / 需回退原样”，由调用方据此决定是否再调 `send_attachment`。
- 修改 `src-tauri/src/channels/dingding.rs`：
  - `send_message_prompt` 的文件循环：非图片文件先走 `route_text_attachment`；其结果为“回退/不处理”时再调现有 `send_attachment`。
  - `client.send_oto_file` 在 docx 分支用 `fileType="docx"`、`fileName=源名+".docx"`（上传媒体仍用临时 docx 文件路径）。
- `src-tauri/src/dingtalk/mod.rs`：挂载新模块。
- `src-tauri/Cargo.toml`：新增依赖 `pulldown-cmark`、一个 zip 库（如 `zip`，仅启用 deflate 特性以控体积）。

docx 字节落地：写入临时文件（`std::env::temp_dir()`）后复用现有 `upload_media(path, "file")`；用后可删除。

---

## 9. 边界与容错

- 读取失败 / 非 UTF-8 / docx 生成失败 / 上传失败 → 静默退回 `send_attachment` 原样发送（D16，不打印警告）。
- 超大文本（理论上 docx 仍很小）：不特殊处理；若上传超 20MB 失败则按兜底退回原样。
- 内联消息发送失败：按兜底退回——改走 docx 或原样（实现时取“失败即发源文件”以最简，不重复刷屏）。
- 文件名含特殊字符：XML/JSON 转义；docx 文件名透传源名 + `.docx`。

---

## 10. 验收 / 自测

对照 spec §6 验收标准；Rust 侧对 `build_*_docx` 产物做“能被 zip 解出且包含必需 part”的单元测试，对扩展名/阈值/语言映射做纯函数单测；docx 样式可用 `docx::tests::emit_manual_samples`（`cargo test emit_manual_samples -- --ignored`）生成样张后在钉钉手工核对。
