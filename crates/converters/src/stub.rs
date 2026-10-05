use std::path::Path;

use async_trait::async_trait;
use scroll_format_core::{
    AppError, ConvertContext, ConvertOutput, Converter, FormatInfo, PreviewData,
};

/// 占位转换器：文档 / 电子书 / 数据 / 压缩 / 字体等后续里程碑实现
pub struct StubConverter {
    pub id: &'static str,
    pub name: &'static str,
    pub kind: scroll_format_core::FormatKind,
    pub inputs: &'static [&'static str],
    pub outputs: &'static [&'static str],
}

#[async_trait]
impl Converter for StubConverter {
    fn id(&self) -> &'static str { self.id }
    fn name(&self) -> &'static str { self.name }
    fn supported_inputs(&self) -> &[&str] { self.inputs }
    fn supported_outputs(&self) -> &[&str] { self.outputs }

    async fn probe(&self, input: &Path) -> Result<FormatInfo, AppError> {
        let size = std::fs::metadata(input).map(|m| m.len()).unwrap_or(0);
        Ok(FormatInfo {
            path: input.to_string_lossy().to_string(),
            ext: input.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase(),
            kind: self.kind,
            size,
            display_name: input.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default(),
            media: None,
        })
    }
    async fn preview(&self, _input: &Path) -> Result<PreviewData, AppError> {
        Ok(PreviewData { thumb_path: None, extra: serde_json::json!({ "note": "预览暂未实现" }) })
    }
    async fn convert(&self, _ctx: ConvertContext) -> Result<ConvertOutput, AppError> {
        Err(AppError::unsupported(format!("「{}」转换器尚未实现，敬请期待", self.name)))
    }
}
