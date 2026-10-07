use std::path::{Path, PathBuf};
use std::time::Duration;

use async_trait::async_trait;
use scroll_format_core::{
    AppError, ConvertContext, ConvertOutput, Converter, FormatInfo, PreviewData,
};
use scroll_format_infra::{find_tool, ProcessRunner, RunSpec};

fn ffmpeg() -> Result<PathBuf, AppError> {
    find_tool("ffmpeg").ok_or_else(|| AppError::tool_missing("未找到 ffmpeg，请安装或将 sidecar 放入 sidecars/"))
}
fn ffprobe() -> Result<PathBuf, AppError> {
    find_tool("ffprobe").ok_or_else(|| AppError::tool_missing("未找到 ffprobe"))
}

async fn probe_media(input: &Path, kind: scroll_format_core::FormatKind) -> Result<FormatInfo, AppError> {
    let probe = ffprobe()?;
    let out = ProcessRunner::run(RunSpec {
        program: probe,
        args: vec![
            // 用 error 级别而非 quiet：探测失败时 stderr 才有原因（如 moov atom not found），
            // 否则上层只能给出「无法解析」这类模糊提示
            "-v".into(), "error".into(),
            "-print_format".into(), "json".into(),
            "-show_format".into(), "-show_streams".into(),
            input.to_string_lossy().to_string(),
        ],
        timeout: Some(Duration::from_secs(30)),
        cancel: None,
        on_stdout: None,
        on_stderr: None,
    })
    .await?;
    let v: serde_json::Value = serde_json::from_str(&out.stdout).unwrap_or(serde_json::Value::Null);
    let format = v.get("format").cloned().unwrap_or_default();
    let streams = v.get("streams").and_then(|s| s.as_array()).cloned().unwrap_or_default();
    let video = streams.iter().find(|s| s["codec_type"] == "video");
    let audio = streams.iter().find(|s| s["codec_type"] == "audio");
    let primary = video.or(audio);
    let media = scroll_format_core::format::MediaInfo {
        duration_secs: format["duration"].as_str().and_then(|s| s.parse::<f64>().ok()),
        width: video.and_then(|s| s["width"].as_u64()).map(|w| w as u32),
        height: video.and_then(|s| s["height"].as_u64()).map(|h| h as u32),
        codec: primary.and_then(|s| s["codec_name"].as_str()).map(|s| s.to_string()),
        bitrate: format["bit_rate"].as_str().and_then(|s| s.parse().ok()),
        sample_rate: audio.and_then(|s| s["sample_rate"].as_str()).and_then(|s| s.parse().ok()),
        channels: audio.and_then(|s| s["channels"].as_u64()).map(|c| c as u32),
    };
    let size = std::fs::metadata(input).map(|m| m.len()).unwrap_or(0);
    Ok(FormatInfo {
        path: input.to_string_lossy().to_string(),
        ext: input.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase(),
        kind,
        size,
        display_name: input.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default(),
        media: Some(media),
    })
}

/// 把 ffmpeg 的原始报错翻译成「人能看懂 + 知道怎么做」的中文提示
fn friendly_ffmpeg_error(msg: &str) -> Option<&'static str> {
    let m = msg.to_ascii_lowercase();
    let has = |k: &str| m.contains(k);
    if has("moov atom not found") {
        return Some("源文件已损坏或不完整：MP4 缺少 moov 索引（常见于下载/复制中断），请重新获取源文件");
    }
    if has("invalid data found when processing input") || has("invalid data found") {
        return Some("源文件已损坏或不完整（MP4/MKV 缺少索引信息，常见于下载或复制中断），请重新获取源文件");
    }
    if has("permission denied") || has("access is denied") {
        return Some("没有访问权限：文件可能正被其他程序占用，或输出目录不可写");
    }
    if has("no such file or directory") || has("cannot find the file") {
        return Some("文件不存在或已被移动/删除，请重新添加");
    }
    if has("unknown encoder") || has("decoder not found") {
        return Some("当前 ffmpeg 不包含所需编解码器，请更换编码格式或更新 ffmpeg");
    }
    if has("nvcuda") || has("cannot load library") || has("nvenc capable") || has("nvidia") {
        return Some("显卡驱动问题：无法初始化 NVENC，请更新 NVIDIA 驱动或改用软件编码");
    }
    if has("no space left") || has("not enough space") {
        return Some("磁盘空间不足，请更换输出目录或清理空间");
    }
    if has("unknown format") || has("invalid data") && has("output") {
        return Some("输出容器与所选编码器不兼容，请更换输出格式");
    }
    None
}

