// 采集引擎冒烟测试：录 3 秒，打印音量电平与 PCM 统计
// 运行：cargo run --example capture_smoke
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use voxis_lib::audio::capture::{CaptureCallbacks, CaptureStream};

fn main() {
    let chunks = Arc::new(AtomicUsize::new(0));
    let frames = Arc::new(AtomicUsize::new(0));
    let peak = Arc::new(std::sync::atomic::AtomicU64::new(0));

    let (done_tx, done_rx) = std::sync::mpsc::channel::<()>();

    let c_chunks = chunks.clone();
    let c_frames = frames.clone();
    let c_peak = peak.clone();
    let stream = CaptureStream::start(
        None,
        Some(Duration::from_secs(3)),
        CaptureCallbacks {
            on_volume: Box::new(move |level| {
                let bits = c_peak.load(Ordering::Relaxed);
                let new = ((bits as f64).max(level * 1_000_000.0)) as u64;
                c_peak.store(new, Ordering::Relaxed);
            }),
            on_pcm: Box::new(move |chunk| {
                c_chunks.fetch_add(1, Ordering::Relaxed);
                c_frames.fetch_add(chunk.len(), Ordering::Relaxed);
            }),
            on_max_duration: Box::new(move || {
                let _ = done_tx.send(());
            }),
            on_error: Box::new(|msg| eprintln!("[error] {msg}")),
        },
    )
    .expect("启动采集失败");

    println!("采集中 3 秒，请对麦克风说话……");
    let _ = done_rx.recv_timeout(Duration::from_secs(5));
    stream.stop();

    let c = chunks.load(Ordering::Relaxed);
    let f = frames.load(Ordering::Relaxed);
    let p = peak.load(Ordering::Relaxed) as f64 / 1_000_000.0;
    println!("PCM 块数: {c}, 总帧数: {f} ({:.2}s @16kHz), 音量峰值: {p:.3}", f as f64 / 16000.0);
    if c > 0 {
        println!("✅ 采集引擎工作正常");
    } else {
        eprintln!("❌ 未收到 PCM 数据");
        std::process::exit(1);
    }
}
