// 会话状态机（PRD M2 核心编排）：Idle → Recording → Committing → Idle
// 录音引擎（同步线程）+ ASR 客户端（async owner task）的桥接层
use crate::asr::client::{AsrConfig, AsrEvent, AsrSession};
use crate::asr::key::resolve_api_key;
use crate::audio::capture::CaptureCallbacks;
use crate::audio::manager::CaptureManager;
use crate::config::Config;
use crate::events::{self, ErrorPayload};
use crate::state::AppState;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    Idle,
    Recording,
    Committing,
}

impl SessionState {
    pub fn as_str(&self) -> &'static str {
        match self {
            SessionState::Idle => "idle",
            SessionState::Recording => "recording",
            SessionState::Committing => "committing",
        }
    }
}

/// collector 与 stop 共享的文本累积
#[derive(Default)]
struct Shared {
    committed: String,       // 已断句文本（空格连接）
    pending_partial: String, // 当前句最新 partial（断句后清空）
}

struct ActiveSession {
    // cpal::Stream 非 Send，采集流由内嵌 CaptureManager 的 owner 线程持有
    session: AsrSession,
    /// collector 结束信号（task-finished / failed / 通道关闭）
    done_rx: tokio::sync::mpsc::Receiver<()>,
    /// wav 泵停止标志（仅 VOXIS_SMOKE_WAV 测试模式）
    pump_stop: Option<Arc<AtomicBool>>,
}

pub struct SessionManager {
    state: Mutex<SessionState>,
    active: Mutex<Option<ActiveSession>>,
    shared: Arc<Mutex<Shared>>,
    /// 防并发启动（连接期间不持 state 锁，用标志位保护）
    starting: AtomicBool,
    /// 采集流管理器（本会话专属，与 dev 命令的互不干扰）
    capture: CaptureManager,
}

impl SessionManager {
    pub fn new() -> Self {
        SessionManager {
            state: Mutex::new(SessionState::Idle),
            active: Mutex::new(None),
            shared: Arc::new(Mutex::new(Shared::default())),
            starting: AtomicBool::new(false),
            capture: CaptureManager::new(),
        }
    }

    pub fn state(&self) -> SessionState {
        *self.state.lock().expect("state 锁")
    }

    fn emit_state(app: &AppHandle, s: SessionState) {
        let _ = app.emit(events::STATE, s.as_str());
        // 托盘图标/菜单文案跟随（Tray 未初始化时为 no-op）
        crate::tray::update_state(app, s);
    }

    fn emit_error(app: &AppHandle, source: &str, message: String) {
        tracing::error!(source, message = %message, "session error");
        let _ = app.emit(
            events::ERROR,
            ErrorPayload { source: source.into(), message },
        );
    }

    /// 开始会话（幂等：进行中调用直接忽略）。
    /// async：内部有唯一的连接 await 点，前后均不持锁。
    /// `pcm_override`：测试用，直接泵指定 PCM（跳过采集）。
    pub async fn start(
        self: &Arc<Self>,
        app: &AppHandle,
        device: Option<String>,
        lock: bool,
        pcm_override: Option<Vec<i16>>,
    ) -> Result<(), String> {
        if self.starting.swap(true, Ordering::SeqCst) {
            tracing::warn!("start 忽略：正在启动中");
            return Ok(());
        }
        let result = self.start_inner(app, device, lock, pcm_override).await;
        self.starting.store(false, Ordering::SeqCst);
        result
    }

