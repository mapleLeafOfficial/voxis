# Voxis — Linux 全局按住说话（语音输入）

复刻微信 PC 端「按住说话」体验的 Linux 桌面应用：全局热键按住 → 录音 → DashScope 流式 ASR → 松开自动上屏。

## 技术栈

- **Tauri 2**（Rust 后端 + Vue 3 前端）
- **录音**：cpal（PipeWire/PulseAudio），Actor 模式 owner 线程
- **ASR**：DashScope `qwen-audio-3.0-asr-flash-streaming`（WebSocket 双工、裸 PCM i16 LE 上行、流式渐进修正）
- **连接健壮化**：DNS 自解析 + IPv4 优先 + 逐地址 1.5s 超时轮换（规避坏节点）+ TCP_NODELAY

## 开发

```bash
bun install
cargo tauri dev          # 或 bun run tauri dev
```

### 无头冒烟测试（无需 GUI 交互）

```bash
export QWEN_API_KEY=sk-xxx
# 会话全流程：加载 wav 以 100ms 实时节奏泵入 → 6s 后 stop → 退出
VOXIS_SMOKE_WAV=/path/to/16k_mono.wav cargo run -- --smoke-session
# 同进程 3 轮 start/stop 泄漏检查
VOXIS_SMOKE_WAV=/path/to/16k_mono.wav cargo run -- --smoke-loop
```

wav 要求：16kHz / 单声道 / PCM16 LE。

## 配置

`~/.config/voxis/config.json`（asr.model / ws_url / silence_ms / semantic_punctuation / audio.device 等）。API Key 解析顺序：环境变量 `QWEN_API_KEY` > `~/.config/qwen-voice-input/config.ini`。

## 进度

见 `doc/plan/README.md`（todo1~4 已完成；实机麦克风验证与热键/上屏在后续 todo）。

## 注意事项

- **async 上下文严禁同步阻塞等待**（std mpsc recv_timeout / block_on）：会冻结 tokio driver worker，导致全 runtime 的 timer/IO 停摆（详见 doc/plan/todo4.md 根因记录）
- DashScope 流式返回含渐进修正（前缀替换），以 `sentence_end=true` 为断句依据
- 服务器对持续静音的容忍有限：空闲 ~10s 会断流（owner 检测后优雅收尾）

---

## Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Vue - Official](https://marketplace.visualstudio.com/items?itemName=Vue.volar) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)
