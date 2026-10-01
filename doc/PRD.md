# Voxis — Linux 全局语音输入 — 产品需求文档 (PRD)

> 复刻微信 PC 端「按住 Ctrl+Win 说话，松开文字上屏」的全局语音输入体验。
> 版本：v0.1 · 状态：待评审

---

## 1. 项目概述

### 1.1 项目背景

微信 PC 端 4.1.8（2026.03 全量）上线了全局语音输入：按住 `Ctrl+Win` 说话，松开后文字自动粘贴到当前光标位置；任意应用（Word、浏览器、记事本）均可调用，配合实时转写、智能标点断句与大模型「整理文字」。该功能仅官方 Windows/Mac 客户端提供，**Linux 无官方版本**。

作者日常使用 Arch Linux（GNOME Wayland）+ 手机虚拟麦克风（MicYou），已有 Python 原型 `voice_input_linux`（batch/stream 两种模式）验证了技术链路。本项目用 **Tauri 2 (Rust + Vue 3)** 从零重构为正式的桌面 GUI 应用。

### 1.2 项目目标

- **核心**：复刻微信 PC 语音输入的完整交互——全局快捷键触发、流式转写气泡预览、松开上屏、无焦点回退复制
- **体验**：按下到出字延迟低、气泡跟手、零打扰（不抢焦点）
- **可靠**：录音/识别/粘贴任一环节失败都有明确通知与回退路径

### 1.3 交互总纲（对齐微信实测行为，已与需求方确认）

```
按下 Ctrl+Win（按住）
   │  开始录音 + 建立 ASR 流
   ▼
持续说话 ──→ 中间转写文字在【气泡浮窗】中陆续出现（不上屏）
   │          （服务端自动断句、加标点、分段）
松开按键
   │  停止录音 → 等待最终结果（≤2s 超时兜底）
   ▼
完整文字一次性上屏：
   - 尝试注入 Ctrl+V 粘贴到当前光标
   - 注入失败（无编辑焦点/不支持）→ 回退为「复制到剪贴板」+ 通知
```

### 1.4 明确不做（Out of Scope）

- ❌ 「over over」语音自动发送（需求方确认不需要）
- ❌ 语音消息录制与发送（不做微信的「按住说话发语音条」）
- ❌ Windows / macOS / 移动端
- ❌ 离线识别（v1 不做本地模型）

### 1.5 目标用户

| 角色 | 描述 | 核心需求 |
|------|------|----------|
| 作者本人（首要） | Arch + GNOME Wayland，大量桌面文字输入，用 MicYou 手机麦克风 | 全局快捷键、低延迟、稳定上屏 |
| Linux 桌面用户（通用） | Wayland/X11 各主流桌面 | 安装简单、权限自检、可自定义快捷键 |

---

## 2. 功能模块

> 按优先级排列。层级标注：**Rust**（src-tauri 后端）/ **Vue**（前端 UI）/ **系统**（外部依赖）。

### 模块 M1：全局热键引擎
- **优先级**：P0
- **涉及层级**：Rust + 系统（evdev、ydotool）
- **描述**：绕过桌面环境限制，底层监听键盘按下/释放事件，实现真正的 hold-to-talk 与锁定模式。
- **用户故事**：
  - 作为用户，我按住 `Ctrl+Win` 说话、松开即结束，以便双手不离开键盘完成语音输入
  - 作为用户，我用 `Ctrl+Win+Shift` 切换锁定模式，以便长段落输入时不用一直按着
- **功能点**：
  - [ ] evdev 直读 `/dev/input/event*`，解析按键按下/释放事件（需 `input` 组权限）
  - [ ] **hold 模式**：组合键按下 → 发出「开始」；松开 → 发出「结束」
  - [ ] **lock 模式**：组合键按下 → toggle 开始/结束
  - [ ] 组合键可自定义（设置界面录制按键，写入配置）
  - [ ] 触发时抑制原按键传递？——**不抑制**（Wayland 下 evdev 只读不阻断；`Ctrl+Win` 本身无系统副作用，单击 `Win` 才会触发 Activities，属可接受行为）
  - [ ] 权限自检：启动时检测 `input` 组、`uinput` 组、`ydotoold` 运行状态，缺失则通知并给修复指引
  - [ ] 热键触发去抖（按下瞬间已有会话进行中 → 忽略/或作为结束信号，取简单方案）

