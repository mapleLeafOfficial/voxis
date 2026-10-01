// 采集管理器：cpal::Stream 不是 Send，不能进 managed state；
// 用 owner 线程独占持有流，state 只存命令通道（Actor 模式）
use super::capture::{CaptureCallbacks, CaptureStream};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Mutex;
use std::time::Duration;

enum OwnerCmd {
    Start {
        device: Option<String>,
        max_duration: Option<Duration>,
        callbacks: CaptureCallbacks,
        reply: Sender<Result<(), String>>,
    },
    Stop {
        reply: Sender<Result<(), String>>,
    },
}

pub struct CaptureManager {
    tx: Mutex<Option<Sender<OwnerCmd>>>,
}

impl CaptureManager {
    pub fn new() -> Self {
        CaptureManager { tx: Mutex::new(None) }
    }

    /// 懒启动 owner 线程并返回其命令通道
    fn owner(&self) -> Result<Sender<OwnerCmd>, String> {
        let mut guard = self.tx.lock().map_err(|e| format!("采集管理锁中毒: {e}"))?;
        if guard.is_none() {
            let (tx, rx) = channel::<OwnerCmd>();
            std::thread::Builder::new()
                .name("voxis-capture-owner".into())
                .spawn(move || owner_loop(rx))
                .map_err(|e| format!("启动采集管理线程失败: {e}"))?;
            *guard = Some(tx);
        }
        Ok(guard.as_ref().expect("刚初始化").clone())
    }

    pub fn start(
        &self,
        device: Option<String>,
        max_duration: Option<Duration>,
        callbacks: CaptureCallbacks,
    ) -> Result<(), String> {
        let owner = self.owner()?;
        let (rtx, rrx) = channel();
        owner
            .send(OwnerCmd::Start { device, max_duration, callbacks, reply: rtx })
            .map_err(|_| "采集管理线程已退出".to_string())?;
        rrx.recv().map_err(|_| "采集管理线程无响应".to_string())?
    }

    pub fn stop(&self) -> Result<(), String> {
        let owner = self.owner()?;
        let (rtx, rrx) = channel();
        owner
            .send(OwnerCmd::Stop { reply: rtx })
            .map_err(|_| "采集管理线程已退出".to_string())?;
        rrx.recv().map_err(|_| "采集管理线程无响应".to_string())?
    }
}

fn owner_loop(rx: Receiver<OwnerCmd>) {
    // Stream 只在本线程内创建/销毁，规避 Send 限制
    let mut current: Option<CaptureStream> = None;
    while let Ok(cmd) = rx.recv() {
        match cmd {
            OwnerCmd::Start { device, max_duration, callbacks, reply } => {
                if current.is_some() {
                    let _ = reply.send(Err("已在采集中".into()));
                    continue;
                }
                match CaptureStream::start(device.as_deref(), max_duration, callbacks) {
                    Ok(s) => {
                        current = Some(s);
                        let _ = reply.send(Ok(()));
                    }
                    Err(e) => {
                        let _ = reply.send(Err(e));
                    }
                }
            }
            OwnerCmd::Stop { reply } => match current.take() {
                Some(s) => {
                    s.stop();
                    let _ = reply.send(Ok(()));
                }
                None => {
                    let _ = reply.send(Err("当前没有进行中的采集".into()));
                }
            },
        }
    }
    // 线程退出：清掉残留流
    if let Some(s) = current.take() {
        s.stop();
    }
}
