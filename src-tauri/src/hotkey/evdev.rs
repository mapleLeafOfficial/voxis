// evdev 设备监听：枚举 /dev/input/event* 中具备键盘能力的设备，
// 每设备一个阻塞读线程（fetch_events 底层为阻塞 read，空闲 CPU 0），事件经 mpsc 汇聚。
// v1 只在启动时枚举一次；热插拔监听（udev）为 P2。
use std::fs::read_dir;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};

use evdev::{Device, EventType, KeyCode};

/// 归并后的原始按键事件（规范名 + 边沿）
#[derive(Debug, Clone)]
pub struct RawKeyEvent {
    pub name: String,
    pub pressed: bool,
}

/// 监听器句柄：drop 时置 stop 标志并 join 线程
pub struct EvdevMonitor {
    stop: Arc<AtomicBool>,
    handles: Vec<JoinHandle<()>>,
}

impl Drop for EvdevMonitor {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        for h in self.handles.drain(..) {
            let _ = h.join();
        }
    }
}

/// 判断设备是否具备键盘能力：有基本字母键 + 空格 + Ctrl
fn is_keyboard(dev: &Device) -> bool {
    dev.supported_keys()
        .map(|keys| {
            keys.contains(KeyCode::KEY_A)
                && keys.contains(KeyCode::KEY_SPACE)
                && keys.contains(KeyCode::KEY_LEFTCTRL)
        })
        .unwrap_or(false)
}

/// 枚举可读的键盘设备路径
fn enumerate_keyboards() -> Vec<(PathBuf, String)> {
    let mut out = Vec::new();
    let Ok(entries) = read_dir("/dev/input") else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| n.starts_with("event"))
            .unwrap_or(false)
        {
            continue;
        }
        let Ok(dev) = Device::open(&path) else {
            continue; // 无权限或非输入设备，跳过
        };
        if is_keyboard(&dev) {
            let name = dev.name().unwrap_or("unknown").to_string();
            out.push((path, name));
        }
    }
    out
}

/// 启动所有键盘设备的阻塞读线程，事件汇聚到 tx。返回监听器句柄（drop 即停）。
pub fn spawn_monitors(tx: std::sync::mpsc::Sender<RawKeyEvent>) -> (EvdevMonitor, usize) {
    let stop = Arc::new(AtomicBool::new(false));
    let devices = enumerate_keyboards();
    let mut handles = Vec::new();

    for (path, name) in devices {
        let tx = tx.clone();
        let stop = stop.clone();
        let handle = thread::Builder::new()
            .name(format!("evdev:{name}"))
            .spawn(move || {
                if let Err(e) = read_loop(&path, &stop, &tx) {
                    tracing::warn!("evdev 读循环退出 {name}: {e}");
                }
            })
            .expect("spawn evdev 线程");
        handles.push(handle);
    }

    let n = handles.len();
    (EvdevMonitor { stop, handles }, n)
}

/// 单设备阻塞读循环：只读不吞（事件仍留在内核供其他读者），EV_KEY 边沿事件发到 tx
fn read_loop(
    path: &PathBuf,
    stop: &AtomicBool,
    tx: &std::sync::mpsc::Sender<RawKeyEvent>,
) -> std::io::Result<()> {
    let mut dev = Device::open(path)?;
    loop {
        if stop.load(Ordering::Relaxed) {
            return Ok(());
        }
        // fetch_events 底层阻塞 read；SYN_DROPPED 时 evdev 会合成同步事件保持状态正确
        for ev in dev.fetch_events()? {
            if ev.event_type() != EventType::KEY {
                continue;
            }
            let pressed = match ev.value() {
                1 => true,
                0 => false,
                _ => continue, // 2 = 长按 repeat，忽略
            };
            let Some(name) = keys::canonical_name(KeyCode::new(ev.code())).map(str::to_string) else {
                continue;
            };
            if tx.send(RawKeyEvent { name, pressed }).is_err() {
                return Ok(()); // 接收端已关闭
            }
        }
    }
}

use crate::hotkey::keys;
