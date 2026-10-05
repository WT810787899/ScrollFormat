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
            "-v".into(), "quiet".into(),
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
    .await?;
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
        Ok(ConvertOutput { output: ctx.output.to_string_lossy().to_string(), bytes })
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
        if let Some(parent) = ctx.output.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut args: Vec<String> = vec!["-i".into(), ctx.input.to_string_lossy().to_string()];
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
        if !vf.is_empty() {
            args.extend(["-vf".into(), vf.join(",")]);
        }
        // 视频编码器（用户可覆盖）
        let v_codec = extra.get("v_codec").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let a_codec = extra.get("a_codec").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let a_bitrate = extra.get("a_bitrate").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let a_sr = extra.get("a_sample_rate").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let a_ch = extra.get("a_channels").and_then(|v| v.as_u64()).unwrap_or(0);
        let faststart = extra.get("faststart").and_then(|v| v.as_bool()).unwrap_or(false);

        match target.as_str() {
            "mp4" | "mkv" | "mov" => {
                let vc = if v_codec.is_empty() { "libx264" } else { v_codec.as_str() };
                args.extend(["-c:v".into(), vc.into(), "-crf".into(), crf.clone(), "-preset".into(), venc_preset.clone()]);
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
            }
            "webm" => {
                let vc = if v_codec.is_empty() { "libvpx-vp9" } else { v_codec.as_str() };
                args.extend(["-c:v".into(), vc.into(), "-crf".into(), crf.clone(), "-b:v".into(), "0".into()]);
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
        run_ffmpeg_with_progress(&ctx, args, duration).await?;
        let bytes = std::fs::metadata(&ctx.output).map(|m| m.len()).unwrap_or(0);
        ctx.report(1.0).await;
        Ok(ConvertOutput { output: ctx.output.to_string_lossy().to_string(), bytes })
    }
}
