// ASR 冒烟测试：生成/读取 16kHz 语音 → 完整流式会话 → 打印事件与全文
//
// 用法：
//   cargo run --example asr_smoke                     # espeak 生成中文语音并转写
//   cargo run --example asr_smoke -- --wav x.wav      # 转写指定 16kHz/mono/s16 PCM WAV
//   cargo run --example asr_smoke -- --model qwen3-asr-flash-streaming
//   cargo run --example asr_smoke -- --bad-key        # 验证错误 Key → task-failed
//
// Key 来源：$QWEN_API_KEY / $DASHSCOPE_API_KEY > voice_input 配置 ~/.config/qwen-voice-input/config.ini
use std::time::{Duration, Instant};
use voxis_lib::asr::client::{AsrConfig, AsrEvent, AsrSession};
use voxis_lib::asr::key::resolve_api_key;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let flag = |name: &str| args.iter().any(|a| a == name);
    let use_tauri_rt = flag("--tauri-rt");
    let value_after = |name: &str| {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    let ws_url = value_after("--url").unwrap_or_else(|| "wss://dashscope.aliyuncs.com/api-ws/v1/inference".into());

    let model = value_after("--model").unwrap_or_else(|| "qwen-audio-3.0-asr-flash-streaming".into());
    let bad_key = flag("--bad-key");
    let wav_path = value_after("--wav");

    let mut api_key = resolve_api_key("")
        .or_else(|| read_voice_input_key())
        .expect("未找到 API Key（设 QWEN_API_KEY 或配置 voice_input config.ini）");
    if bad_key {
        api_key = format!("sk-invalid-{}", &api_key[3..10].to_string());
    }
    println!("模型: {model}, Key: sk-…{} (len={})", &api_key[api_key.len()-4..], api_key.len());

    let pcm = match wav_path {
        Some(p) => load_wav_16k_mono(&p),
        None => {
            let tmp = "/tmp/voxis_asr_smoke.wav";
            gen_speech_wav(tmp);
            load_wav_16k_mono(tmp)
        }
    };
    println!("PCM: {} 帧 ({:.2}s)", pcm.len(), pcm.len() as f64 / 16000.0);

    let closure = || {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(run_session(AsrConfig {
            model: model.clone(),
            ws_url: ws_url.clone(),
            api_key: api_key.clone(),
            sample_rate: 16000,
            max_sentence_silence_ms: 1300,
            semantic_punctuation: true,
            language_hints: None,
                        prefer_ipv4: true,
        }, pcm.clone(), bad_key))
    };
    if use_tauri_rt {
        tauri::async_runtime::block_on(async {
            // 用 Tauri 托管 runtime 跑同一逻辑（对照实验）
            let t0 = Instant::now();
            let cfg = AsrConfig {
                model,
                ws_url,
                api_key,
                sample_rate: 16000,
                max_sentence_silence_ms: 1300,
                semantic_punctuation: true,
                language_hints: None,
                        prefer_ipv4: true,
            };
            let r = run_session(cfg, pcm, bad_key).await;
            println!("[tauri-rt] 总耗时 {:.1}s", t0.elapsed().as_secs_f32());
            r
        });
    } else {
        closure();
    }
}

