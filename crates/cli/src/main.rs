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
        Cmd::Convert { input, output, quality, preset } => {
            let kind = scroll_format_core::detect_kind(&input).unwrap_or(FormatKind::Custom);
            let registry = scroll_format_converters::build_default_registry();
            let converter = registry.for_kind(kind).into_iter().next().ok_or_else(|| anyhow::anyhow!("无可用转换器"))?;
            let ext = output.extension().and_then(|e| e.to_str()).unwrap_or("").to_string();
            let (ptx, mut prx) = tokio::sync::mpsc::channel(8);
            let pt = tokio::spawn(async move { while let Some(p) = prx.recv().await { eprintln!("进度: {:.0}%", p * 100.0); } });
            let (ltx, mut lrx) = tokio::sync::mpsc::channel(64);
            let lt = tokio::spawn(async move { while let Some(l) = lrx.recv().await { eprintln!("[log] {l}"); } });
            let ctx = ConvertContext {
                input,
                output,
                options: scroll_format_core::ConvertOptions { target_ext: ext, quality, preset, extra: serde_json::Value::Null },
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
