// ASR 会话客户端：异步 owner task 独占 WS 连接，会话句柄仅持有命令通道
use std::sync::Arc;
use super::protocol::{self, ServerEvent};
use futures_util::{SinkExt, StreamExt};
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

/// 单个二进制帧最多携带的 PCM 帧数（3200 帧 = 200ms @16kHz）
const MAX_PCM_PER_FRAME: usize = 3200;
/// 连接超时
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// 读空闲超时（超过视为断流）
const IDLE_TIMEOUT: Duration = Duration::from_secs(10);
/// select 空转 tick：兼顾 stop 标志响应性与空闲检测精度
const SELECT_TICK: Duration = Duration::from_millis(300);
/// finish-task 后等 task-finished 的兜底时长（PRD：≤2s）
const FINISH_TIMEOUT: Duration = Duration::from_secs(2);

/// ASR 会话参数（从应用配置映射）
#[derive(Debug, Clone)]
pub struct AsrConfig {
    pub model: String,
    pub ws_url: String,
    pub api_key: String,
    pub sample_rate: u32,
    pub max_sentence_silence_ms: u32,
    pub semantic_punctuation: bool,
    /// 留空 = 自动检测
    pub language_hints: Option<Vec<String>>,
    /// 优先 IPv4 连接（阿里云 v6 入口流式识别劣化）
    pub prefer_ipv4: bool,
}

/// 会话事件（owner task → 上层）
#[derive(Debug, Clone, PartialEq)]
pub enum AsrEvent {
    /// task-started，可以开始发音频
    Started,
    /// 中间结果
    Partial(String),
    /// 断句完成
    Sentence(String),
    /// 会话结束，携带全文（各句以空格拼接；失败中止时携带已识别部分）
    Finished(String),
    Failed { code: String, message: String },
}

/// 会话句柄（可 Clone）
#[derive(Clone)]
pub struct AsrSession {
    cmd_tx: mpsc::Sender<Cmd>,
    /// 停止请求标志：置位后 owner 在下个 tick（≤300ms）自行发 finish-task。
    /// 不依赖跨 task 唤醒，收尾路径 100% 确定性。
    stop_flag: Arc<std::sync::atomic::AtomicBool>,
    pub task_id: String,
}

enum Cmd {
    Pcm(Vec<i16>),
    Finish,
}

