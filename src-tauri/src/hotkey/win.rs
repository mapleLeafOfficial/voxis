// Windows 热键源（todo2 实现真钩子）：当前为编译桩，产出空事件流。
// 真实现：SetWindowsHookExW(WH_KEYBOARD_LL) + 消息循环线程，vkCode → 规范名。
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::Sender;
use std::sync::Arc;

use super::monitor::MonitorHandle;
use super::RawKeyEvent;

/// 桩实现：起一个空转线程占住句柄（drop 即退），设备数报 1 使引擎流程可达状态机。
pub fn spawn_monitors(_tx: Sender<RawKeyEvent>) -> (MonitorHandle, usize) {
    tracing::debug!("[hotkey] windows 桩源（todo2 实现 WH_KEYBOARD_LL）");
    let stop = Arc::new(AtomicBool::new(false));
    let stop2 = stop.clone();
    let handle = std::thread::spawn(move || {
        while !stop2.load(std::sync::atomic::Ordering::Relaxed) {
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
    });
    (MonitorHandle::new(stop, vec![handle]), 1)
}
