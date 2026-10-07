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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConvertOutput {
    /// 主输出路径。多产物时（如 PDF 拆成多张图片）指向产物所在目录
    pub output: String,
    pub bytes: u64,
    /// 其余产物路径（PDF 多页导图时是第 2 页起的文件）
    #[serde(default)]
    pub extra_outputs: Vec<String>,
}

impl ConvertOutput {
    /// 单产物
    pub fn single(output: impl Into<String>, bytes: u64) -> Self {
        Self { output: output.into(), bytes, extra_outputs: Vec::new() }
    }
    /// 多产物：首个作为主输出，其余进 extra_outputs
    pub fn many(mut outputs: Vec<String>) -> Self {
        if outputs.is_empty() {
            return Self::default();
        }
        let bytes = outputs
            .first()
            .and_then(|p| std::fs::metadata(p).ok().map(|m| m.len()))
            .unwrap_or(0);
        let head = outputs.remove(0);
        Self { output: head, bytes, extra_outputs: outputs }
    }
    /// 全部产物（主输出在前）
    pub fn all_outputs(&self) -> Vec<String> {
        let mut v = Vec::with_capacity(self.extra_outputs.len() + 1);
        if !self.output.is_empty() {
            v.push(self.output.clone());
        }
        v.extend(self.extra_outputs.iter().cloned());
        v
    }
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
