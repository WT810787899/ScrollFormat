use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::error::Result;
use crate::format::{Format, FormatInfo};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreviewData {
    pub thumb_path: Option<String>,
    pub extra: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct ConvertContext {
    pub input: PathBuf,
    pub output: PathBuf,
    pub options: crate::task::ConvertOptions,
    pub cancel: CancellationToken,
    pub progress: mpsc::Sender<f32>,
    pub log: mpsc::Sender<String>,
}

impl ConvertContext {
    pub async fn report(&self, p: f32) {
        let _ = self.progress.send(p.clamp(0.0, 1.0)).await;
    }
    pub async fn note(&self, msg: impl Into<String>) {
        let _ = self.log.send(msg.into()).await;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConvertOutput {
    pub output: String,
    pub bytes: u64,
}

#[async_trait]
pub trait Converter: Send + Sync {
    fn id(&self) -> &'static str;
    fn name(&self) -> &'static str;
    fn supported_inputs(&self) -> &[&str];
    fn supported_outputs(&self) -> &[&str];
    fn output_formats(&self) -> Vec<Format> {
        self.supported_outputs()
            .iter()
            .map(|ext| Format { ext: ext.to_string(), kind: crate::format::detect_kind(Path::new(&format!("x.{ext}"))).unwrap_or(crate::format::FormatKind::Custom) })
            .collect()
    }
    async fn probe(&self, input: &Path) -> Result<FormatInfo>;
    async fn preview(&self, input: &Path) -> Result<PreviewData>;
    async fn convert(&self, ctx: ConvertContext) -> Result<ConvertOutput>;
}
