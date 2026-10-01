# Todo2：录音引擎

## 基本信息

- **对应 PRD 模块**：M2
- **优先级**：P0
- **预估工作量**：~7 个文件
- **依赖**：todo1
- **状态**：✅ 已完成（音量条实听验证待用户确认）

---

## 目标描述

用 cpal 实现低延迟麦克风采集（16kHz/单声道/i16），支持设备枚举与切换、RMS 音量事件、最长录音兜底与设备热插拔容错。音频只在内存流动，不落盘。

---

## 任务清单

### Rust

- [x] **依赖**：Cargo.toml 增加 `cpal`
- [x] **采集模块** `src-tauri/src/audio/capture.rs`：
  - `CaptureStream`：给定设备名（`"default"` 用 cpal 默认输入），启动 stream，16kHz mono i16，块 1024 帧
  - 设备原生采样率 ≠16k 时用 `rubato` 或线性插值重采样（简单实现优先）
  - PCM 块回调：`(Vec<i16>) -> ()`（本 todo 先接 tracing 计数，todo4 接 ASR）
  - RMS 电平每 100ms 聚合 → 发 `session://volume` 事件（0.0~1.0，已做对数刻度）
  - `stop()` 优雅停止并 join 线程
- [x] **设备枚举** `src-tauri/src/audio/devices.rs`：`list_input_devices()` → `Vec<{id, name, is_default}>`（含 PipeWire source；MicYou 虚拟设备自然出现）
- [x] **最长录音兜底**：采集开始时起计时器，达 `asr.max_duration` 秒 → 发 `session://max_duration_reached`（会话层 todo4 消费）
- [x] **容错**：设备打开失败 / 运行中 stream 报错 → 发 `session://error{source:"audio"}` 并停止
- [x] **命令**：`list_input_devices`、临时 `dev_capture_start(device)` / `dev_capture_stop()`（DevView 用）

---

## 涉及的文件

| 操作 | 文件路径 | 说明 |
|------|----------|------|
| 修改 | `src-tauri/Cargo.toml` | +cpal |
| 新增 | `src-tauri/src/audio/mod.rs` | 模块声明 |
| 新增 | `src-tauri/src/audio/capture.rs` | 采集核心 |
| 新增 | `src-tauri/src/audio/devices.rs` | 设备枚举 |
| 修改 | `src-tauri/src/state.rs` | AppState 挂当前 CaptureStream |
| 修改 | `src-tauri/src/commands/mod.rs` | 新命令 |
| 修改 | `src/views/DevView.vue` | 临时采集开关 + 音量显示 |

---

## 验证标准

- [x] `cargo build` 通过（零警告）
- [x] 采集引擎冒烟测试（`cargo run --example capture_smoke`）：3 秒录得 2.94s@16kHz PCM（47104 帧/46 块），max_duration 兜底触发自动停止 ✅
- [x] `list_input_devices` 返回设备列表且含当前默认输入（probe_device 实测：默认设备 44100Hz 2ch F32）
- [ ] 切换到 MicYou 虚拟设备后音量随手机端输入变化（待用户实机验证）
- [ ] DevView 音量条随声音实时起伏（待用户实机验证）
- [x] 录满 max_duration 自动停止并记录日志（冒烟测试内置验证）

## 备注

1. **cpal::Stream 不是 Send**（含裸指针），不能进 Tauri managed state —— 新增 `audio/manager.rs`：owner 线程独占持有流（Actor 模式），state 只存命令通道。todo4 SessionManager 延用此模式。
2. **重采样步进方向 bug**（已修）：初版把「每输出前进的输入位置增量」写成了「每输入的输出比例」，导致 44.1k→16k 变 2.76 倍上采样（3s 录出 22.46s）。冒烟测试就是为抓这类问题写的。
3. 本机默认输入设备实测：44100Hz / 2ch / F32；已支持 f32/i16/u16 三种格式、任意声道混音、任意采样率线性重采样。
4. 新增 `examples/capture_smoke.rs`（采集冒烟）与 `examples/probe_device.rs`（设备能力探查），后续调试可复用。

## 备注

GNOME Wayland 下 PipeWire 采集无需额外权限（屏幕录制才需要 portal）；若遇 flatpak/sandbox 问题按非打包 dev 运行处理。
