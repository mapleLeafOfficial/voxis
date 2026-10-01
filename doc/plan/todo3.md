# Todo3：流式 ASR 客户端

## 基本信息

- **对应 PRD 模块**：M3
- **优先级**：P0
- **预估工作量**：~6 个文件
- **依赖**：todo1
- **状态**：✅ 已完成（含真实转写/错误 Key 双冒烟验证）

---

## 目标描述

实现 DashScope 双工 ASR WebSocket 客户端：run-task → task-started → 二进制 PCM 上行 → result-generated（partial/final 句子）→ finish-task → task-finished。参数、模型、API Key 解析均可配置，异常全兜底。

---

## 任务清单

### Rust

- [x] **依赖**：`tokio-tungstenite`（native-tls）、`futures-util`、`uuid`（v4，task_id；无需单独 rand）
- [x] **协议层** `src-tauri/src/asr/protocol.rs`：
  - 请求构造：`run-task`（`task_group=audio`、`task=asr`、`function=recognition`、model、`input{}`、`parameters{sample_rate:16000, format:"pcm", max_sentence_silence, semantic_punctuation_enabled, language_hints}`）
  - 响应解析：`task-started` / `result-generated`（`payload.output.sentence`：`text`、`sentence_end: bool`、心跳忽略）/ `task-failed`（code+message）/ `task-finished`
  - 音频：**裸 PCM 二进制帧**（每帧 ≤3200 帧，i16 LE；见备注1）
- [x] **客户端** `src-tauri/src/asr/client.rs`（async owner-task 模式，句柄仅持命令通道，可 Clone）：
  - `AsrSession::start(cfg)`：连接（鉴权 `Authorization: Bearer`）→ 发 run-task → task-started 前的音频自动缓冲
  - `send_pcm(&[i16])`：自动按 ≤3200 帧切分二进制帧
  - `mpsc::Receiver<AsrEvent>`：`Started` / `Partial` / `Sentence` / `Finished(全文)` / `Failed{code,message}`
  - `finish()`：发 finish-task，内部等 task-finished（≤2s，PRD 口径）
  - 读空闲 >10s / 对端关闭 → `Failed{code:"idle"}`；Failed 后仍发 `Finished`（携带已识别部分，尽力而为）
- [x] **API Key 解析** `src-tauri/src/asr/key.rs`：env `QWEN_API_KEY` > `DASHSCOPE_API_KEY` > config；`resolve_api_key()` 返回 `Option<String>`
- [x] **默认模型**：**`qwen-audio-3.0-asr-flash-streaming`**（实测 `qwen3-asr-flash-streaming` 对现有 Key 返回 `ModelNotFound`，已改默认值并在 config.rs 注释说明；连接失败不自动重试）
- [x] **集成测试** `src-tauri/examples/asr_smoke.rs`：espeak-ng 生成中文语音→ffmpeg 转 16k 或 `--wav` 指定文件 → 完整会话 → 打印事件流与全文；支持 `--model` / `--bad-key`

---

## 涉及的文件

| 操作 | 文件路径 | 说明 |
|------|----------|------|
| 修改 | `src-tauri/Cargo.toml` | +ws/futures/uuid |
| 新增 | `src-tauri/src/asr/mod.rs` | 模块声明 |
| 新增 | `src-tauri/src/asr/protocol.rs` | 消息构造/解析 |
| 新增 | `src-tauri/src/asr/client.rs` | WS 客户端 |
| 新增 | `src-tauri/src/asr/key.rs` | Key 解析 |
| 新增 | `src-tauri/examples/asr_smoke.rs` | 冒烟测试 |

---

## 验证标准

- [x] `cargo build` 通过（零警告）
- [x] `asr_smoke` 用真实 Key 跑通：espeak 中文语音 → 全文 `你好，欢迎使用语音输入。今天天气怎么样？`（含标点）✅
- [x] partial → sentence_end 序列在日志可见（8 次中间结果递增，末尾断句+finished）✅
- [x] 错误路径：错误 Key → 连接阶段 `HTTP error: 401 Unauthorized`；模型不存在 → `task-failed ModelNotFound` ✅
- [x] 无 Key 时 `resolve_api_key()` 返回 None，不影响编译/启动 ✅

## 备注

1. **协议与计划书的差异**：本计划原写「4 字节二进制帧头」，但实测（以跑通的 voice_input.py 为准）DashScope 双工 ASR 接受 **JSON 文本帧控制 + 裸 PCM 二进制帧（无帧头）**，按后者实现。
2. **TLS 方案**：native-tls（系统 OpenSSL 3.6）而非 rustls —— rustls 需手动选 CryptoProvider，native-tls 更省事且 Arch 开箱即用。
3. **模型实测**：`qwen-audio-3.0-asr-flash-streaming` ✅ 可用；`qwen3-asr-flash-streaming` ❌ ModelNotFound（当前 Key 未开通），应用默认值已改为前者，用户可在设置里自行尝试新模型。
4. `Finished` 事件在失败路径也会发出（携带已识别文本）——上层 todo4 以 `Failed` 为中止信号，`Finished` 里的部分文本可用于尽力而为输出。
5. 冒烟 example 内置了 espeak-ng/ffmpeg 语音生成与 WAV 解析，仅为本机调试用，不进主程序。