/// 转换前先体检源文件：坏文件直接给出明确原因，避免跑完再报一堆天书
async fn ensure_input_readable(input: &Path) -> Result<(), AppError> {
    if !input.exists() {
        return Err(AppError::io("源文件不存在或已被移动/删除，请重新添加"));
    }
    let kind = scroll_format_core::FormatKind::Video;
    if let Err(e) = probe_media(input, kind).await {
        let detail = friendly_ffmpeg_error(&e.message)
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("源文件无法解析（{}），文件可能已损坏或格式不支持", e.message));
        return Err(AppError::new(scroll_format_core::ErrorCode::ConvertFailed, detail));
    }
    Ok(())
}

/// 不同编码器的 `-preset` 取值体系不同，需按族映射，否则 ffmpeg 会报
/// "Unable to parse option value" 而失败。
fn map_preset(codec: &str, preset: &str) -> Option<String> {
    // NVENC：p1(最快) ~ p7(最慢)
    let nvenc = |p: &str| -> String {
        match p {
            "ultrafast" => "p1", "superfast" => "p2", "veryfast" => "p3", "faster" => "p4",
            "fast" => "p5", "medium" => "p5", "slow" => "p6", "slower" => "p7", _ => "p5",
        }
        .to_string()
    };
    // QSV：1(最快) ~ 7(最慢)
    let qsv = |p: &str| -> String {
        match p {
            "ultrafast" => "1", "superfast" => "2", "veryfast" => "3", "faster" => "4",
            "fast" => "5", "medium" => "5", "slow" => "6", "slower" => "7", _ => "5",
        }
        .to_string()
    };
    // VP8/VP9/AV1(libaom)：realtime / good / best
    let vpx = |p: &str| -> String {
        match p {
            "ultrafast" | "superfast" | "veryfast" => "realtime",
            "faster" | "fast" => "good",
            "slow" | "slower" | "veryslow" => "best",
            _ => "medium",
        }
        .to_string()
    };
    if codec.contains("nvenc") {
        return Some(nvenc(preset));
    }
    if codec.contains("qsv") {
        return Some(qsv(preset));
    }
    if codec.contains("vpx") || codec.contains("aom") {
        return Some(vpx(preset));
    }
    if codec.contains("amf") {
        // AMF 使用 -quality，而非 -preset
        return None;
    }
    // libx264 / libx265 / mpeg4 等沿用原名
    Some(preset.to_string())
}

