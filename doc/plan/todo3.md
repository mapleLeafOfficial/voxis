# Todo3：流式 ASR 客户端

## 基本信息

- **对应 PRD 模块**：M3
- **优先级**：P0
- **预估工作量**：~6 个文件
- **依赖**：todo1
- **状态**：⬜ 未开始

---

## 目标描述

实现 DashScope 双工 ASR WebSocket 客户端：run-task → task-started → 二进制 PCM 上行 → result-generated（partial/final 句子）→ finish-task → task-finished。参数、模型、API Key 解析均可配置，异常全兜底。

---

## 任务清单

### Rust

- [ ] **依赖**：`tokio-tungstenite`（含 native-tls）、`futures-util`、`uuid`、`rand`（nonce）
- [ ] **协议层** `src-tauri/src/asr/protocol.rs`：
  - 请求构造：`run-task`（header 含 `X-DashScope-DataInspection: enable`，`task_group=audio`，`task=asr`，`function=recognition`，model、`input{}`、`parameters{sample_rate:16000, format:"pcm", max_sentence_silence, semantic_punctuation_enabled, language_hints}`）
  - 响应解析：`task-started` / `result-generated`（`payload.output.sentence`：`text`、`end_time`、`sentence_end: bool`）/ `task-failed`（code+message）/ `task-finished`
  - 二进制帧：4 字节 header + payload PCM
- [ ] **客户端** `src-tauri/src/asr/client.rs`：
  - `AsrClient::connect(cfg) -> Result`：连 `wss://dashscope.aliyuncs.com/api-ws/v1/inference`，鉴权 `Authorization: bearer <key>`，发 run-task 等 task-started
  - `send_pcm(&[i16])`：切片 ≤ 3200 帧打包二进制帧；静音时发零值块
  - 回调通道：`mpsc::Receiver<AsrEvent>`，事件 `Partial(String)` / `SentenceEnd(String)` / `Finished(String /*全文*/)` / `Failed{code,message}` / `Closed`
  - `finish()`：发 finish-task；内部等 task-finished（带超时）
  - 掉线/超时检测：读空闲 > 10s 视为断流 → Failed
- [ ] **API Key 解析** `src-tauri/src/asr/key.rs`：env `QWEN_API_KEY` > `DASHSCOPE_API_KEY` > config；`resolve_api_key()` 返回 `Option<String>`
- [ ] **默认模型**：`qwen3-asr-flash-streaming`（配置可改；连接失败且模型报无效时日志提示回退 `qwen-audio-3.0-asr-flash-streaming`，不自动重试）
- [ ] **集成测试** `src-tauri/examples/asr_smoke.rs`：读 16k wav → 走完整会话 → 打印全文（手动 `cargo run --example` 验证）

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

- [ ] `cargo build` 通过
- [ ] `asr_smoke` 用真实 Key 跑通：输出一段 16kHz wav 的转写全文，含标点
- [ ] partial → sentence_end 序列在日志可见（说话中 text 变化、末尾 sentence_end=true）
- [ ] 错误路径：错误 Key → `task-failed` 事件带明确 code；拔网线 → 断流 Failed
- [ ] 无 Key 时 `resolve_api_key()` 返回 None 且不影响编译/启动

## 备注

模型名以百炼实际开通为准；`language_hints` 留空 = 自动检测。冒烟 wav 可用现有 voice_input 脚本录一段或 ffmpeg 生成。
