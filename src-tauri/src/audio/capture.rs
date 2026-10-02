// 麦克风采集核心：设备输入（任意采样率/声道/格式）→ 混音单声道 → 线性重采样 16kHz → i16
// 设计要点：
// - 音频回调线程只做「转 f32 + 混音 + 投递」，重活全在 worker 线程（不阻塞实时线程）
// - 音频仅经内存 channel 流动，不落盘（PRD 隐私要求）
// - RMS 每 100ms 一报；PCM 以 1024 帧切块回调
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

pub const TARGET_RATE: u32 = 16_000;
pub const PCM_CHUNK: usize = 1024;

/// worker 消息：音频数据或运行错误
pub enum AudioMsg {
    Data(Vec<f32>),
    Error(String),
}

/// 统一样本 → f32 转换（Copy：从切片按值取出）
trait IntoF32: Copy {
    fn into_f32(self) -> f32;
}
impl IntoF32 for f32 {
    fn into_f32(self) -> f32 {
        self
    }
}
impl IntoF32 for i16 {
    fn into_f32(self) -> f32 {
        self as f32 / 32768.0
    }
}
impl IntoF32 for u16 {
    fn into_f32(self) -> f32 {
        (self as f32 - 32768.0) / 32768.0
    }
}

/// 采集回调集合（由上层注入：命令层发 tauri 事件，todo4 接 ASR）
pub struct CaptureCallbacks {
    /// RMS 音量 0.0~1.0，约 100ms 一次
    pub on_volume: Box<dyn Fn(f64) + Send>,
    /// 16kHz mono i16 PCM 块
    pub on_pcm: Box<dyn Fn(Vec<i16>) + Send>,
    /// 达到最长录音时长
    pub on_max_duration: Box<dyn Fn() + Send>,
    /// 采集错误（含设备热拔等运行中错误）
    pub on_error: Box<dyn Fn(String) + Send>,
}

pub struct CaptureStream {
    stream: cpal::Stream,
    worker: Option<JoinHandle<()>>,
    max_timer: Option<JoinHandle<()>>,
    running: Arc<AtomicBool>,
}