    async fn start_inner(
        self: &Arc<Self>,
        app: &AppHandle,
        device: Option<String>,
        lock: bool,
        pcm_override: Option<Vec<i16>>,
    ) -> Result<(), String> {
        {
            let st = self.state.lock().expect("state 锁");
            if *st != SessionState::Idle {
                tracing::warn!("start 忽略：会话进行中（{:?}）", *st);
                return Ok(());
            }
        }

        let cfg = {
            let app_state = app.state::<AppState>();
            let guard = app_state.config.read().expect("config 锁");
            guard.clone()
        };

        // 1. API Key（无 → error 事件 + 弹出主窗口，保持 Idle）
        let Some(api_key) = resolve_api_key(&cfg.api_key) else {
            Self::emit_error(app, "key", "未配置 API Key".into());
            // GTK 只能主线程操作（start_inner 从 tokio 线程调用）
            let app2 = app.clone();
            let _ = app.run_on_main_thread(move || {
                if let Some(w) = app2.get_webview_window("main") {
                    let _ = w.show();
                    let _ = w.set_focus();
                }
            });
            return Err("未配置 API Key".into());
        };

        // 2. ASR 会话（读当前配置，无需重启应用）—— 唯一 await 点
        let asr_cfg = build_asr_config(&cfg, api_key);
        let key_tail = &asr_cfg.api_key[asr_cfg.api_key.len().saturating_sub(4)..];
        tracing::info!(
            model = %asr_cfg.model, ws = %asr_cfg.ws_url, key_tail,
            rate = asr_cfg.sample_rate, silence = asr_cfg.max_sentence_silence_ms,
            punct = asr_cfg.semantic_punctuation, lang = ?asr_cfg.language_hints,
            v4 = asr_cfg.prefer_ipv4,
            "asr_cfg"
        );
        tracing::info!(lock, device = ?device.as_deref().unwrap_or(cfg.input.device.as_str()), "启动会话");
        let (session, evt_rx) = match AsrSession::start(asr_cfg).await {
            Ok(x) => x,
            Err(e) => {
                Self::emit_error(app, "asr", e.clone());
                return Err(e);
            }
        };

        // 3. collector：ASR 事件 → 前端 + 文本累积；ASR 终止后自动收尾
        let (done_tx, done_rx) = tokio::sync::mpsc::channel::<()>(1);
        let collector = CollectorHandles {
            app: app.clone(),
            manager: self.clone(),
            shared: self.shared.clone(),
            done_tx,
        };
        tauri::async_runtime::spawn(collector.run(evt_rx));

        // 4. 音频源：测试模式泵 wav；否则走采集引擎（流由 CaptureManager 的 owner 线程持有）
        let device = device.or_else(|| {
            let d = cfg.input.device.trim();
            if d.is_empty() || d == "default" { None } else { Some(d.to_string()) }
        });

        if let Some(pcm) = pcm_override {
            // 测试泵：100ms/1600 帧实时节奏（async 发送，与 asr_smoke 同路径），stop 置标志后退出
            let pump_stop = Arc::new(AtomicBool::new(false));
            let stop_flag = pump_stop.clone();
            let sess = session.clone();
            tauri::async_runtime::spawn(async move {
                let mut sent = 0usize;
                for chunk in pcm.chunks(1600) {
                    if stop_flag.load(Ordering::Relaxed) {
                        break;
                    }
                    if sess.send_pcm(chunk.to_vec()).await.is_err() {
                        break;
                    }
                    sent += chunk.len();
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                }
                tracing::info!("wav 泵退出（已发 {sent} 帧）");
            });
            {
                let mut st = self.state.lock().expect("state 锁");
                if *st != SessionState::Idle {
                    tracing::warn!("启动期间状态已变化，回滚");
                    return Ok(());
                }
                *self.active.lock().expect("active 锁") = Some(ActiveSession {
                    session,
                    done_rx,
                    pump_stop: Some(pump_stop),
                });
                *st = SessionState::Recording;
            }
            Self::emit_state(app, SessionState::Recording);
            crate::bubble::show(app);
            return Ok(());
        }

        self.capture.start(
            device,
            Some(std::time::Duration::from_secs(cfg.asr.max_duration as u64)),
            CaptureCallbacks {
                on_volume: {
                    let app = app.clone();
                    Box::new(move |level| {
                        let _ = app.emit(events::VOLUME, level);
                    })
                },
                on_pcm: {
                    let session = session.clone();
                    let app = app.clone();
                    let send_err = Arc::new(AtomicBool::new(false));
                    let first = Arc::new(AtomicBool::new(false));
                    Box::new(move |chunk| {
                        if !first.swap(true, Ordering::Relaxed) {
                            tracing::info!("首块 PCM {} 帧 → ASR", chunk.len());
                        }
                        if let Err(e) = session.send_pcm_blocking(chunk) {
                            // ASR 侧已终止：只报一次，collector 会触发收尾
                            if !send_err.swap(true, Ordering::Relaxed) {
                                Self::emit_error(&app, "asr", e);
                            }
                        }
                    })
                },
                on_max_duration: {
                    let app = app.clone();
                    let manager = self.clone();
                    Box::new(move || {
                        let _ = app.emit(events::MAX_DURATION, ());
                        let (app2, manager2) = (app.clone(), manager.clone());
                        tauri::async_runtime::spawn(async move {
                            if let Err(e) = manager2.stop(&app2).await {
                                tracing::error!("max_duration 自动收尾失败: {e}");
                            }
                        });
                    })
                },
                on_error: {
                    let app = app.clone();
                    Box::new(move |msg| Self::emit_error(&app, "audio", msg))
                },
            },
        )?;

        // 5. 挂入活动会话，转 Recording
        {
            let mut st = self.state.lock().expect("state 锁");
            if *st != SessionState::Idle {
                // 极小概率：连接期间已被 stop/其他 start 抢占 —— 回滚新资源
                tracing::warn!("启动期间状态已变化，回滚");
                let _ = self.capture.stop();
                return Ok(());
            }
            *self.active.lock().expect("active 锁") = Some(ActiveSession {
                session,
                done_rx,
                pump_stop: None,
            });
            *st = SessionState::Recording;
        }
        Self::emit_state(app, SessionState::Recording);
        crate::bubble::show(app);
        Ok(())
    }

