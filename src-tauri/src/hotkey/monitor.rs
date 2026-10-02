// 平台无关的键盘事件与监控句柄：Linux(evdev) / Windows(低级钩子) 产出同样的事件流。
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;

/// 归并后的原始按键事件（规范名 + 边沿）——状态机输入，平台无关
#[derive(Debug, Clone)]
pub struct RawKeyEvent {
    pub name: String,
    pub pressed: bool,
}

/// 监听器句柄：drop 时置 stop 标志并 join 线程（平台无关）。
/// Windows 钩子线程阻塞在消息循环上，需 quitter 主动发 WM_QUIT 才能退。
pub struct MonitorHandle {
    stop: Arc<AtomicBool>,
    handles: Vec<JoinHandle<()>>,
    quitter: Option<Box<dyn FnOnce() + Send + Sync>>,
}

impl MonitorHandle {
    pub fn new(stop: Arc<AtomicBool>, handles: Vec<JoinHandle<()>>) -> Self {
        Self { stop, handles, quitter: None }
    }

    /// 带主动退出回调（Windows 消息循环用）
    pub fn with_quitter(
        stop: Arc<AtomicBool>,
        handles: Vec<JoinHandle<()>>,
        quitter: Box<dyn FnOnce() + Send + Sync>,
    ) -> Self {
        Self { stop, handles, quitter: Some(quitter) }
    }
}

impl Drop for MonitorHandle {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(q) = self.quitter.take() {
            q();
        }
        for h in self.handles.drain(..) {
            let _ = h.join();
        }
    }
}