impl AsrSession {
    /// 连接服务端并发出 run-task，**等到 task-started 才返回**（返回即可实时发音频）。
    /// 首块音频与 Started 的微小竞态仍由 owner 缓冲兑底。
    pub async fn start(cfg: AsrConfig) -> Result<(Self, mpsc::Receiver<AsrEvent>), String> {
        let task_id = protocol::new_task_id();
        let (cmd_tx, cmd_rx) = mpsc::channel::<Cmd>(64);
        let (evt_tx, evt_rx) = mpsc::channel::<AsrEvent>(64);
        let stop_flag = Arc::new(std::sync::atomic::AtomicBool::new(false));

        let mut request = cfg
            .ws_url
            .clone()
            .into_client_request()
            .map_err(|e| format!("WebSocket 地址无效: {e}"))?;
        request.headers_mut().insert(
            "Authorization",
            format!("Bearer {}", cfg.api_key)
                .parse()
                .map_err(|e| format!("构造鉴权头失败: {e}"))?,
        );
        let (ws, _resp) = if cfg.prefer_ipv4 {
            // 优先 IPv4，并按序尝试每个地址（1.5s 超时轮换）：
            // 阿里云多个 A 记录中存在「能握手但收流卡死」的坏节点，必须允许跳过。
            use tokio::net::TcpStream;
            let host = request.uri().host().ok_or("ws_url 缺少 host")?.to_string();
            let port = request.uri().port_u16().unwrap_or(443);
            let mut addrs: Vec<std::net::SocketAddr> = tokio::net::lookup_host((host.as_str(), port))
                .await
                .map_err(|e| format!("DNS 解析失败({host}): {e}"))?
                .collect();
            // v4 在前，保留原有顺序（去重）
            addrs.sort_by_key(|a| !a.is_ipv4());
            addrs.dedup();
            if addrs.is_empty() {
                return Err(format!("无可用地表({host})"));
            }
            let mut tcp_err = String::new();
            let mut tcp = None;
            for addr in &addrs {
                match tokio::time::timeout(std::time::Duration::from_millis(1500), TcpStream::connect(addr)).await {
                    Ok(Ok(s)) => {
                        // 关 Nagle：音频帧需按 100ms 节奏即时上线
                        let _ = s.set_nodelay(true);
                        tcp = Some(s);
                        break;
                    }
                    Ok(Err(e)) => tcp_err.push_str(&format!("{addr}: {e}; ")),
                    Err(_) => tcp_err.push_str(&format!("{addr}: 超时; ")),
                }
            }
            let tcp = tcp.ok_or_else(|| format!("连接失败(已试 {} 个地址): {tcp_err}", addrs.len()))?;
            tokio::time::timeout(
                CONNECT_TIMEOUT,
                tokio_tungstenite::client_async_tls(request, tcp),
            )
            .await
            .map_err(|_| "TLS 握手超时".to_string())?
            .map_err(|e| format!("TLS/WS 握手失败: {e}"))?
        } else {
            tokio::time::timeout(
                CONNECT_TIMEOUT,
                tokio_tungstenite::connect_async(request),
            )
            .await
            .map_err(|_| "连接超时".to_string())?
            .map_err(|e| format!("连接失败: {e}"))?
        };

        tracing::info!(task_id, model = %cfg.model, "ASR 已连接，发送 run-task");
        let (started_tx, started_rx) = tokio::sync::oneshot::channel::<()>();
        tokio::spawn(owner_task(ws, cmd_rx, evt_tx.clone(), cfg, task_id.clone(), Some(started_tx), stop_flag.clone()));

        // 等 task-started（避免上层在 started 前积压音频 → flush 时 burst 变速失真）
        if tokio::time::timeout(std::time::Duration::from_secs(3), started_rx)
            .await
            .is_err()
        {
            let _ = evt_tx.send(AsrEvent::Failed {
                code: "started".into(),
                message: "服务端未确认任务（task-started 超时）".into(),
            }).await;
            return Err("ASR 任务未就绪（task-started 超时）".into());
        }

        Ok((AsrSession { cmd_tx, stop_flag, task_id }, evt_rx))
    }

    /// 发送一段 PCM（i16 LE 单声道）。task-started 前调用会缓冲。
    pub async fn send_pcm(&self, pcm: Vec<i16>) -> Result<(), String> {
        for chunk in pcm.chunks(MAX_PCM_PER_FRAME) {
            self.cmd_tx
                .send(Cmd::Pcm(chunk.to_vec()))
                .await
                .map_err(|_| "ASR 会话已关闭".to_string())?;
        }
        Ok(())
    }

    /// 阻塞版发送：供非 async 线程（如采集 worker）调用；
    /// 会话已关闭时立即报错。勿在 tokio 运行时线程内调用。
    pub fn send_pcm_blocking(&self, pcm: Vec<i16>) -> Result<(), String> {
        for chunk in pcm.chunks(MAX_PCM_PER_FRAME) {
            self.cmd_tx
                .blocking_send(Cmd::Pcm(chunk.to_vec()))
                .map_err(|_| "ASR 会话已关闭".to_string())?;
        }
        Ok(())
    }

    /// 结束识别：发 finish-task；owner 等 task-finished（≤2s）后发 Finished(全文)
    pub async fn finish(&self) -> Result<(), String> {
        self.cmd_tx
            .send(Cmd::Finish)
            .await
            .map_err(|_| "ASR 会话已关闭".to_string())
    }

    /// 请求停止：仅置标志，owner 在下个 tick 自行发 finish-task（无跨 task 唤醒依赖）
    pub fn request_stop(&self) {
        self.stop_flag
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }

}