    /// 结束会话（幂等）：停采集 → finish → 2s 兜底收文本 → Committed → Idle。
    /// async：finish 用 tokio mpsc send().await（与泵同一唤醒机制，实测可靠）。
    /// 注意：state 锁只在同步段短暂持有，不得跨 await。
    pub async fn stop(&self, app: &AppHandle) -> Result<(), String> {
        // 1. 同步段：检查状态、take 走会话句柄（锁随块结束释放）
        let mut active = {
            let mut st = self.state.lock().expect("state 锁");
            if *st == SessionState::Idle {
                return Ok(());
            }
            *st = SessionState::Committing;
            Self::emit_state(app, SessionState::Committing);

            let Some(active) = self.active.lock().expect("active 锁").take() else {
                *st = SessionState::Idle;
                Self::emit_state(app, SessionState::Idle);
                return Ok(());
            };
            // 停音频源：wav 泵置停（采集停在下方同步调用）
            if let Some(flag) = &active.pump_stop {
                flag.store(true, Ordering::Relaxed);
            }
            active
        };

        // 2. 停采集（同步）+ 请求 owner 发 finish-task
        //    ⚠️ 用 stop 标志让 owner 在下个 tick（≤300ms）自行发 finish-task：
        //    不依赖跨 task 唤醒，收尾路径 100% 确定性。
        let _ = self.capture.stop();
        active.session.request_stop();

        // 3. 2s 兜底等最终结果（宁要快、不要全）
        // ⚠️ 必须异步等待：std recv_timeout 会同步阻塞 tokio driver worker，
        //    冻结整个 runtime 的 timer/IO 驱动（owner/collector 全部停摆）——旧 bug 根因
        if tokio::time::timeout(std::time::Duration::from_secs(2), active.done_rx.recv())
            .await
            .is_err()
        {
            tracing::warn!("收尾等待超时（2s），使用已有文本");
        }

        // 4. 产出 Committed（锁外读共享文本）
        let text = {
            let mut sh = self.shared.lock().expect("shared 锁");
            let mut text = std::mem::take(&mut sh.committed);
            let partial = std::mem::take(&mut sh.pending_partial);
            if !partial.trim().is_empty() {
                if !text.is_empty() {
                    text.push(' ');
                }
                text.push_str(&partial);
            }
            text
        };
        if text.trim().is_empty() {
            tracing::info!("会话结束：无文本（未说话或未识别）");
        }
        // 上屏编排（阻塞：剪贴板 + 150ms 静默 + 注入），committed 事件由 commit 内发出
        let app2 = app.clone();
        let committed_text = text.clone();
        let result = tauri::async_runtime::spawn_blocking(move || {
            crate::commit::commit(&app2, &committed_text)
        })
        .await
        .unwrap_or(crate::commit::RESULT_COPIED);
        tracing::info!("提交结果: {result}");
        // 气泡：留 1.8s 给前端结果徽标展示与淡出，再隐藏窗口
        crate::bubble::hide_delayed(app.clone(), 1800);

        *self.state.lock().expect("state 锁") = SessionState::Idle;
        Self::emit_state(app, SessionState::Idle);
        Ok(())
    }
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new()
    }
}

