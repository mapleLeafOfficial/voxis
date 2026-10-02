# Voxis — Linux 全局按住说话（语音输入）

复刻微信 PC 端「按住说话」体验的 Linux 桌面应用：**任意应用焦点下按住快捷键说话 → 松开，文字直接出现在光标处**。

| 微信功能 | Voxis 对应 |
|---|---|
| 按住说话，松开上屏 | hold 模式：按住 `Ctrl+Super` 说话，松开自动上屏 |
| 点击麦克风说话 | 托盘图标左键点击：开始/结束会话 |
| 语音转文字后可修改 | 整理模式（off/auto/manual）：qwen-flash 剔除语气词、理顺标点 |
| — | lock 模式：`Ctrl+Super+Shift` 切换「持续听写」，无需按住 |
| — | 权限/热键/上屏全程可配置，设置窗口可视化编辑 |

## 安装

### 方式一：脚本（推荐，Arch Linux）

```bash
git clone <repo> && cd voxis
sudo ./scripts/setup.sh     # 系统依赖 + 用户组 + ydotoold 服务
./scripts/build.sh          # 构建（产出 deb/AppImage/裸二进制）
sudo pacman -U src-tauri/target/release/bundle/deb/voxis_*.deb  # 或 dpkg -i
```

> ⚠️ setup.sh 加了用户组（input/uinput），**必须注销重新登录**才生效。

### 方式二：只装包

```bash
sudo pacman -S ydotool wl-clipboard gnome-shell-extension-appindicator
sudo usermod -aG input,uinput $USER && sudo groupadd -f uinput
sudo pacman -U voxis_0.1.0_amd64.deb   # 注销重登后启动 voxis
```

### API Key

DashScope（阿里云百炼）Key，优先级：设置页填写的 Key > `QWEN_API_KEY` 环境变量 > `~/.config/qwen-voice-input/config.ini`。

## 使用

| 操作 | 效果 |
|---|---|
| 按住 `Ctrl+Super` 说话，松开 | 文字上屏到当前光标（按住期间任意窗口可打字，焦点不被打断） |
| 按 `Ctrl+Super+Shift` | 锁定开始听写；再按一次结束 |
| 托盘图标左键 | 开始/结束会话 |
| 托盘右键 | 显示主窗口 / 设置 / 检查权限 / 退出 |
| `Ctrl+Super+O`（manual 模式） | 整理剪贴板中的口语文字并粘贴 |
| `voxis --minimized` | 仅托盘启动（开机自启用） |

气泡浮窗：录音时屏幕底部显示呼吸红点 + 音量条；结束后展示结果徽标（已上屏 / 已复制 / 未识别 / 错误）。

## 配置

`~/.config/voxis/config.json`（设置窗口可视化编辑，改动即时生效）：

- `hotkey.hold/lock`：组合键（规范名：Ctrl/Super/Shift/Alt/A-Z/0-9 等，左右修饰键不分）
- `asr`：模型 / 断句静音 ms / 语义断句 / 语言 / 最长录音
- `input.device`：录音设备（默认 `pipewire`；虚拟麦克风用 `MicYouVirtualSink.monitor` 之类）
- `commit.mode`：`auto`（自动粘贴）/ `clipboard_only`（仅复制）
- `polish.mode`：`off` / `auto`（上屏前整理）/ `manual`（快捷键整理剪贴板）
- `ui`：气泡位置（底部居中/四角）/ 主题（浅色/深色/跟随系统）
- `general.autostart`：开机自启（设置页开关，写 XDG autostart）

## 故障排查

| 症状 | 处理 |
|---|---|
| 热键无反应 | `groups` 查看是否在 `input` 组（需重登录）；日志 `~/.local/share/voxis/logs/` 搜「权限不足」 |
| 上屏变「已复制」 | ydotoold 未运行：`sudo systemctl status ydotoold-voxis`；或 X11 无 DISPLAY 且无 ydotoold |
| 托盘图标不显示 | GNOME 需 `gnome-shell-extension-appindicator`；装后注销重登 |
| 粘贴进 XWayland 应用无内容 | 某些 XWayland 客户端对 data-control 支持差，先复制后手动 Ctrl+V |
| 录音静音 | `input.device` 语义是 ALSA PCM 名：默认 `pipewire` 跟随系统默认源；不要填 `default` |
| 气泡位置不对 | Wayland 下用 gtk-layer-shell 定位（需 libgtk-layer-shell）；不支持的合成器降级 X11 set_position |
| 完全无法启动 | 日志文件看 panic；删除 `~/.config/voxis/config.json` 恢复默认配置 |

## 隐私

- **音频不落盘**：PCM 只在内存流转（调试 wav 泵仅 `--smoke-session` 显式触发）
- 识别与整理经 HTTPS/WSS 发往 DashScope（阿里云），需自备 API Key；本地无第三方遥测
- 剪贴板：上屏前写入你的识别文本并保留（commit 后不清除），注意敏感内容

## 开发

```bash
bun install
bun run tauri dev          # 开发（vite HMR + Rust 调试构建）
./scripts/build.sh         # 打包
```

技术栈：Tauri 2（Rust + Vue 3）、cpal 录音（Actor 模式）、DashScope 流式 ASR（WebSocket 双工，IPv4 优先 + 逐地址超时轮换）、evdev 全局热键、ydotool/uinput 注入、gtk-layer-shell 气泡定位、wl-clipboard-rs 剪贴板。

### 无头冒烟

```bash
export QWEN_API_KEY=sk-xxx
VOXIS_SMOKE_WAV=/path/to/16k_mono.wav cargo run -- --smoke-session  # 全流程一轮
VOXIS_SMOKE_WAV=/path/to/16k_mono.wav cargo run -- --smoke-loop     # 3 轮泄漏检查
```

wav 要求：16kHz / 单声道 / PCM16 LE。开发文档见 `doc/PRD.md` 与 `doc/plan/`。