async fn run_session(cfg: AsrConfig, pcm: Vec<i16>, expect_fail: bool) {
    let t0 = Instant::now();
    let (session, mut events) = match AsrSession::start(cfg).await {
        Ok(x) => x,
        Err(e) => {
            eprintln!("❌ 连接/启动失败: {e}");
            std::process::exit(1);
        }
    };

    // 事件打印（独立 task，带时间戳）
    let printer = tokio::spawn(async move {
        let mut saw_partial = false;
        let mut saw_sentence = false;
        let mut full: Option<String> = None;
        let mut failure: Option<String> = None;
        while let Some(ev) = events.recv().await {
            let t = t0.elapsed().as_millis();
            match ev {
                AsrEvent::Started => println!("[{t:5}ms] ▶ task-started"),
                AsrEvent::Partial(p) => {
                    saw_partial = true;
                    println!("[{t:5}ms] … {p}");
                }
                AsrEvent::Sentence(s) => {
                    saw_sentence = true;
                    println!("[{t:5}ms] ✔ 句: {s}");
                }
                AsrEvent::Finished(f) => {
                    full = Some(f);
                    println!("[{t:5}ms] ■ finished");
                }
                AsrEvent::Failed { code, message } => {
                    failure = Some(format!("{code}: {message}"));
                    println!("[{t:5}ms] ✗ failed {code}: {message}");
                }
            }
        }
        (saw_partial, saw_sentence, full, failure)
    });

    // 实时节奏发送（1600 帧 = 100ms）；会话失败后发送会报错，优雅退出
    // --delay-finish <ms>：模拟 SessionManager 式延迟收尾（泵完后再等 N ms 才 finish）
    let delay_finish: u64 = std::env::args()
        .position(|a| a == "--delay-finish")
        .and_then(|i| std::env::args().nth(i + 1))
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    for chunk in pcm.chunks(1600) {
        if session.send_pcm(chunk.to_vec()).await.is_err() {
            println!("── 会话已关闭，停止发送 ──");
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    if delay_finish > 0 {
        println!("── 泵完，延迟 {delay_finish}ms 后 finish ──");
        tokio::time::sleep(Duration::from_millis(delay_finish)).await;
    }
    let _ = session.finish().await;
    println!("── 音频发送完毕，finish-task 已发出 ──");

    let (saw_partial, saw_sentence, full, failure) = printer.await.unwrap();
    if expect_fail {
        match failure {
            Some(f) => println!("✅ 错误路径验证通过（task-failed: {f}）"),
            None => eprintln!("❌ 预期失败但未收到 Failed 事件"),
        }
        return;
    }
    match (full, failure) {
        (Some(f), None) => {
            println!("\n全文: {f}");
            if saw_sentence && saw_partial {
                println!("✅ 冒烟通过（partial→sentence_end 序列完整）");
            } else {
                println!("✅ 冒烟通过（提示：未观察到 partial/断句序列，句子可能过短）");
            }
        }
        (Some(f), Some(err)) => {
            println!("\n部分文本: {f}\n⚠️ 会话带错误结束: {err}");
            std::process::exit(2);
        }
        (None, _) => {
            eprintln!("❌ 未收到 Finished 事件");
            std::process::exit(1);
        }
    }
}

/// espeak-ng 生成中文测试语音（22050Hz）→ ffmpeg 转 16k/mono/s16
fn gen_speech_wav(path: &str) {
    let raw = "/tmp/voxis_asr_smoke_raw.wav";
    let st = std::process::Command::new("espeak-ng")
        .args(["-v", "zh", "-s", "140", "-w", raw, "你好，欢迎使用语音输入。今天天气怎么样？"])
        .status()
        .expect("espeak-ng");
    assert!(st.success());
    let st = std::process::Command::new("ffmpeg")
        .args(["-y", "-loglevel", "error", "-i", raw, "-ar", "16000", "-ac", "1", "-sample_fmt", "s16", path])
        .status()
        .expect("ffmpeg");
    assert!(st.success());
}

/// 解析 16kHz/单声道/PCM16 WAV（仅支持标准 RIFF，逐 chunk 走）
fn load_wav_16k_mono(path: &str) -> Vec<i16> {
    let b = std::fs::read(path).expect("读 wav");
    assert_eq!(&b[0..4], b"RIFF");
    let mut i = 12usize;
    let (mut rate, mut ch, mut bits, mut data) = (0u32, 0u16, 0u16, &[][..]);
    while i + 8 <= b.len() {
        let id = &b[i..i + 4];
        let size = u32::from_le_bytes(b[i + 4..i + 8].try_into().unwrap()) as usize;
        if id == b"fmt " {
            ch = u16::from_le_bytes(b[i + 10..i + 12].try_into().unwrap());
            rate = u32::from_le_bytes(b[i + 12..i + 16].try_into().unwrap());
            bits = u16::from_le_bytes(b[i + 22..i + 24].try_into().unwrap());
        } else if id == b"data" {
            data = &b[i + 8..(i + 8 + size).min(b.len())];
        }
        i += 8 + size + (size & 1);
    }
    assert_eq!((rate, ch, bits), (16000, 1, 16), "需 16kHz/mono/PCM16");
    data.chunks_exact(2)
        .map(|c| i16::from_le_bytes([c[0], c[1]]))
        .collect()
}

/// 兜底：从 voice_input 配置读 key（本机便捷）
fn read_voice_input_key() -> Option<String> {
    let p = dirs::home_dir()?.join(".config/qwen-voice-input/config.ini");
    let text = std::fs::read_to_string(p).ok()?;
    let mut in_auth = false;
    for line in text.lines() {
        let l = line.trim();
        if l.starts_with('[') {
            in_auth = l.eq_ignore_ascii_case("[auth]");
        } else if in_auth && l.starts_with("api_key") {
            return l.split_once('=').map(|(_, v)| v.trim().to_string());
        }
    }
    None
}
