use std::path::Path;

use async_trait::async_trait;
use image::ImageEncoder;
use scroll_format_core::{
    AppError, ConvertContext, ConvertOutput, Converter, FormatInfo, PreviewData,
};

pub struct ImageConverter;

/// 解析 #rrggbb / #rgb 颜色
fn parse_hex(hex: &str) -> (u8, u8, u8) {
    let s = hex.trim().trim_start_matches('#');
    let parse = |a: &str| u8::from_str_radix(a, 16).unwrap_or(255);
    match s.len() {
        6 => (parse(&s[0..2]), parse(&s[2..4]), parse(&s[4..6])),
        3 => {
            let r = parse(&s[0..1]);
            let g = parse(&s[1..2]);
            let b = parse(&s[2..3]);
            (r * 17, g * 17, b * 17)
        }
        _ => (255, 255, 255),
    }
}

const INPUTS: &[&str] = &["jpg", "jpeg", "png", "webp", "gif", "bmp", "tiff", "tif", "ico"];
const OUTPUTS: &[&str] = &["jpg", "jpeg", "png", "webp", "gif", "bmp", "tiff", "ico"];

#[async_trait]
impl Converter for ImageConverter {
    fn id(&self) -> &'static str { "image" }
    fn name(&self) -> &'static str { "图片转换器 (image-rs)" }
    fn supported_inputs(&self) -> &[&str] { INPUTS }
    fn supported_outputs(&self) -> &[&str] { OUTPUTS }

    async fn probe(&self, input: &Path) -> Result<FormatInfo, AppError> {
        let size = std::fs::metadata(input).map(|m| m.len()).unwrap_or(0);
        let (w, h) = image::image_dimensions(input).unwrap_or((0, 0));
        Ok(FormatInfo {
            path: input.to_string_lossy().to_string(),
            ext: input.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase(),
            kind: scroll_format_core::FormatKind::Image,
            size,
            display_name: input.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default(),
            media: Some(scroll_format_core::format::MediaInfo {
                width: Some(w),
                height: Some(h),
                ..Default::default()
            }),
        })
    }

    async fn preview(&self, input: &Path) -> Result<PreviewData, AppError> {
        let info = self.probe(input).await?;
        Ok(PreviewData {
            thumb_path: Some(input.to_string_lossy().to_string()),
            extra: serde_json::json!({ "width": info.media.as_ref().and_then(|m| m.width), "height": info.media.as_ref().and_then(|m| m.height), "size": info.size }),
        })
    }

    async fn convert(&self, ctx: ConvertContext) -> Result<ConvertOutput, AppError> {
        let input = ctx.input.clone();
        let output = ctx.output.clone();
        let target = ctx.options.target_ext.to_lowercase();
        let quality = ctx.options.quality.unwrap_or(85);
        let extra = ctx.options.extra.clone();
        ctx.report(0.05).await;
        if ctx.cancel.is_cancelled() { return Err(AppError::cancelled()); }

        let result = tokio::task::spawn_blocking(move || -> Result<u64, AppError> {
            let img = image::open(&input).map_err(|e| AppError::conv(e.to_string()))?;
            let format = match target.as_str() {
                "jpg" | "jpeg" => image::ImageFormat::Jpeg,
                "png" => image::ImageFormat::Png,
                "webp" => image::ImageFormat::WebP,
                "gif" => image::ImageFormat::Gif,
                "bmp" => image::ImageFormat::Bmp,
                "tiff" | "tif" => image::ImageFormat::Tiff,
                "ico" => image::ImageFormat::Ico,
                other => return Err(AppError::unsupported(format!("不支持的输出格式 {other}"))),
            };
            if let Some(parent) = output.parent() {
                std::fs::create_dir_all(parent).map_err(|e| AppError::io(e.to_string()))?;
            }
            // 插值算法
            let filter = match extra.get("filter").and_then(|v| v.as_str()).unwrap_or("lanczos3") {
                "catmullrom" => image::imageops::FilterType::CatmullRom,
                "triangle" => image::imageops::FilterType::Triangle,
                "nearest" => image::imageops::FilterType::Nearest,
                _ => image::imageops::FilterType::Lanczos3,
            };
            // 按 extra.max_side 缩放，或按 extra.percent 等比缩放
            let mut img = if let Some(pct) = extra.get("percent").and_then(|v| v.as_u64()) {
                if pct > 0 && pct != 100 {
                    let w = ((img.width() as f64) * pct as f64 / 100.0).round().max(1.0) as u32;
                    let h = ((img.height() as f64) * pct as f64 / 100.0).round().max(1.0) as u32;
                    img.resize(w, h, filter)
                } else {
                    img
                }
            } else {
                match extra.get("max_side").and_then(|v| v.as_u64()) {
                    Some(max) if max > 0 && (img.width() as u64 > max || img.height() as u64 > max) => {
                        img.resize(max as u32, max as u32, filter)
                    }
                    _ => img,
                }
            };
            // 透明区域填充背景色（输出格式不支持透明时）
            if extra.get("flatten").and_then(|v| v.as_bool()).unwrap_or(false)
                && matches!(format, image::ImageFormat::Jpeg | image::ImageFormat::Bmp)
            {
                let hex = extra.get("bg").and_then(|v| v.as_str()).unwrap_or("#ffffff");
                let rgb = parse_hex(hex);
                let (w, h) = (img.width(), img.height());
                let mut canvas = image::RgbImage::from_pixel(w, h, image::Rgb([rgb.0, rgb.1, rgb.2]));
                let rgb_img = img.to_rgb8();
                for (x, y, p) in rgb_img.enumerate_pixels() {
                    canvas.put_pixel(x, y, *p);
                }
                img = image::DynamicImage::ImageRgb8(canvas);
            }
            let mut out = std::fs::File::create(&output).map_err(|e| AppError::io(e.to_string()))?;
            match format {
                image::ImageFormat::Jpeg => {
                    let mut enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, quality);
                    enc.encode_image(&img).map_err(|e| AppError::conv(e.to_string()))?;
                }
                image::ImageFormat::Ico => {
                    // ICO 仅支持 1..=256，先按需缩放；write_to 不支持 ICO，需专用编码器
                    let rgba = if img.width() > 256 || img.height() > 256 {
                        image::DynamicImage::ImageRgba8(img.to_rgba8()).thumbnail(256, 256).to_rgba8()
                    } else {
                        img.to_rgba8()
                    };
                    let (w, h) = (rgba.width(), rgba.height());
                    let enc = image::codecs::ico::IcoEncoder::new(&mut out);
                    enc.write_image(&rgba, w, h, image::ExtendedColorType::Rgba8)
                        .map_err(|e| AppError::conv(e.to_string()))?;
                }
                _ => {
                    img.write_to(&mut std::io::BufWriter::new(&mut out), format)
                        .map_err(|e| AppError::conv(e.to_string()))?;
                }
            }
            Ok(std::fs::metadata(&output).map(|m| m.len()).unwrap_or(0))
        })
        .await
        .map_err(|e| AppError::conv(e.to_string()))??;

        ctx.report(1.0).await;
        ctx.note(format!("输出: {}", ctx.output.display())).await;
        Ok(ConvertOutput::single(ctx.output.to_string_lossy().to_string(), result))
    }
}