### 模块 M2：录音引擎
- **优先级**：P0
- **涉及层级**：Rust（cpal / PipeWire）
- **描述**：低延迟采集麦克风 PCM，喂给 ASR 流，同时输出音量电平给 UI。
- **功能点**：
  - [ ] cpal 采集默认输入设备：16kHz / 单声道 / i16 PCM，块大小 1024 帧
  - [ ] 输入设备可切换（下拉列出 PipeWire 所有 source，含 MicYou 虚拟麦克风）
  - [ ] RMS 音量回调（100ms 粒度）→ 气泡动画
  - [ ] 最长录音兜底（默认 60s，可配置），到时自动按「松开」处理
  - [ ] 音频仅存内存环形缓冲，**不落盘**；会话结束即释放
  - [ ] 设备热插拔容错：录音中设备消失 → 结束会话并通知

### 模块 M3：流式 ASR 客户端
- **优先级**：P0
- **涉及层级**：Rust（tokio-tungstenite）
- **描述**：对接阿里云百炼 DashScope 双工 ASR WebSocket，实时拿到中间/最终转写结果。
- **用户故事**：作为用户，我边说边看到文字在气泡里出现，以便确认识别内容是否正确。
- **功能点**：
  - [ ] WebSocket 协议：`run-task` → `task-started` → 二进制 PCM 上行 → `result-generated` → `finish-task` → `task-finished`
  - [ ] 模型可配置，默认 `qwen3-asr-flash-streaming`（以百炼实际可用版本为准；回退 `qwen-audio-3.0-asr-flash-streaming`）
  - [ ] 服务端断句参数可配置：`max_sentence_silence`（VAD 静音 ms）、`semantic_punctuation_enabled`（语义断句开关）
  - [ ] 语言提示可配置：留空自动检测 / `zh` / `en`（支持方言与中英混说，依赖服务端能力）
  - [ ] 中间结果（`sentence_end=false`）→ 实时推送气泡；句子结束 → 累积进最终文本
  - [ ] 静音块发零值 PCM（省流量的做法保留）
  - [ ] API Key 读取顺序：环境变量 `QWEN_API_KEY`/`DASHSCOPE_API_KEY` > 配置文件
  - [ ] 异常处理：连接失败 / `task-failed` / 中途断流 / 结束超时（松开后最多等 2s 最终结果，超时用已有累积文本兜底上屏）→ 均有桌面通知
  - [ ] 未配置 Key 时触发热键 → 通知并自动打开设置窗口

### 模块 M4：气泡浮窗（实时预览 UI）
- **优先级**：P0
- **涉及层级**：Vue（Tauri 独立 WebviewWindow）
- **描述**：录音期间出现的置顶无边框小浮窗，实时展示转写文字与音量动画；**绝不抢键盘焦点**。
- **功能点**：
  - [ ] 无边框、置顶、透明背景、跳过任务栏；`focusable=false` 不抢焦点
  - [ ] 位置默认屏幕底部居中（上方 120px），可在设置中改为四角
  - [ ] 内容：麦克风状态点（呼吸动画）+ 音量条 + 中间转写文字（自动滚动到底部）
  - [ ] 松开后：浮窗显示「✓ 已上屏」或「⧉ 已复制」约 1.5s 后淡出
  - [ ] 出错：浮窗显示错误摘要，同时发系统通知
  - [ ] 点击浮窗 = 手动结束（等同松开按键）
  - [ ] 深浅色跟随系统（GNOME 主题）

### 模块 M5：上屏引擎
- **优先级**：P0
- **涉及层级**：Rust + 系统（ydotool / xdotool）
- **描述**：把最终文本送进「当前光标位置」；Wayland 无法探测焦点是否可编辑，采用**先粘贴、失败回退复制**策略实现「没有编辑位置就是复制」。
- **功能点**：
  - [ ] 复制：`wl-clipboard-rs`（Wayland）→ `arboard` → `xclip/xsel`（X11 回退）
  - [ ] 注入 `Ctrl+V`：ydotool（uinput，Wayland 通用；需 `ydotoold` 运行）→ `xdotool`（X11 回退）
  - [ ] 上屏方式可配置：`自动`（默认，粘贴失败回退复制）/ `仅复制`
  - [ ] 上屏结果三态反馈：已粘贴 / 已复制（通知提示手动 Ctrl+V）/ 失败
  - [ ] 注入前后 150ms 静默（等待剪贴板同步），避免粘贴到旧内容
  - [ ] 空结果（没识别到文字）→ 通知「未识别到内容」，不上屏