impl CaptureStream {
    /// 打开设备并开始采集。`device_name` 为 None 或 "default" 时用首选输入
    /// （`pipewire` PCM，跟随 WirePlumber 默认源；不可用则回退系统默认）。
    pub fn start(
        device_name: Option<&str>,
        max_duration: Option<Duration>,
        callbacks: CaptureCallbacks,
    ) -> Result<Self, String> {
        let host = cpal::default_host();
        let device = match device_name.filter(|s| !s.is_empty() && *s != "default") {
            None => super::devices::resolve_default_device(&host)?,
            Some(name) => {
                let found = host
                    .input_devices()
                    .map_err(|e| format!("枚举输入设备失败: {e}"))?
                    .find(|d| d.name().ok().as_deref() == Some(name));
                found.ok_or_else(|| format!("找不到输入设备: {name}"))?
            }
        };
        let dev_name = device.name().unwrap_or_else(|_| "?".into());

        let cfg = device
            .default_input_config()
            .map_err(|e| format!("读取设备默认配置失败({dev_name}): {e}"))?;
        let native_rate = cfg.sample_rate().0;
        let channels = cfg.channels() as usize;
        let sample_format = cfg.sample_format();
        let stream_cfg: cpal::StreamConfig = cfg.into();
        tracing::info!(
            "打开输入设备 {dev_name}: {native_rate}Hz × {channels}ch, format={sample_format:?}"
        );

        // ---- 音频回调 → worker 的单一通道（数据与错误同通道，避免回调闭包间共享 Box 回调）----
        let (tx, rx) = mpsc::channel::<AudioMsg>();
        let running = Arc::new(AtomicBool::new(true));

        // 先拆解回调，各归其主（避免整体移动冲突）
        let CaptureCallbacks { on_volume, on_pcm, on_max_duration, on_error } = callbacks;

        let err_flag = running.clone();
        let err_tx = tx.clone();
        let err_cb = move |e: cpal::StreamError| {
            // 设备热拔/流中断等：标记停止并经通道通知 worker → 上层
            tracing::error!("采集流错误: {e}");
            err_flag.store(false, Ordering::Relaxed);
            let _ = err_tx.send(AudioMsg::Error(format!("采集流错误: {e}")));
        };

        let data_f32 = tx.clone();
        let data_i16 = tx.clone();
        let data_u16 = tx.clone();
        let stream = match sample_format {
            cpal::SampleFormat::F32 => device.build_input_stream(
                &stream_cfg,
                move |data: &[f32], _| forward::<f32>(data_f32.clone(), data, channels),
                err_cb,
                None,
            ),
            cpal::SampleFormat::I16 => device.build_input_stream(
                &stream_cfg,
                move |data: &[i16], _| forward::<i16>(data_i16.clone(), data, channels),
                err_cb,
                None,
            ),
            cpal::SampleFormat::U16 => device.build_input_stream(
                &stream_cfg,
                move |data: &[u16], _| forward::<u16>(data_u16.clone(), data, channels),
                err_cb,
                None,
            ),
            other => return Err(format!("不支持的采样格式: {other:?}")),
        }
        .map_err(|e| format!("创建采集流失败({dev_name}): {e}"))?;
        stream.play().map_err(|e| format!("启动采集流失败: {e}"))?;

        // ---- worker：重采样 + RMS + PCM 切块 ----
        let worker_running = running.clone();
        let worker = std::thread::Builder::new()
            .name("voxis-capture-worker".into())
            .spawn(move || {
                let mut resampler = LinearResampler::new(native_rate);
                let mut rms_acc = RmsAccumulator::new(native_rate);
                let mut pcm_buf: Vec<i16> = Vec::with_capacity(PCM_CHUNK * 2);
                let mut errored = false;
                while worker_running.load(Ordering::Relaxed) {
                    // 100ms 无数据视为静音，发 0 电平保持 UI 活性
                    match rx.recv_timeout(Duration::from_millis(100)) {
                        Ok(AudioMsg::Data(chunk)) => {
                            for &s in &chunk {
                                rms_acc.push(s);
                            }
                            if let Some(level) = rms_acc.take_level() {
                                (on_volume)(level);
                            }
                            resampler.push(&chunk, &mut pcm_buf);
                            drain_pcm(&mut pcm_buf, &on_pcm);
                        }
                        Ok(AudioMsg::Error(msg)) => {
                            (on_error)(msg);
                            errored = true;
                            break;
                        }
                        Err(mpsc::RecvTimeoutError::Timeout) => {
                            (on_volume)(0.0);
                        }
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    }
                }
                if !errored {
                    // 收尾：冲掉残余
                    resampler.flush(&mut pcm_buf);
                    drain_pcm(&mut pcm_buf, &on_pcm);
                }
                tracing::info!("采集 worker 退出");
            })
            .map_err(|e| format!("启动采集 worker 失败: {e}"))?;

        // ---- 最长录音兜底 ----
        let max_timer = max_duration.map(|d| {
            let timer_running = running.clone();
            let cb = on_max_duration;
            std::thread::Builder::new()
                .name("voxis-capture-maxtimer".into())
                .spawn(move || {
                    std::thread::sleep(d);
                    if timer_running.load(Ordering::Relaxed) {
                        tracing::info!("达到最长录音时长");
                        cb();
                    }
                })
                .expect("启动 max_timer 失败")
        });

        Ok(CaptureStream { stream, worker: Some(worker), max_timer, running })
    }