async fn owner_task(
    ws: WebSocketStream<MaybeTlsStream<TcpStream>>,
    mut cmd_rx: mpsc::Receiver<Cmd>,
    evt_tx: mpsc::Sender<AsrEvent>,
    cfg: AsrConfig,
    task_id: String,
    mut started_tx: Option<tokio::sync::oneshot::Sender<()>>,
    stop_flag: Arc<std::sync::atomic::AtomicBool>,
) {
    let (mut sink, mut stream) = ws.split();

    let run_task = protocol::build_run_task(
        &cfg.model,
        cfg.sample_rate,
        cfg.max_sentence_silence_ms,
        cfg.semantic_punctuation,
        cfg.language_hints.as_deref(),
        &task_id,
    );
    if let Err(e) = sink.send(Message::Text(run_task.to_string().into())).await {
        let _ = evt_tx
            .send(AsrEvent::Failed { code: "send".into(), message: format!("发送 run-task 失败: {e}") })
            .await;
        return;
    }
    tracing::info!("run-task: {run_task}");

    let mut full_text = String::new();
    let mut started = false;
    let mut finishing = false;
    let mut pending: Vec<Vec<i16>> = Vec::new(); // task-started 前缓冲的音频

    let mut idle_acc = std::time::Duration::ZERO;

    'outer: loop {
        // 停止请求：由 owner 自己发 finish-task（确定性收尾，不依赖跨 task 唤醒）
        if !finishing && stop_flag.load(std::sync::atomic::Ordering::Relaxed) {
            tracing::info!("owner: 收到 stop 请求，发送 finish-task");
            if let Err(e) = sink
                .send(Message::Text(protocol::build_finish_task(&task_id).to_string().into()))
                .await
            {
                let _ = evt_tx
                    .send(AsrEvent::Failed { code: "send".into(), message: format!("发送 finish-task 失败: {e}") })
                    .await;
                break;
            }
            finishing = true;
        }
        if finishing {
            // 收尾：等 task-finished / 尾部断句，带兜底超时
            match tokio::time::timeout(FINISH_TIMEOUT, stream.next()).await {
                Ok(Some(Ok(msg))) => {
                    match handle_message(msg, &mut sink).await {
                        Ok(Some(ev)) => {
                            let done = matches!(ev, ServerEvent::TaskFinished | ServerEvent::Failed { .. });
                            on_server_event(ev, &evt_tx, &mut full_text).await;
                            if done {
                                break;
                            }
                        }
                        Ok(None) => {}
                        Err(e) => {
                            let _ = evt_tx
                                .send(AsrEvent::Failed { code: "send".into(), message: e })
                                .await;
                            break;
                        }
                    }
                }
                _ => break, // 超时/关闭：以已有内容收尾
            }
            continue;
        }

        // 非阻塞取空命令通道（无 waker 依赖）
        loop {
            match cmd_rx.try_recv() {
                Ok(Cmd::Pcm(pcm)) => {
                    if started {
                        if let Err(e) = flush_pcm(&mut sink, &pcm).await {
                            let _ = evt_tx.send(AsrEvent::Failed { code: "send".into(), message: format!("发送音频失败: {e}") }).await;
                            break 'outer;
                        }
                        tracing::info!("→ 已上线 {} 帧", pcm.len());
                    } else {
                        pending.push(pcm);
                    }
                }
                Ok(Cmd::Finish) => {
                    if let Err(e) = sink.send(Message::Text(protocol::build_finish_task(&task_id).to_string().into())).await {
                        let _ = evt_tx.send(AsrEvent::Failed { code: "send".into(), message: format!("发送 finish-task 失败: {e}") }).await;
                    }
                    finishing = true;
                    break;
                }
                Err(mpsc::error::TryRecvError::Disconnected) => {
                    if let Err(e) = sink.send(Message::Text(protocol::build_finish_task(&task_id).to_string().into())).await {
                        let _ = evt_tx.send(AsrEvent::Failed { code: "send".into(), message: format!("发送 finish-task 失败: {e}") }).await;
                    }
                    finishing = true;
                    break;
                }
                Err(mpsc::error::TryRecvError::Empty) => break,
            }
        }

        // 阻塞等下一条服务端消息（最多 SELECT_TICK，醒来后回到循环顶检查 stop 标志）
        match tokio::time::timeout(SELECT_TICK, stream.next()).await {
            Ok(Some(Ok(msg))) => {
                idle_acc = std::time::Duration::ZERO;
                match &msg {
                    Message::Text(t) => tracing::info!("← text: {}", t.as_str()),
                    _ => {}
                }
                match handle_message(msg, &mut sink).await {
                    Ok(Some(ev)) => {
                        let is_failed = matches!(ev, ServerEvent::Failed { .. });
                        let is_started = matches!(ev, ServerEvent::TaskStarted);
                        on_server_event(ev, &evt_tx, &mut full_text).await;
                        if is_failed {
                            break;
                        }
                        if is_started {
                            started = true;
                            let _ = evt_tx.send(AsrEvent::Started).await;
                            if let Some(tx) = started_tx.take() {
                                let _ = tx.send(());
                            }
                            // 冲缓冲（竞态兑底，此时仅极少量音频）
                            for p in pending.drain(..) {
                                if let Err(e) = flush_pcm(&mut sink, &p).await {
                                    let _ = evt_tx.send(AsrEvent::Failed { code: "send".into(), message: format!("发送音频失败: {e}") }).await;
                                    break;
                                }
                            }
                        }
                    }
                    Ok(None) => {}
                    Err(e) => {
                        let _ = evt_tx.send(AsrEvent::Failed { code: "send".into(), message: e }).await;
                        break;
                    }
                }
            }
            Ok(Some(Err(e))) => {
                let _ = evt_tx.send(AsrEvent::Failed {
                    code: "read".into(),
                    message: format!("读流出错: {e}"),
                }).await;
                break;
            }
            Ok(None) => {
                // 连接被对端关闭
                let _ = evt_tx.send(AsrEvent::Failed {
                    code: "idle".into(),
                    message: "服务端断流（连接关闭）".into(),
                }).await;
                break;
            }
            Err(_) => {
                // tick 到期无数据：累计空闲，超阈值判定断流；否则回循环顶（检查 stop 标志）
                idle_acc += SELECT_TICK;
                if idle_acc >= IDLE_TIMEOUT {
                    let _ = evt_tx.send(AsrEvent::Failed {
                        code: "idle".into(),
                        message: "服务端断流（读空闲超时）".into(),
                    }).await;
                    break;
                }
            }
        }
    }

    let _ = evt_tx.send(AsrEvent::Finished(full_text)).await;
    let _ = sink.close().await;
    tracing::info!("ASR owner 退出");
}