/// 提取用户自定义 ffmpeg 参数（参数数组透传，不做 shell 解析）
fn extra_args(extra: &serde_json::Value) -> Vec<String> {
    extra
        .get("custom_args")
        .and_then(|v| v.as_str())
        .map(|s| {
            s.split_whitespace()
                .map(|x| x.trim().trim_matches('"').to_string())
                .filter(|x| !x.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

/// 通过 `-progress pipe:1` 解析 ffmpeg 进度
async fn run_ffmpeg_with_progress(ctx: &ConvertContext, args: Vec<String>, duration: Option<f64>) -> Result<(), AppError> {
    let ff = ffmpeg()?;
    let (tx, mut rx) = tokio::sync::mpsc::channel::<String>(64);
    let ctx_progress = ctx.progress.clone();
    let cancel = ctx.cancel.clone();
    tokio::spawn(async move {
        while let Some(line) = rx.recv().await {
            if let Some(rest) = line.strip_prefix("out_time_us=") {
                if let (Some(us), Some(d)) = (rest.trim().parse::<f64>().ok(), duration) {
                    if d > 0.0 {
                        let p = (us / 1e6 / d).clamp(0.0, 1.0) as f32;
                        let _ = ctx_progress.send(p).await;
                    }
                }
            }
        }
    });

    let mut full_args: Vec<String> = vec!["-nostdin".into(), "-y".into(), "-progress".into(), "pipe:1".into()];
    full_args.extend(args);

    let on_stdout = Box::new(move |line: &str| {
        let _ = tx.try_send(line.to_string());
    });
    let log_cb = ctx.log.clone();
    let on_stderr = Box::new(move |line: &str| {
        let _ = log_cb.try_send(line.to_string());
    });

    ProcessRunner::run(RunSpec {
        program: ff,
        args: full_args,
        timeout: None,
        cancel: Some(cancel),
        on_stdout: Some(on_stdout),
        on_stderr: Some(on_stderr),
    })
    .await
    .map_err(|e| match friendly_ffmpeg_error(&e.message) {
        Some(tip) => AppError::new(e.code, tip),
        None => e,
    })?;
    Ok(())
}

pub struct AudioConverter;

const AUDIO_IN: &[&str] = &["mp3", "wav", "flac", "aac", "ogg", "m4a", "opus", "wma"];
const AUDIO_OUT: &[&str] = &["mp3", "wav", "flac", "aac", "ogg", "m4a", "opus"];

#[async_trait]
impl Converter for AudioConverter {
    fn id(&self) -> &'static str { "audio" }
    fn name(&self) -> &'static str { "音频转换器 (ffmpeg)" }
    fn supported_inputs(&self) -> &[&str] { AUDIO_IN }
    fn supported_outputs(&self) -> &[&str] { AUDIO_OUT }

    async fn probe(&self, input: &Path) -> Result<FormatInfo, AppError> {
        probe_media(input, scroll_format_core::FormatKind::Audio).await
    }
    async fn preview(&self, input: &Path) -> Result<PreviewData, AppError> {
        let info = self.probe(input).await?;
        Ok(PreviewData { thumb_path: None, extra: serde_json::to_value(&info.media).unwrap_or_default() })
    }
    async fn convert(&self, ctx: ConvertContext) -> Result<ConvertOutput, AppError> {
        ensure_input_readable(&ctx.input).await?;
        let info = self.probe(&ctx.input).await.ok();
        let duration = info.and_then(|i| i.media).and_then(|m| m.duration_secs);
        let target = ctx.options.target_ext.to_lowercase();
        let extra = ctx.options.extra.clone();
        let raw_bitrate = extra
            .get("bitrate")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| match ctx.options.preset.as_deref() {
                Some("fast") | Some("快速") => "128k".to_string(),
                Some("high") | Some("高质量") => "320k".to_string(),
                _ => "192k".to_string(),
            });
        // 「无损」不传 -b:a，由编码器/容器决定
        let bitrate = if raw_bitrate == "lossless" { String::new() } else { raw_bitrate };
        let with_br = |args: &mut Vec<String>, flags: &[&str]| {
            args.push(flags[0].into());
            args.push(flags[1].into());
            if !bitrate.is_empty() && flags.contains(&"-b:a") {
                args.push("-b:a".into());
                args.push(bitrate.clone());
            }
        };
        if let Some(parent) = ctx.output.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut args: Vec<String> = vec!["-i".into(), ctx.input.to_string_lossy().to_string()];
        if let Some(sr) = extra.get("sample_rate").and_then(|v| v.as_str()) {
            if sr != "original" {
                args.extend(["-ar".into(), sr.into()]);
            }
        }
        if let Some(ch) = extra.get("channels").and_then(|v| v.as_u64()) {
            args.extend(["-ac".into(), ch.to_string()]);
        }
        // 用户指定编码器优先
        let codec = extra.get("codec").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let codec_map: &[(&str, &[&str])] = &[
            ("mp3", &["-codec:a", "libmp3lame", "-b:a"]),
            ("wav", &["-codec:a", "pcm_s16le"]),
            ("flac", &["-codec:a", "flac"]),
            ("aac", &["-codec:a", "aac", "-b:a"]),
            ("m4a", &["-codec:a", "aac", "-b:a"]),
            ("ogg", &["-codec:a", "libvorbis", "-b:a"]),
            ("opus", &["-codec:a", "libopus", "-b:a"]),
            ("wma", &["-codec:a", "wmav2", "-b:a"]),
            ("alac", &["-codec:a", "alac"]),
            ("vorbis", &["-codec:a", "libvorbis", "-b:a"]),
            ("mp2", &["-codec:a", "mp2", "-b:a"]),
        ];
        match target.as_str() {
            "mp3" => with_br(&mut args, &["-codec:a", "libmp3lame", "-b:a"]),
            "wav" => args.extend(["-codec:a".into(), "pcm_s16le".into()]),
            "flac" => args.extend(["-codec:a".into(), "flac".into()]),
            "aac" | "m4a" => with_br(&mut args, &["-codec:a", "aac", "-b:a"]),
            "ogg" => with_br(&mut args, &["-codec:a", "libvorbis", "-b:a"]),
            "opus" => with_br(&mut args, &["-codec:a", "libopus", "-b:a"]),
            other => {
                if let Some((_, flags)) = codec_map.iter().find(|(k, _)| *k == other) {
                    with_br(&mut args, flags);
                } else {
                    return Err(AppError::unsupported(format!("不支持的音频输出 {other}")));
                }
            }
        }
        if !codec.is_empty() && matches!(target.as_str(), "mp3" | "wav" | "flac" | "aac" | "m4a" | "ogg" | "opus") {
            // 用自定义编码器覆盖默认（容器兼容性由 ffmpeg 校验）
            let idx = args.iter().position(|a| a == "-codec:a").unwrap_or(0);
            if idx + 1 < args.len() {
                args[idx + 1] = codec.clone();
            }
        }
        // 音量归一化
        match extra.get("loudnorm").and_then(|v| v.as_str()).unwrap_or("off") {
            "ebur128" => args.extend(["-af".into(), "loudnorm=I=-14:TP=-1:LRA=11".into()]),
            "ebur128_16" => args.extend(["-af".into(), "loudnorm=I=-14:TP=-1.5:LRA=11".into()]),
            "dynaudnorm" => args.extend(["-af".into(), "dynaudnorm=f=200:g=15".into()]),
            _ => {}
        }
        // 元数据：不保留时清空
        if !extra.get("keep_meta").and_then(|v| v.as_bool()).unwrap_or(true) {
            args.extend(["-map_metadata".into(), "-1".into()]);
        }
        args.extend(extra_args(&extra));
        args.push(ctx.output.to_string_lossy().to_string());
        run_ffmpeg_with_progress(&ctx, args, duration).await?;
        let bytes = std::fs::metadata(&ctx.output).map(|m| m.len()).unwrap_or(0);
        ctx.report(1.0).await;
        Ok(ConvertOutput::single(ctx.output.to_string_lossy().to_string(), bytes))
    }
}

pub struct VideoConverter;

const VIDEO_IN: &[&str] = &["mp4", "mkv", "mov", "avi", "webm", "gif", "flv", "wmv"];
const VIDEO_OUT: &[&str] = &["mp4", "mkv", "mov", "webm", "avi", "gif"];

#[async_trait]
impl Converter for VideoConverter {
    fn id(&self) -> &'static str { "video" }
    fn name(&self) -> &'static str { "视频转换器 (ffmpeg)" }
    fn supported_inputs(&self) -> &[&str] { VIDEO_IN }
    fn supported_outputs(&self) -> &[&str] { VIDEO_OUT }

    async fn probe(&self, input: &Path) -> Result<FormatInfo, AppError> {
        probe_media(input, scroll_format_core::FormatKind::Video).await
    }
    async fn preview(&self, input: &Path) -> Result<PreviewData, AppError> {
        // 抽取首帧缩略图
        let ff = ffmpeg()?;
        let thumb = std::env::temp_dir().join(format!("scrollfmt_thumb_{}.jpg", uuid::Uuid::new_v4()));
        let _ = ProcessRunner::run(RunSpec {
            program: ff,
            args: vec!["-y".into(), "-ss".into(), "1".into(), "-i".into(), input.to_string_lossy().to_string(), "-frames:v".into(), "1".into(), thumb.to_string_lossy().to_string()],
            timeout: Some(Duration::from_secs(30)),
            cancel: None,
            on_stdout: None,
            on_stderr: None,
        })
        .await;
        let info = self.probe(input).await.ok();
        Ok(PreviewData {
            thumb_path: if thumb.exists() { Some(thumb.to_string_lossy().to_string()) } else { None },
            extra: serde_json::json!({ "media": info.and_then(|i| i.media) }),
        })
    }
    async fn convert(&self, ctx: ConvertContext) -> Result<ConvertOutput, AppError> {
        ensure_input_readable(&ctx.input).await?;
        let info = self.probe(&ctx.input).await.ok();
        let duration = info.and_then(|i| i.media).and_then(|m| m.duration_secs);
        let target = ctx.options.target_ext.to_lowercase();
        let extra = ctx.options.extra.clone();
        let crf = extra
            .get("crf")
            .and_then(|v| v.as_u64())
            .map(|n| n.to_string())
            .unwrap_or_else(|| match ctx.options.preset.as_deref() {
                Some("high") | Some("高质量") => "20".to_string(),
                Some("size") | Some("体积优先") => "30".to_string(),
                Some("fast") | Some("快速") => "26".to_string(),
                _ => "23".to_string(),
            });
        let scale = extra.get("scale").and_then(|v| v.as_str()).unwrap_or("original").to_string();
        let fps = extra.get("fps").and_then(|v| v.as_str()).unwrap_or("original").to_string();
        let venc_preset = extra.get("enc_preset").and_then(|v| v.as_str()).unwrap_or("veryfast").to_string();
        // 视频编码器（用户可覆盖；auto_hw = 有独显时优先硬件编码）
        let mut v_codec = extra.get("v_codec").and_then(|v| v.as_str()).unwrap_or("").to_string();
        if v_codec == "auto_hw" {
            // webm 只有 VP8/VP9/AV1 软件编码器，NVENC 无对应编码，保持软件
            let has_nv = target != "webm" && scroll_format_infra::env::probe_gpu().await.available;
            v_codec = if has_nv { "h264_nvenc".to_string() } else { "libx264".to_string() };
        }
        // ── 硬件解码（与硬件编码配套：解码也交给显卡，显著降低 CPU 占用）──
        // 仅在选硬件编码器时启用；GPU 不支持该输入格式时由下面的降级链自动回落。
        let want_hw_decode = extra.get("hw_decode").and_then(|v| v.as_bool()).unwrap_or(true);
        let hw_codec = v_codec.contains("nvenc") || v_codec.contains("qsv");
        let hwaccel: Option<&'static str> = if want_hw_decode && hw_codec {
            if v_codec.contains("nvenc") {
                Some("cuda")
            } else {
                Some("qsv")
            }
        } else {
            None
        };
        if let Some(parent) = ctx.output.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut args: Vec<String> = Vec::new();
        if let Some(acc) = hwaccel {
            args.extend(["-hwaccel".into(), acc.into()]);
        }
        args.extend(["-i".into(), ctx.input.to_string_lossy().to_string()]);
        // 输出 wav/mp3 = 只提取音轨，不需要视频滤镜，也不需要硬解
        let audio_only = matches!(target.as_str(), "wav" | "mp3");
        if audio_only {
            args.clear();
            args.extend(["-i".into(), ctx.input.to_string_lossy().to_string()]);
        }
        let mut vf: Vec<String> = vec![];
        if scale != "original" {
            vf.push(format!("scale={scale}"));
        }
        if fps != "original" {
            vf.push(format!("fps={fps}"));
        }
        if extra.get("deinterlace").and_then(|v| v.as_bool()).unwrap_or(false) {
            vf.push("yadif".to_string());
        }
        if !vf.is_empty() && !audio_only {
            args.extend(["-vf".into(), vf.join(",")]);
        }
        let a_codec = extra.get("a_codec").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let a_bitrate = extra.get("a_bitrate").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let a_sr = extra.get("a_sample_rate").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let a_ch = extra.get("a_channels").and_then(|v| v.as_u64()).unwrap_or(0);
        let faststart = extra.get("faststart").and_then(|v| v.as_bool()).unwrap_or(false);
        // 硬件编码器基本只接受 yuv420p（8bit），显式指定可避免
        // "Unsupported pix_fmt" / "Incompatible pixel format" 之类失败
        let is_hw = |c: &str| c.contains("nvenc") || c.contains("qsv") || c.contains("amf");

        match target.as_str() {
            "mp4" | "mkv" | "mov" => {
                let vc = if v_codec.is_empty() { "libx264" } else { v_codec.as_str() };
                args.extend(["-c:v".into(), vc.into(), "-crf".into(), crf.clone()]);
                if let Some(p) = map_preset(vc, &venc_preset) {
                    args.extend(["-preset".into(), p]);
                }
                let ac = if a_codec.is_empty() { "aac" } else { a_codec.as_str() };
                if ac == "none" {
                    args.push("-an".into());
                } else {
                    args.extend(["-c:a".into(), ac.into()]);
                    if !a_bitrate.is_empty() && ac != "copy" {
                        args.extend(["-b:a".into(), a_bitrate.clone()]);
                    }
                }
                if target == "mp4" && faststart {
                    args.extend(["-movflags".into(), "+faststart".into()]);
                }
                if is_hw(vc) {
                    args.extend(["-pix_fmt".into(), "yuv420p".into()]);
                }
            }
            "webm" => {
                let vc = if v_codec.is_empty() { "libvpx-vp9" } else { v_codec.as_str() };
                args.extend(["-c:v".into(), vc.into(), "-crf".into(), crf.clone(), "-b:v".into(), "0".into()]);
                if let Some(p) = map_preset(vc, &venc_preset) {
                    args.extend(["-preset".into(), p]);
                }
                let ac = if a_codec.is_empty() { "libopus" } else { a_codec.as_str() };
                if ac == "none" {
                    args.push("-an".into());
                } else {
                    args.extend(["-c:a".into(), ac.into()]);
                    if !a_bitrate.is_empty() {
                        args.extend(["-b:a".into(), a_bitrate.clone()]);
                    }
                }
            }
            "avi" => {
                let vc = if v_codec.is_empty() { "mpeg4" } else { v_codec.as_str() };
                args.extend(["-c:v".into(), vc.into(), "-q:v".into(), "5".into()]);
                let ac = if a_codec.is_empty() { "mp3" } else { a_codec.as_str() };
                if ac != "none" {
                    args.extend(["-c:a".into(), ac.into()]);
                }
            }
            "gif" => {
                if vf.is_empty() {
                    args.extend(["-vf".into(), "fps=15,scale=480:-1:flags=lanczos".into()]);
                }
                args.extend(["-loop".into(), "0".into()]);
            }
            // 从视频里提取音轨（不做视频编码，直接丢流）
            "wav" => {
                args.push("-vn".into());
                args.extend(["-c:a".into(), "pcm_s16le".into()]);
            }
            "mp3" => {
                args.push("-vn".into());
                args.extend(["-c:a".into(), "libmp3lame".into(), "-b:a".into(), "192k".into()]);
            }
            other => return Err(AppError::unsupported(format!("不支持的视频输出 {other}"))),
        }
        // 音频重采样 / 声道
        if !a_sr.is_empty() && a_sr != "original" {
            args.extend(["-ar".into(), a_sr.clone()]);
        }
        if a_ch > 0 {
            args.extend(["-ac".into(), a_ch.to_string()]);
        }
        args.extend(extra_args(&extra));
        args.push(ctx.output.to_string_lossy().to_string());

        // 明确告诉用户这次到底用的编码器与解码方式，避免「以为在用显卡其实在用 CPU」
        let eff = args
            .iter()
            .position(|a| a == "-c:v")
            .and_then(|i| args.get(i + 1))
            .cloned()
            .unwrap_or_else(|| "容器默认".to_string());
        let decode_note = match hwaccel {
            Some("cuda") => "解码：NVDEC 硬解",
            Some("qsv") => "解码：QSV 硬解",
            _ => "解码：CPU",
        };
        if audio_only {
            ctx.note(format!("提取音轨 → .{}（不重新编码视频）", target)).await;
        } else {
            ctx.note(format!(
                "视频编码器：{eff} · {} · {decode_note}",
                if is_hw(&eff) { "GPU 硬件加速" } else { "CPU 软件编码" }
            ))
            .await;
        }

        // ── 降级链：硬件解码+编码 → 仅硬件编码 → 软件编码 ──
        // 任何一步失败都会自动退到下一步（老显卡不支持某 profile、GPU 不支持该输入
        // 编码格式、驱动异常等场景都不会让任务直接失败）。
        let mut variants: Vec<(Vec<String>, String)> = vec![(args.clone(), String::new())];
        if is_hw(&v_codec) {
            if hwaccel.is_some() {
                let no_dec: Vec<String> = args
                    .iter()
                    .enumerate()
                    .filter(|(i, a)| !(*a == "-hwaccel" || (*i > 0 && args[i - 1] == "-hwaccel")))
                    .map(|(_, a)| a.clone())
                    .collect();
                variants.push((no_dec, "GPU 不支持该输入格式的硬件解码，已改为 CPU 解码".into()));
            }
            let fallback = if target == "webm" { "libvpx-vp9" } else { "libx264" };
            let mut sw = args.clone();
            if let Some(i) = sw.iter().position(|a| a == "-c:v") {
                if i + 1 < sw.len() {
                    sw[i + 1] = fallback.into();
                }
            }
            if let Some(p) = map_preset(fallback, &venc_preset) {
                if let Some(i) = sw.iter().position(|a| a == "-preset") {
                    if i + 1 < sw.len() {
                        sw[i + 1] = p;
                    }
                }
            }
            // 软件编码时去掉所有硬件相关开关，避免继续走 GPU 路径
            let sw: Vec<String> = sw
                .iter()
                .enumerate()
                .filter(|(i, a)| !(*a == "-hwaccel" || *a == "-pix_fmt" || (*i > 0 && (args[i - 1] == "-hwaccel" || args[i - 1] == "-pix_fmt"))))
                .map(|(_, a)| a.clone())
                .collect();
            variants.push((sw, format!("硬件编码 {v_codec} 不可用，已回落到 {fallback}")));
        }

        let mut last_err = None;
        for (idx, (vargs, why)) in variants.into_iter().enumerate() {
            if idx > 0 {
                let _ = std::fs::remove_file(&ctx.output);
                ctx.note(why).await;
            }
            match run_ffmpeg_with_progress(&ctx, vargs, duration).await {
                Ok(()) => {
                    last_err = None;
                    break;
                }
                Err(e) => last_err = Some(e),
            }
        }
        if let Some(e) = last_err {
            return Err(e);
        }
        let bytes = std::fs::metadata(&ctx.output).map(|m| m.len()).unwrap_or(0);
        ctx.report(1.0).await;
        Ok(ConvertOutput::single(ctx.output.to_string_lossy().to_string(), bytes))
    }
}
