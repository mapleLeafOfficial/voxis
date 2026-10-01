// Voxis 日志初始化 — tracing → 控制台 + ~/.local/share/voxis/logs/voxis.log（按天滚动）
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt::writer::MakeWriterExt;

pub fn init(level: &str) {
    let dir = crate::config::data_dir().join("logs");
    if let Err(e) = std::fs::create_dir_all(&dir) {
        eprintln!("创建日志目录失败({e})，仅输出到控制台");
    }

    let filter = EnvFilter::try_new(level).unwrap_or_else(|_| EnvFilter::new("info"));

    let file_appender = tracing_appender::rolling::daily(&dir, "voxis.log");
    let (writer, guard) = tracing_appender::non_blocking(file_appender);
    // guard 需与进程同生命周期，否则文件写入静默停止
    std::mem::forget(guard);

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(writer.and(std::io::stdout))
        .with_target(false)
        .init();
}
