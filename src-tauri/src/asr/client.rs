// ASR 会话客户端：异步 owner task 独占 WS 连接，会话句柄仅持有命令通道
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
    pub task_id: String,
}

enum Cmd {
    Pcm(Vec<i16>),
    Finish,
}

impl AsrSession {
    /// 连接服务端并发出 run-task，返回（会话句柄, 事件接收端）。
    /// task-started 到达前收到的音频会缓冲，Started 后按序发出。
    pub async fn start(cfg: AsrConfig) -> Result<(Self, mpsc::Receiver<AsrEvent>), String> {
        let task_id = protocol::new_task_id();
        let (cmd_tx, cmd_rx) = mpsc::channel::<Cmd>(64);
        let (evt_tx, evt_rx) = mpsc::channel::<AsrEvent>(64);

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
        let (ws, _resp) = tokio::time::timeout(
            CONNECT_TIMEOUT,
            tokio_tungstenite::connect_async(request),
        )
        .await
        .map_err(|_| "连接超时".to_string())?
        .map_err(|e| format!("连接失败: {e}"))?;

        tracing::info!(task_id, model = %cfg.model, "ASR 已连接，发送 run-task");
        tokio::spawn(owner_task(ws, cmd_rx, evt_tx, cfg, task_id.clone()));

        Ok((AsrSession { cmd_tx, task_id }, evt_rx))
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

    /// 结束识别：发 finish-task；owner 等 task-finished（≤2s）后发 Finished(全文)
    pub async fn finish(&self) -> Result<(), String> {
        self.cmd_tx
            .send(Cmd::Finish)
            .await
            .map_err(|_| "ASR 会话已关闭".to_string())
    }
}

async fn owner_task(
    ws: WebSocketStream<MaybeTlsStream<TcpStream>>,
    mut cmd_rx: mpsc::Receiver<Cmd>,
    evt_tx: mpsc::Sender<AsrEvent>,
    cfg: AsrConfig,
    task_id: String,
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

    let mut full_text = String::new();
    let mut started = false;
    let mut finishing = false;
    let mut pending: Vec<Vec<i16>> = Vec::new(); // task-started 前缓冲的音频

    loop {
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

        tokio::select! {
            cmd = cmd_rx.recv() => match cmd {
                Some(Cmd::Pcm(pcm)) => {
                    if started {
                        if let Err(e) = flush_pcm(&mut sink, &pcm).await {
                            let _ = evt_tx.send(AsrEvent::Failed { code: "send".into(), message: format!("发送音频失败: {e}") }).await;
                            break;
                        }
                    } else {
                        pending.push(pcm);
                    }
                }
                Some(Cmd::Finish) | None => {
                    // 句柄被整体丢弃（未显式 finish）也走正常收尾
                    if let Err(e) = sink.send(Message::Text(protocol::build_finish_task(&task_id).to_string().into())).await {
                        let _ = evt_tx.send(AsrEvent::Failed { code: "send".into(), message: format!("发送 finish-task 失败: {e}") }).await;
                    }
                    finishing = true;
                }
            },
            read = tokio::time::timeout(IDLE_TIMEOUT, stream.next()) => match read {
                Ok(Some(Ok(msg))) => {
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
                                // 冲缓冲
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
                _ => {
                    // 读空闲超时（断流）或连接被对端关闭
                    let _ = evt_tx.send(AsrEvent::Failed {
                        code: "idle".into(),
                        message: "服务端断流（读空闲超时或连接关闭）".into(),
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
