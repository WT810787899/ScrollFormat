use std::path::PathBuf;

use clap::{Parser, Subcommand};
use scroll_format_core::{ConvertContext, FormatKind};

#[derive(Parser)]
#[command(name = "scrollfmt", about = "格式卷轴 ScrollFormat CLI", version)]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// 转换单个文件
    Convert {
        input: PathBuf,
        #[arg(short = 'o', long = "out")]
        output: PathBuf,
        #[arg(long)]
        quality: Option<u8>,
        #[arg(long)]
        preset: Option<String>,
        /// 视频编码器，如 h264_nvenc / hevc_nvenc / libx264；auto_hw=有独显用 NVENC
        #[arg(long)]
        v_codec: Option<String>,
        /// 编码速度（软件编码器用 x264 风格名，硬件编码器会自动换算）
        #[arg(long)]
        enc_preset: Option<String>,
        /// PDF 转图片的分辨率（72=屏幕，150=默认，300=印刷）
        #[arg(long)]
        dpi: Option<u32>,
        /// 导出页码：all=全部页 / first=首页 / 自定义如 1-3,5,8-
        #[arg(long)]
        pages: Option<String>,
    },
    /// 环境检测
    Env,
    /// 列出任务
    TaskList,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Cmd::Convert { input, output, quality, preset, v_codec, enc_preset, dpi, pages } => {
            let kind = scroll_format_core::detect_kind(&input).unwrap_or(FormatKind::Custom);
            let registry = scroll_format_converters::build_default_registry();
            let converter = registry.for_kind(kind).into_iter().next().ok_or_else(|| anyhow::anyhow!("无可用转换器"))?;
            let ext = output.extension().and_then(|e| e.to_str()).unwrap_or("").to_string();
            let (ptx, mut prx) = tokio::sync::mpsc::channel(8);
            let pt = tokio::spawn(async move { while let Some(p) = prx.recv().await { eprintln!("进度: {:.0}%", p * 100.0); } });
            let (ltx, mut lrx) = tokio::sync::mpsc::channel(64);
            let lt = tokio::spawn(async move { while let Some(l) = lrx.recv().await { eprintln!("[log] {l}"); } });
            let mut extra = serde_json::Map::new();
            if let Some(v) = v_codec {
                extra.insert("v_codec".into(), serde_json::Value::String(v));
            }
            if let Some(p) = enc_preset {
                extra.insert("enc_preset".into(), serde_json::Value::String(p));
            }
            if let Some(d) = dpi {
                extra.insert("dpi".into(), serde_json::Value::from(d));
            }
            if let Some(pg) = pages {
                extra.insert("pages".into(), serde_json::Value::String(pg));
            }
            let ctx = ConvertContext {
                input,
                output,
                options: scroll_format_core::ConvertOptions {
                    target_ext: ext,
                    quality,
                    preset,
                    extra: serde_json::Value::Object(extra),
                },
                cancel: tokio_util::sync::CancellationToken::new(),
                progress: ptx,
                log: ltx,
            };
            converter.convert(ctx).await?;
            let _ = pt.await;
            let _ = lt.await;
            println!("完成");
        }
        Cmd::Env => {
            let report = scroll_format_infra::probe_env().await;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Cmd::TaskList => {
            let data_dir = scroll_format_infra::paths::app_data_dir();
            let db = scroll_format_infra::Db::open(&data_dir.join("scrollformat.db"))?;
            for t in db.list_tasks()? {
                println!("{}\t{}\t{}\t{:.0}%", t.id, t.name, t.status.as_str(), t.progress * 100.0);
            }
        }
    }
    Ok(())
}