/// 转发 Partial/Sentence 事件
async fn on_server_event(ev: ServerEvent, evt_tx: &mpsc::Sender<AsrEvent>, full_text: &mut String) {
    match ev {
        ServerEvent::Partial(p) => {
            let _ = evt_tx.send(AsrEvent::Partial(p)).await;
        }
        ServerEvent::Sentence(s) => {
            if !full_text.is_empty() {
                full_text.push(' ');
            }
            full_text.push_str(&s);
            tracing::info!(sentence = %s, "ASR 断句");
            let _ = evt_tx.send(AsrEvent::Sentence(s)).await;
        }
        ServerEvent::TaskStarted => {}
        ServerEvent::TaskFinished => {}
        ServerEvent::Failed { code, message } => {
            tracing::error!(code = %code, message = %message, "ASR task-failed");
            let _ = evt_tx.send(AsrEvent::Failed { code, message }).await;
        }
        ServerEvent::Ignored => {}
    }
}

/// 处理一条服务端消息；返回解析出的事件（None = 忽略/心跳）
async fn handle_message<S>(msg: Message, sink: &mut S) -> Result<Option<ServerEvent>, String>
where
    S: SinkExt<Message> + Unpin,
    <S as futures_util::Sink<Message>>::Error: std::fmt::Display,
{
    match msg {
        Message::Text(t) => {
            let ev = protocol::parse_server_message(&t);
            if matches!(ev, ServerEvent::Ignored) {
                Ok(None)
            } else {
                Ok(Some(ev))
            }
        }
        Message::Ping(p) => sink
            .send(Message::Pong(p))
            .await
            .map_err(|e| format!("回复 Pong 失败: {e}"))
            .map(|_| None),
        Message::Close(_) => Err("服务端关闭连接".into()),
        _ => Ok(None),
    }
}

/// 把 PCM 切成 ≤MAX_PCM_PER_FRAME 的二进制帧发送（i16 LE）
async fn flush_pcm<S>(sink: &mut S, pcm: &[i16]) -> Result<(), String>
where
    S: SinkExt<Message> + Unpin,
    <S as futures_util::Sink<Message>>::Error: std::fmt::Display,
{
    for chunk in pcm.chunks(MAX_PCM_PER_FRAME) {
        let bytes: Vec<u8> = chunk.iter().flat_map(|s| s.to_le_bytes()).collect();
        sink.send(Message::Binary(bytes.into()))
            .await
            .map_err(|e| format!("{e}"))?;
    }
    Ok(())
}