/// collector：消费 ASR 事件 → 前端事件 + 文本累积；结束后自动 stop
struct CollectorHandles {
    app: AppHandle,
    manager: Arc<SessionManager>,
    shared: Arc<Mutex<Shared>>,
    done_tx: tokio::sync::mpsc::Sender<()>,
}

impl CollectorHandles {
    async fn run(self, mut evt_rx: tokio::sync::mpsc::Receiver<AsrEvent>) {
        while let Some(ev) = evt_rx.recv().await {
            match ev {
                AsrEvent::Started => {
                    tracing::info!("ASR task-started ✓");
                }
                AsrEvent::Partial(p) => {
                    self.shared.lock().expect("shared 锁").pending_partial = p.clone();
                    let _ = self.app.emit(events::PARTIAL, p);
                }
                AsrEvent::Sentence(s) => {
                    {
                        let mut sh = self.shared.lock().expect("shared 锁");
                        if !sh.committed.is_empty() {
                            sh.committed.push(' ');
                        }
                        sh.committed.push_str(&s);
                        sh.pending_partial.clear();
                    }
                    let _ = self.app.emit(events::SENTENCE, s);
                }
                AsrEvent::Failed { code, message } => {
                    Self::emit_error_static(&self.app, "asr", format!("{code}: {message}"));
                    break;
                }
                AsrEvent::Finished(_) => break, // 全文以 shared 累积为准
            }
        }
        let _ = self.done_tx.send(()).await;
        // ASR 侧终止（failed/断流）时自动收尾；正常 stop 先行则此处幂等忽略
        if let Err(e) = self.manager.stop(&self.app).await {
            tracing::error!("collector 自动收尾失败: {e}");
        }
    }

    fn emit_error_static(app: &AppHandle, source: &str, message: String) {
        tracing::error!(source, message = %message, "session error");
        let _ = app.emit(
            events::ERROR,
            ErrorPayload { source: source.into(), message },
        );
    }
}

pub fn build_asr_config(cfg: &Config, api_key: String) -> AsrConfig {
    let lang = cfg.asr.language.trim();
    AsrConfig {
        model: cfg.asr.model.clone(),
        ws_url: cfg.asr.ws_url.clone(),
        api_key,
        sample_rate: 16000,
        max_sentence_silence_ms: cfg.asr.silence_ms,
        semantic_punctuation: cfg.asr.semantic_punct,
        language_hints: if lang.is_empty() { None } else { Some(vec![lang.to_string()]) },
        prefer_ipv4: true,
    }
}