### 模块 M6：设置窗口
- **优先级**：P1
- **涉及层级**：Vue + Rust（配置读写）
- **描述**：主窗口，集中管理全部配置；从托盘/气泡/通知入口打开。
- **功能点**：
  - [ ] 分组页签：账户 / 快捷键 / 识别 / 上屏 / 整理 / 外观 / 关于
  - [ ] 账户：API Key（脱敏显示、测试连通性按钮）
  - [ ] 快捷键：hold 组合键与 lock 组合键各自录制（抓取 evdev 键码映射）
  - [ ] 识别：模型选择、断句静音 ms、语义断句开关、语言、最长录音秒数
  - [ ] 上屏：自动粘贴/仅复制、输入设备选择
  - [ ] 整理：off / 自动 / 手动（见 M7）
  - [ ] 外观：气泡位置、主题（亮/暗/跟随系统）
  - [ ] 开机自启开关（写入 `~/.config/autostart`）
  - [ ] 权限自检面板：`input` 组 / `uinput` 组 / `ydotoold` / 剪贴板工具，逐项 ✓/✗ + 一键修复指引
  - [ ] 配置持久化 `~/.config/voxis/config.json`（0600 权限）

### 模块 M7：整理文字（大模型后处理）
- **优先级**：P1
- **涉及层级**：Rust（reqwest → DashScope OpenAI 兼容接口）
- **描述**：用 `qwen-flash` 对最终文本做口语清洗：剔除「嗯、啊、然后、那个」等语气词，精简冗余、理顺标点，不改变原意。
- **功能点**：
  - [ ] 三种模式：`off`（默认）/ `auto`（松开后先整理再上屏，整体延迟 +1~2s）/ `manual`
  - [ ] `manual` 模式：全局快捷键（如 `Ctrl+Win+O`）触发，把**剪贴板中的文本**整理后重新复制+尝试粘贴（局限：不能替换已粘贴进第三方输入框的内容，通知中说明）
  - [ ] 整理 prompt 可在高级配置中查看/修改
  - [ ] 整理失败 → 回退上屏原始文本 + 通知

### 模块 M8：托盘 / 自启 / 单实例
- **优先级**：P1
- **涉及层级**：Rust + Vue
- **功能点**：
  - [ ] 托盘图标两态：空闲（麦克风）/ 录音中（红色高亮）
  - [ ] **点击托盘图标 = 点击麦克风入口**：空闲时开始会话（lock 式），录音中结束（对应微信「点击开始、再点结束」）
  - [ ] 右键菜单：打开设置 / 锁定模式开关 / 检查权限 / 退出
  - [ ] 单实例：二次启动唤起设置窗口而不是新进程
  - [ ] 开机自启：autostart desktop 文件管理（Tauri autostart 插件）

### 模块 M9：安装脚本与打包
- **优先级**：P1
- **涉及层级**：系统 / 打包
- **功能点**：
  - [ ] `setup.sh`：安装系统依赖（`ydotool`、`wl-clipboard`）、把用户加入 `input`/`uinput` 组、`systemctl enable --now ydotoold`、权限自检
  - [ ] 构建脚本：`bun`（前端）+ `cargo tauri build`（产出 .deb / AppImage / 原生二进制）
  - [ ] README：Arch GNOME Wayland 为主文档，X11/其他桌面为附注
  - [ ] （P2，可选）AUR `voxis-bin` PKGBUILD

### 模块 M10：本地文字历史
- **优先级**：P2（默认关闭）
- **涉及层级**：Rust（SQLite 或 JSONL）
- **功能点**：
  - [ ] 设置开关（默认 off），开启后记录时间戳 + 最终文本到 `~/.local/share/voxis/history.db`
  - [ ] 设置窗口内可查看 / 搜索 / 清空

---

## 3. 数据模型设计

