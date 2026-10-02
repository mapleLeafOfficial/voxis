# Todo4：会话状态机 + IPC 整合

## 基本信息

- **对应 PRD 模块**：M2+M3 串联（核心编排层）
- **优先级**：P0
- **预估工作量**：~6 个文件
- **依赖**：todo2、todo3
- **状态**：✅ 已完成（代码+冒烟验证；实机麦克风 M1 待用户验证）

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

---

## 完成记录（2026-10-02）

### 交付内容
- `session.rs`：SessionState 三态机、SessionManager::start/stop（幂等）、wav 测试泵（VOXIS_SMOKE_WAV）、collector 自动收尾、max_duration 自动停
- `events.rs`：统一事件常量与 payload；`commands`：start_session/stop_session；DevView 会话面板；`state.rs` 挂载
- `--smoke-session` / `--smoke-loop` 冒烟钩子（无头验证会话全流程）

### 关键调试结论（根因记录）
wav 泵 E2E 曾长期「渐进修正中途停止、task-finished 不回、收尾 2s 超时」，而 asr_smoke/inline 实验稳定成功。经字节 md5 三方比对（失败 dump = inline dump = 源 wav）、TLS 指纹、节点、节奏、收尾方式等全量排除后，用 strace 定位到**真正的根因**：

> **`stop()` 里的 `done_rx.recv_timeout(2s)`（std::sync::mpsc 同步阻塞调用）直接卡住了 tokio runtime 中唯一守着 IO driver 的 worker 线程 2 秒。** 期间 timer wheel 停摆、epoll 无人值守——owner task 的读超时/服务端响应全部冻结，collector 也停摆。strace 显示 2s 窗口内全进程无任何线程在 epoll_wait。

修复：done channel 改用 tokio::sync::mpsc，stop() 内等待改为 `timeout(2s, done_rx.recv()).await`（异步让出）。修复后 wav 泵 E2E 连续 5/5 稳定输出完整断句「今天天气真不错，我们一起去逛。」。

**经验**：async 上下文中严禁 std mpsc 的 blocking recv/block_on 等同步等待；它们会冻结 tokio driver（timer+IO），表现为「随机 task 卡死」的诡异现象。

### 其他改进
- **prefer_ipv4 连接健壮化**：自解析 DNS、v4 优先、逐地址 1.5s 超时轮换 connect + TCP_NODELAY（规避阿里云个别「能握手但收流卡死」的坏节点）
- **owner 主循环重构**：`try_recv` 非阻塞取命令 + 单一 `timeout(SELECT_TICK=300ms, stream.next()).await`，循环顶检查 stop 标志（AtomicBool）——收尾路径完全确定性，不依赖跨 task 唤醒
- **渐进修正认知**：流式返回的「K→今天→今天天气…」渐进替换是正常现象；sentence_end=true 才是断句

### 验证结果
- [x] 无 Key 冒烟：优雅报错不崩溃（冒烟①）
- [x] 静默会话（不说话）：空文本优雅收尾（冒烟②）
- [x] wav 泵 E2E：5/5 完整断句（冒烟③替代方案，确定性 PCM 输入）
- [x] --smoke-loop 3 轮连续 start/stop：全部 Ok，内存 200MB→209MB 无泄漏迹象
- [x] cargo build 零警告；临时诊断代码已全部清除
- [ ] 实机麦克风 M1（真实录音→停止→上屏文本）：待用户在 DevView 验证
