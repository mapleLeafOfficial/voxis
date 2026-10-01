# Todo4：会话状态机 + IPC 整合

## 基本信息

- **对应 PRD 模块**：M2+M3 串联（核心编排层）
- **优先级**：P0
- **预估工作量**：~6 个文件
- **依赖**：todo2、todo3
- **状态**：⬜ 未开始

---

## 目标描述

把录音引擎与 ASR 客户端编排成完整会话状态机（Idle → Recording → Committing → Idle），对外暴露 `start_session`/`stop_session` 命令与 `session://state|partial|volume|error|committed` 事件。这是后续气泡、热键、上屏共同依赖的枢纽。

---

## 任务清单

### Rust

- [ ] **状态机** `src-tauri/src/session.rs`：
  - `SessionState`：Idle / Recording / Committing；`SessionManager`（AppState 持有）
  - `start()`：解析 Key（无 → 发 error 事件 + 打开设置窗口，转 Idle）→ 起 CaptureStream + AsrClient → PCM 回调直喂 `send_pcm` → 转 Recording
  - `stop()`：停采集 → `client.finish()` → **收尾兜底：最多等 2s**（`SentenceEnd` 累积 + 最后一个 partial），超时用已有累积文本 → 产出 `Committed{text}` 事件（本 todo 尚无上屏，文本打日志）→ 释放资源 → Idle
  - max_duration 事件 → 自动 stop；audio/ASR error → 清理转 Idle + error 事件
  - 幂等：Recording 中收到 start 忽略；Idle 中收到 stop 忽略
  - 音频缓冲不落盘：PCM 块仅经 channel 流动，会话结束 drop
- [ ] **事件总线** `src-tauri/src/events.rs`：统一事件名常量 + payload 结构（serde），发到全局 webview
- [ ] **命令**：`start_session(lock: bool)` / `stop_session()`（`lock` 参数本 todo 仅记录，语义在 todo6）
- [ ] **配置生效**：start 时读取当前 config 的 asr/audio 参数，无需重启应用
- [ ] **前端**：DevView 增加会话面板——开始/停止按钮 + 状态显示 + partial/final 文本区（此为过渡 UI，todo5 由气泡替代）

---

## 涉及的文件

| 操作 | 文件路径 | 说明 |
|------|----------|------|
| 新增 | `src-tauri/src/session.rs` | 状态机 + SessionManager |
| 新增 | `src-tauri/src/events.rs` | 事件定义 |
| 修改 | `src-tauri/src/state.rs` | 挂 SessionManager |
| 修改 | `src-tauri/src/commands/mod.rs` | +start/stop_session |
| 修改 | `src-tauri/src/lib.rs` | 启动时初始化 |
| 修改 | `src/views/DevView.vue` | 会话测试面板 |

---

## 验证标准（里程碑 M1：可听）

- [ ] DevView 点「开始」说话 → partial 文本实时刷新；点「停止」→ 2s 内收到最终全文（含标点）
- [ ] 不说话直接停止 → committed 文本为空，通知语义正确（日志确认）
- [ ] 录满 60s 自动收尾
- [ ] 无 Key 环境测试：启动会话 → 自动弹出主窗口（设置占位）
- [ ] 连续 10 次开始/停止循环无泄漏（内存稳定、无残留线程，`btop` 观察）

## 备注

收尾 2s 兜底是「松开即上屏」的关键体验点，宁要快、不要全：超时即用现有文本。