    /// 停止采集并回收线程（音频缓冲随之丢弃，不落盘）
    pub fn stop(mut self) {
        self.running.store(false, Ordering::Relaxed);
        drop(self.stream); // 先停实时流，channel 断开后 worker 自然退出
        if let Some(w) = self.worker.take() {
            let _ = w.join();
        }
        // timer 线程睡醒后看到标志位不再发事件；不 join 避免阻塞停止
        drop(self.max_timer.take());
        tracing::info!("采集已停止");
    }

    /// todo4 SessionManager 使用；当前 dev 命令未消费
    #[allow(dead_code)]
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }
}

// ---------- 采集回调：转 f32 + 混音到单声道并投递 ----------

fn forward<S: IntoF32 + Send + 'static>(tx: Sender<AudioMsg>, data: &[S], channels: usize) {
    if data.is_empty() {
        return;
    }
    let mut out = Vec::with_capacity(data.len() / channels.max(1) + 1);
    match channels {
        1 => {
            for f in data.chunks_exact(1) {
                out.push(f[0].into_f32());
            }
        }
        2 => {
            for f in data.chunks_exact(2) {
                out.push((f[0].into_f32() + f[1].into_f32()) * 0.5);
            }
        }
        n => {
            for f in data.chunks_exact(n) {
                let sum: f32 = f.iter().map(|s| s.into_f32()).sum();
                out.push(sum / n as f32);
            }
        }
    }
    let _ = tx.send(AudioMsg::Data(out));
}

// ---------- 线性重采样器 ----------

struct LinearResampler {
    /// 每输出一个样本，输入位置前进的增量（input_rate / target_rate，如 44.1k→16k 为 2.756）
    ratio: f64,
    phase: f64,
    prev: f32,
    flushed: bool,
}

impl LinearResampler {
    fn new(input_rate: u32) -> Self {
        LinearResampler {
            ratio: input_rate as f64 / TARGET_RATE as f64,
            phase: 1.0,
            prev: 0.0,
            flushed: false,
        }
    }

    fn push(&mut self, input: &[f32], out: &mut Vec<i16>) {
        for &s in input {
            while self.phase < 1.0 {
                let v = self.prev + (s - self.prev) * (self.phase as f32);
                out.push((v.clamp(-1.0, 1.0) * 32767.0) as i16);
                self.phase += self.ratio;
            }
            self.phase -= 1.0;
            self.prev = s;
        }
    }

    fn flush(&mut self, out: &mut Vec<i16>) {
        if self.flushed {
            return;
        }
        self.flushed = true;
        while self.phase < 1.0 {
            out.push((self.prev.clamp(-1.0, 1.0) * 32767.0) as i16);
            self.phase += self.ratio;
        }
    }
}

// ---------- RMS 音量 ----------

struct RmsAccumulator {
    window: usize,
    sum_sq: f64,
    count: usize,
}

impl RmsAccumulator {
    fn new(input_rate: u32) -> Self {
        RmsAccumulator { window: (input_rate as usize / 10).max(1), sum_sq: 0.0, count: 0 }
    }

    fn push(&mut self, s: f32) {
        self.sum_sq += (s as f64) * (s as f64);
        self.count += 1;
    }

    /// 满 100ms 返回一次 0.0~1.0 的电平（放大 + 开方压缩动态范围，语音段更可读）
    fn take_level(&mut self) -> Option<f64> {
        if self.count < self.window {
            return None;
        }
        let rms = (self.sum_sq / self.count.max(1) as f64).sqrt();
        self.sum_sq = 0.0;
        self.count = 0;
        // 语音 RMS 通常 0.002~0.3
        Some((rms * 10.0).sqrt().min(1.0))
    }
}

// ---------- PCM 切块 ----------

fn drain_pcm(buf: &mut Vec<i16>, on_pcm: &dyn Fn(Vec<i16>)) {
    while buf.len() >= PCM_CHUNK {
        let chunk: Vec<i16> = buf.drain(..PCM_CHUNK).collect();
        on_pcm(chunk);
    }
}