### 3.1 配置文件 `~/.config/voxis/config.json`

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `api_key` | String | "" | 百炼 API Key（也可走环境变量） |
| `asr.model` | String | "qwen3-asr-flash-streaming" | 流式 ASR 模型 |
| `asr.ws_url` | String | 内置默认 | DashScope WebSocket 地址 |
| `asr.silence_ms` | Int | 1300 | 服务端 VAD 断句静音 |
| `asr.semantic_punct` | Bool | true | 语义断句 |
| `asr.language` | String | "" | 语言提示，空=自动 |
| `asr.max_duration` | Int | 60 | 最长录音秒数 |
| `hotkey.hold` | String[] | ["Ctrl","Super"] | hold 模式组合键（evdev 键码映射） |
| `hotkey.lock` | String[] | ["Ctrl","Super","Shift"] | lock 模式组合键 |
| `input.device` | String | "default" | 采集设备 |
| `commit.mode` | Enum | "auto" | auto / clipboard_only |
| `polish.mode` | Enum | "off" | off / auto / manual |
| `polish.model` | String | "qwen-flash" | 整理用模型 |
| `polish.hotkey` | String | "Ctrl+Super+O" | manual 整理触发键 |
| `ui.bubble_pos` | Enum | "bottom_center" | 气泡位置 |
| `ui.theme` | Enum | "system" | light / dark / system |
| `general.autostart` | Bool | false | 开机自启 |

### 3.2 会话状态机（内存）

```
Idle ──(热键按下/托盘点击)──→ Recording ──(松开/点击浮窗/60s超时)──→ Committing
  ▲                               │                                    │
  └────────(完成/错误)────────────┴────────────────────────────────────┘
Recording:  音频→ASR流, partial → 气泡
Committing: finish-task, 收最终结果(≤2s兜底) → 整理(可选) → 上屏/复制 → Done → Idle
```

---

## 4. 接口设计概要

### 4.1 外部服务

| 接口 | 协议 | 地址/模型 | 说明 | 所属模块 |
|------|------|-----------|------|----------|
| 流式识别 | WebSocket (双向) | `wss://dashscope.aliyuncs.com/api-ws/v1/inference` · `qwen3-asr-flash-streaming` | run-task / 二进制 PCM / result-generated / finish-task | M3 |
| 文本整理 | HTTPS POST | DashScope OpenAI 兼容 `chat/completions` · `qwen-flash` | system prompt 清洗口语，temperature≈0 | M7 |

### 4.2 Tauri IPC（前端 ⇄ Rust）

| Command | 方向 | 说明 | 所属模块 |
|---------|------|------|----------|
| `start_session(lock: bool)` | 前端→Rust | 托盘/浮窗触发开始 | M2/M3 |
| `stop_session()` | 前端→Rust | 手动结束 | M5 |
| `get_permission_status()` | 前端→Rust | 权限自检结果 | M1/M6 |
| `get/set_config()` | 前端⇄Rust | 配置读写 | M6 |
| `list_input_devices()` | 前端→Rust | 枚举采集设备 | M2/M6 |
| `test_api_key()` | 前端→Rust | 连通性测试 | M6 |
| `polish_clipboard()` | 前端→Rust | manual 整理 | M7 |
| Event `session://partial` | Rust→前端 | 中间转写文本 | M3/M4 |
| Event `session://volume` | Rust→前端 | 音量电平 | M2/M4 |
| Event `session://state` | Rust→前端 | 状态机变更 | M4 |
| Event `session://committed` | Rust→前端 | 上屏结果（pasted/copied/failed + 文本） | M4/M5 |

---

## 5. 非功能性需求

- **性能**：热键按下 → 开始录音 < 100ms；说话中首字节出现在气泡 < 1s（网络相关）；松开 → 上屏 ≤ 2s（含收尾兜底）；空闲常驻内存 ≤ 80MB，空闲 CPU ≈ 0
- **隐私安全**：音频不落盘、会话结束即释放；文字历史默认关闭；API Key 本地存储权限 0600；所有网络仅连 DashScope
- **兼容性**：首选 Arch Linux + GNOME + Wayland + PipeWire；回退 X11（xdotool 路径）；不承诺 KDE/Wayland 其他合成器，尽力兼容
- **健壮性**：任何外部依赖缺失（Key/权限/工具）都有明确通知与指引，不静默失败；识别/整理失败不阻塞主流程（回退原始文本）

## 6. 开发约束

- 技术栈：Tauri 2 + Rust 2021（tokio 异步运行时）+ Vue 3 + TypeScript + Vite + Tailwind CSS
- 包管理：前端 `bun`，Rust `cargo`；严格区分 `src-tauri/` 与 `src/`
- 模块边界：热键/录音/ASR/上屏/整理各自独立 Rust 模块（`src-tauri/src/` 下分文件），UI 只通过 IPC Command/Event 通信
- 键码映射统一走 evdev 常量（`evdev` crate `Key` 枚举），禁止硬编码扫描码魔数
- 日志：`tracing` 输出到 `~/.local/share/voxis/voxis.log`，级别可配置
- 每个模块（todo）完成时必须可构建、可运行、可手动验证该模块功能
