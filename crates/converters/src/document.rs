//! 文档转换器
//!
//! 路由策略（按「是否有外部工具」自动降级，能用多少用多少）：
//!   1. txt / md / html 三族互转  → 内置纯 Rust 实现，零依赖
//!   2. → pdf                     → LibreOffice 优先；纯文本走内置 PDF 生成器（仅西文）
//!   3. → png / jpg               → pdftoppm(poppler) → ImageMagick → LibreOffice（取第一页）
//!   4. docx/odt/rtf ↔ md/html/txt → pandoc 优先，其次 LibreOffice
//!   5. pdf → docx/md/html/txt    → LibreOffice（PDF 以 Draw 方式导入，版式可能变化）
//!
//! 外部工具缺失时返回明确的中文错误，告诉用户装什么，而不是丢一个 ffmpeg 风格的英文报错。

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use scroll_format_core::{AppError, ConvertContext, ConvertOutput, Converter, FormatInfo, FormatKind, PreviewData};
use unicode_normalization::UnicodeNormalization;
use scroll_format_infra::{find_tool, ProcessRunner, RunSpec};

use crate::doc_builtin;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Src {
    Txt,
    Md,
    Html,
    Rtf,
    Docx,
    Odt,
    Pdf,
    Unknown,
}

fn detect_src(input: &Path) -> Src {
    let e = input
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match e.as_str() {
        "txt" | "text" | "log" => Src::Txt,
        "md" | "markdown" | "mdown" => Src::Md,
        "html" | "htm" | "xhtml" => Src::Html,
        "rtf" => Src::Rtf,
        "docx" | "doc" => Src::Docx,
        "odt" => Src::Odt,
        "pdf" => Src::Pdf,
        _ => Src::Unknown,
    }
}

/// 读文本并去掉 UTF-8 BOM（PowerShell/记事本保存的 txt、md、html 常带 BOM，
/// 不去掉会导致首行的 `#` 标题、`<html>` 标签匹配不上）
fn read_text(path: &Path) -> Result<String, AppError> {
    let raw = std::fs::read(path).map_err(|e| AppError::io(format!("读取失败：{e}")))?;
    let s = String::from_utf8_lossy(&raw).to_string();
    Ok(s.strip_prefix('\u{feff}').unwrap_or(&s).to_string())
}

const DOC_IN: &[&str] = &["pdf", "docx", "doc", "odt", "rtf", "txt", "md", "markdown", "html", "htm", "xhtml"];
const DOC_OUT: &[&str] = &["pdf", "docx", "odt", "rtf", "txt", "md", "html", "htm", "png", "jpg", "jpeg"];

fn nfkc(s: &str) -> String {
    s.nfkc().collect()
}

fn need(tool: &str, hint: &str) -> AppError {
    AppError::tool_missing(format!("未找到 {tool}：{hint}"))
}

// ═══════════════════════════════════════════════════════════
// 内置：纯文本族互转（零依赖）
// ═══════════════════════════════════════════════════════════

/// HTML → 纯文本：块级标签换行、行内标签直接去掉、解实体、压缩空行
fn html_to_text(html: &str) -> String {
    const BLOCK: [&str; 24] = [
        "p", "div", "br", "li", "tr", "td", "h1", "h2", "h3", "h4", "h5", "h6", "section", "article",
        "header", "footer", "ul", "ol", "pre", "blockquote", "table", "hr", "figure", "figcaption",
    ];
    let mut out = String::with_capacity(html.len());
    let mut skip_tag = String::new();
    let bytes = html.as_bytes();
    let mut i = 0usize;
    while i < html.len() {
        if bytes[i] == b'<' {
            let end = html[i..].find('>').map(|p| i + p).unwrap_or_else(|| html.len().saturating_sub(1));
            let inner_end = end.max(i + 1);
            let raw = &html[i + 1..inner_end];
            let is_close = raw.starts_with('/');
            let name: String = raw
                .trim_start_matches('/')
                .trim_start()
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric())
                .collect::<String>()
                .to_ascii_lowercase();
            if name == "script" || name == "style" || name == "head" {
                if is_close {
                    skip_tag.clear();
                } else {
                    skip_tag = name.clone();
                }
            }
            if skip_tag.is_empty() && !is_close && BLOCK.contains(&name.as_str()) {
                out.push('\n');
            }
            i = inner_end + 1;
            continue;
        }
        if !skip_tag.is_empty() {
            i += 1;
            continue;
        }
        let ch = html[i..].chars().next().unwrap_or('\n');
        out.push(ch);
        i += ch.len_utf8();
    }
    // 常见实体（&amp; 放最后，避免把 &amp;lt; 误解码）
    let text = out
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&nbsp;", " ")
        .replace("&mdash;", "—")
        .replace("&ndash;", "–")
        .replace("&hellip;", "…")
        .replace("&amp;", "&");
    let text = nfkc(&text);
    // 压缩连续空行 + 行尾空白
    let mut lines: Vec<String> = Vec::new();
    for l in text.lines() {
        let l = l.trim_end();
        if l.is_empty() {
            if lines.last().map(|p: &String| p.is_empty()).unwrap_or(false) {
                continue;
            }
            lines.push(String::new());
        } else {
            lines.push(l.to_string());
        }
    }
    while lines.last().map(|l| l.is_empty()).unwrap_or(false) {
        lines.pop();
    }
    lines.join("\n")
}

/// Markdown → 纯文本：去掉标记符号，保留结构
fn md_to_text(md: &str) -> String {
    let mut out = String::with_capacity(md.len());
    for line in md.lines() {
        let mut l = line.to_string();
        // 代码块围栏
        if l.trim_start().starts_with("```") {
            continue;
        }
        // 标题 / 引用 / 列表符号
        let t = l.trim_start();
        if t.starts_with('#') {
            l = t.trim_start_matches('#').trim_start().to_string();
        } else if t.starts_with("> ") {
            l = t[2..].to_string();
        } else if t.starts_with("- ") || t.starts_with("* ") || t.starts_with("+ ") {
            l = t[2..].to_string();
        }
        // 行内标记：**粗体** *斜体* `代码` [文字](链接) ![图](链接)
        l = l.replace("**", "").replace("__", "");
        l = l.replace('*', "").replace('`', "");
        // 链接/图片整段去掉（含前面的方括号）
        loop {
            let Some(open) = l.find('[') else { break };
            let Some(close_rel) = l[open..].find(')') else { break };
            let end = open + close_rel + 1;
            // 必须是 ]( 形式，否则保留（如 [abc] 纯方括号）
            if l[open..end].contains("](") {
                l.replace_range(open..end, "");
            } else {
                break;
            }
        }
        // 分隔线
        if l.trim().len() >= 3 && l.trim().chars().all(|c| c == '-' || c == '*' || c == '_') {
            l.clear();
        }
        out.push_str(l.trim_end());
        out.push('\n');
    }
    out.lines().map(str::trim_end).filter(|l| !l.is_empty()).collect::<Vec<_>>().join("\n")
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

fn text_to_html(text: &str) -> String {
    format!(
        "<!doctype html>\n<html lang=\"zh-CN\">\n<head>\n<meta charset=\"utf-8\" />\n<title>{}</title>\n</head>\n<body>\n<pre style=\"white-space:pre-wrap;font-family:Consolas,monospace\">{}</pre>\n</body>\n</html>\n",
        "Document",
        escape_html(text)
    )
}

fn text_to_md(text: &str) -> String {
    // 极简：整段作为正文，保留换行
    text.to_string()
}

// 外部工具调用
// ═══════════════════════════════════════════════════════════

async fn run_tool(program: PathBuf, args: Vec<String>, ctx: &ConvertContext, label: &str) -> Result<String, AppError> {
    let log = ctx.log.clone();
    let on_stderr = Box::new(move |line: &str| {
        let _ = log.try_send(line.to_string());
    });
    let out = ProcessRunner::run(RunSpec {
        program,
        args,
        timeout: Some(std::time::Duration::from_secs(180)),
        cancel: Some(ctx.cancel.clone()),
        on_stdout: None,
        on_stderr: Some(on_stderr),
    })
    .await
    .map_err(|e| {
        // ffmpeg 风格的英文报错在这里没有意义，换成文档场景能看懂的提示
        let msg = e.message.to_ascii_lowercase();
        if msg.contains("no such file") || msg.contains("not found") {
            need(label, "请确认已安装并加入 PATH")
        } else {
            AppError::conv(format!("{label} 转换失败：{}", e.message))
        }
    })?;
    ctx.note(format!("已调用 {label} 完成转换")).await;
    Ok(out.stdout)
}

/// LibreOffice headless 转换（输出到临时目录后再移动到目标名）
async fn via_libreoffice(ctx: &ConvertContext, convert_to: &str) -> Result<(), AppError> {
    let soffice = find_tool("soffice").or_else(|| find_tool("libreoffice"))
        .ok_or_else(|| need("LibreOffice", "请安装 LibreOffice（soffice）以进行 Office/PDF 转换"))?;
    let tmp = ctx.output.with_extension(format!("lo_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&tmp)?;
    // 每次用独立 user profile，避免并行任务互相抢锁
    let profile = std::env::temp_dir().join(format!("lo_profile_{}", uuid::Uuid::new_v4()));
    let profile_uri = format!("file:///{}", profile.to_string_lossy().replace('\\', "/"));

    let stem = ctx.input.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "out".into());
    let produced = tmp.join(format!("{stem}.{convert_to}"));
    let res = run_tool(
        soffice,
        vec![
            format!("-env:UserInstallation={profile_uri}"),
            "--headless".into(),
            "--norestore".into(),
            "--invisible".into(),
            "--convert-to".into(),
            convert_to.to_string(),
            "--outdir".into(),
            tmp.to_string_lossy().to_string(),
            ctx.input.to_string_lossy().to_string(),
        ],
        ctx,
        "LibreOffice",
    )
    .await;
    let _ = std::fs::remove_dir_all(&profile);
    res?;
    if !produced.exists() {
        let _ = std::fs::remove_dir_all(&tmp);
        return Err(AppError::conv(format!(
            "LibreOffice 未生成 .{convert_to}（该格式可能不被支持，建议先转成 PDF 再处理）"
        )));
    }
    if let Some(parent) = ctx.output.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let _ = std::fs::remove_file(&ctx.output);
    std::fs::rename(&produced, &ctx.output).or_else(|_| {
        std::fs::copy(&produced, &ctx.output).map(|_| ())
    })?;
    let _ = std::fs::remove_dir_all(&tmp);
    Ok(())
}

async fn via_pandoc(ctx: &ConvertContext, from: &str, to: &str) -> Result<(), AppError> {
    let pandoc = find_tool("pandoc").ok_or_else(|| need("pandoc", "请安装 pandoc 以在 docx/rtf/markdown/html 之间转换"))?;
    run_tool(
        pandoc,
        vec![
            "-f".into(),
            from.into(),
            "-t".into(),
            to.into(),
            "-o".into(),
            ctx.output.to_string_lossy().to_string(),
            ctx.input.to_string_lossy().to_string(),
        ],
        ctx,
        "pandoc",
    )
    .await
    .map(|_| ())
}

/// 解析页码范围参数
///
/// - `None` / 空 / `"all"` → 全部页
/// - `"first"` / `"1"` → 只有首页
/// - `"1-3,5,8-"` → 显式页码（1 起；`8-` 表示 8 到末页）
fn parse_page_range(spec: Option<&str>, total: usize) -> Vec<usize> {
    let spec = spec.map(|s| s.trim()).unwrap_or("");
    if spec.is_empty() || spec.eq_ignore_ascii_case("all") || spec == "全部" {
        return (0..total).collect();
    }
    if spec.eq_ignore_ascii_case("first") || spec == "首页" || spec == "1" {
        return if total > 0 { vec![0] } else { Vec::new() };
    }
    let mut out = Vec::new();
    for part in spec.split([',', '，', ';', '；']) {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if let Some((a, b)) = part.split_once('-') {
            let a = a.trim().parse::<usize>().unwrap_or(1);
            let b = if b.trim().is_empty() { total } else { b.trim().parse::<usize>().unwrap_or(total) };
            if a == 0 || b < a {
                continue;
            }
            for p in a..=b.min(total) {
                if !out.contains(&(p - 1)) {
                    out.push(p - 1);
                }
            }
        } else if let Ok(p) = part.parse::<usize>() {
            // 单页越界时夹到末页（而不是退回全部页）—— 用户写 "99" 通常是想看最后一页
            let p = p.clamp(1, total.max(1));
            if !out.contains(&(p - 1)) {
                out.push(p - 1);
            }
        }
    }
    out.sort_unstable();
    if out.is_empty() { (0..total).collect() } else { out }
}

/// 产物文件名：多页时用 `名字-01.png` 这种带序号的形式
fn page_file_name(stem: &str, idx: usize, total: usize, ext: &str) -> String {
    if total <= 1 {
        format!("{stem}.{ext}")
    } else {
        format!("{stem}-{:02}.{ext}", idx + 1)
    }
}

/// PDF → 图片，支持多页
///
/// 返回产出的全部文件路径。单页时就是 ctx.output 本身；
/// 多页时统一放进 `ctx.output` 同名的子目录里（名字-01.png、名字-02.png…）。
async fn pdf_to_images(
    ctx: &ConvertContext,
    target: &str,
    dpi: u32,
    pages_spec: Option<&str>,
) -> Result<Vec<PathBuf>, AppError> {
    let jpeg = target == "jpg" || target == "jpeg";
    let ext = if jpeg { "jpg" } else { "png" };
    let stem = ctx
        .output
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "page".into());

    // 内置 PDFium（编译期内嵌，零外部依赖）优先
    let total = doc_builtin::pdf_page_count(&ctx.input).unwrap_or(1);
    let page_ids = parse_page_range(pages_spec, total);
    if !page_ids.is_empty() {
        // 多页 → 写进子目录；单页 → 保持原路径（不改变已有行为）
        let multi = page_ids.len() > 1;
        let dir = if multi {
            let d = ctx
                .output
                .parent()
                .map(|p| p.join(&stem))
                .unwrap_or_else(|| PathBuf::from(&stem));
            std::fs::create_dir_all(&d)?;
            d
        } else {
            ctx.output.parent().unwrap_or(Path::new(".")).to_path_buf()
        };
        let mut written: Vec<PathBuf> = Vec::new();
        let n = page_ids.len();
        for (i, &pid) in page_ids.iter().enumerate() {
            let out = dir.join(page_file_name(&stem, i, n, ext));
            match doc_builtin::render_pdf_page(&ctx.input, pid, dpi, jpeg, 92) {
                Ok((data, w, h)) => {
                    std::fs::write(&out, &data)?;
                    written.push(out);
                    if n > 1 {
                        // 进度跟着页数走
                        ctx.report(0.3 + 0.65 * (i + 1) as f32 / n as f32).await;
                    }
                    if i == 0 || i + 1 == n {
                        ctx.note(format!("第 {} 页：{w}×{h} px / {dpi} DPI", pid + 1)).await;
                    }
                }
                Err(e) => {
                    if written.is_empty() {
                        return Err(e);
                    }
                    ctx.note(format!("第 {} 页渲染失败，已跳过：{e}", pid + 1)).await;
                }
            }
        }
        if written.is_empty() {
            return Err(AppError::conv("PDF 渲染没有产出任何页面"));
        }
        if n > 1 {
            ctx.note(format!(
                "已用内置 PDF 引擎导出 {}/{} 页 → {}",
                written.len(),
                n,
                dir.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default()
            ))
            .await;
        } else {
            ctx.note("已用内置 PDF 引擎渲染（无需外部工具）").await;
        }
        return Ok(written);
    }

    // ── 以下是外部工具兜底，只出首页 ──────────────────────────
    if let Some(pdftoppm) = find_tool("pdftoppm") {
        let tmp = ctx.output.with_extension("ppm_tmp");
        let prefix = tmp.to_string_lossy().to_string();
        run_tool(
            pdftoppm,
            vec![
                "-png".into(),
                "-r".into(),
                dpi.to_string(),
                "-f".into(),
                "1".into(),
                "-l".into(),
                "1".into(),
                "-singlefile".into(),
                ctx.input.to_string_lossy().to_string(),
                prefix.clone(),
            ],
            ctx,
            "pdftoppm",
        )
        .await?;
        let produced = PathBuf::from(format!("{prefix}.png"));
        let final_out = if target == "jpg" || target == "jpeg" {
            ctx.output.with_extension("png")
        } else {
            ctx.output.clone()
        };
        if !produced.exists() {
            return Err(AppError::conv("pdftoppm 未生成图片"));
        }
        let _ = std::fs::remove_file(&ctx.output);
        std::fs::rename(&produced, &final_out)?;
        let _ = std::fs::remove_file(&tmp);
        return Ok(vec![final_out]);
    }
    if let Some(magick) = find_tool("magick") {
        run_tool(
            magick,
            vec![
                "-density".into(),
                dpi.to_string(),
                format!("{}[0]", ctx.input.to_string_lossy()),
                // ImageMagick 按输出扩展名决定格式
                ctx.output.to_string_lossy().to_string(),
            ],
            ctx,
            "ImageMagick",
        )
        .await?;
        return Ok(vec![ctx.output.clone()]);
    }
    // 兜底：LibreOffice 能把 PDF 导入 Draw 并导出首页 png
    if find_tool("soffice").is_some() || find_tool("libreoffice").is_some() {
        via_libreoffice_to(ctx, "png", &ctx.output.clone()).await?;
        return Ok(vec![ctx.output.clone()]);
    }
    Err(need(
        "PDF 转图片引擎",
        "内置渲染引擎加载失败，请安装 poppler(pdftoppm)、ImageMagick 或 LibreOffice 之一作为备用",
    ))
}

async fn via_libreoffice_to(ctx: &ConvertContext, convert_to: &str, out_path: &Path) -> Result<(), AppError> {
    let soffice = find_tool("soffice").or_else(|| find_tool("libreoffice")).ok_or_else(|| {
        need("LibreOffice", "请安装 LibreOffice（soffice）")
    })?;
    let tmp = out_path.with_extension(format!("lo_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&tmp)?;
    let profile = std::env::temp_dir().join(format!("lo_profile_{}", uuid::Uuid::new_v4()));
    let profile_uri = format!("file:///{}", profile.to_string_lossy().replace('\\', "/"));
    let stem = ctx.input.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "out".into());
    let produced = tmp.join(format!("{stem}.{convert_to}"));
    let res = run_tool(
        soffice,
        vec![
            format!("-env:UserInstallation={profile_uri}"),
            "--headless".into(),
            "--norestore".into(),
            "--convert-to".into(),
            convert_to.to_string(),
            "--outdir".into(),
            tmp.to_string_lossy().to_string(),
            ctx.input.to_string_lossy().to_string(),
        ],
        ctx,
        "LibreOffice",
    )
    .await;
    let _ = std::fs::remove_dir_all(&profile);
    res?;
    if !produced.exists() {
        let _ = std::fs::remove_dir_all(&tmp);
        return Err(AppError::conv(format!("LibreOffice 未生成 .{convert_to}")));
    }
    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let _ = std::fs::remove_file(out_path);
    std::fs::rename(&produced, out_path).or_else(|_| std::fs::copy(&produced, out_path).map(|_| ()))?;
    let _ = std::fs::remove_dir_all(&tmp);
    Ok(())
}

pub struct DocumentConverter;

#[async_trait]
impl Converter for DocumentConverter {
    fn id(&self) -> &'static str {
        "document"
    }
    fn name(&self) -> &'static str {
        "文档转换器 (pandoc / LibreOffice / 内置)"
    }
    fn supported_inputs(&self) -> &[&str] {
        DOC_IN
    }
    fn supported_outputs(&self) -> &[&str] {
        DOC_OUT
    }

    async fn probe(&self, input: &Path) -> Result<FormatInfo, AppError> {
        let md = std::fs::metadata(input).map_err(|e| AppError::io(format!("无法读取文件：{e}")))?;
        Ok(FormatInfo {
            path: input.to_string_lossy().to_string(),
            ext: input
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_ascii_lowercase(),
            kind: FormatKind::Document,
            size: md.len(),
            display_name: input
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default(),
            media: None,
        })
    }

    async fn preview(&self, input: &Path) -> Result<PreviewData, AppError> {
        // 文本类直接给前几行做预览
        let mut text = String::new();
        if matches!(detect_src(input), Src::Txt | Src::Md | Src::Html) {
            if let Ok(s) = read_text(input) {
                let t = match detect_src(input) {
                    Src::Html => html_to_text(&s),
                    Src::Md => md_to_text(&s),
                    _ => s,
                };
                text = t.chars().take(1200).collect();
            }
        }
        Ok(PreviewData { thumb_path: None, extra: serde_json::json!({ "text": text }) })
    }

    async fn convert(&self, ctx: ConvertContext) -> Result<ConvertOutput, AppError> {
        let src = detect_src(&ctx.input);
        let target = ctx.options.target_ext.to_ascii_lowercase();
        let extra = ctx.options.extra.clone();
        let dpi = extra.get("dpi").and_then(|v| v.as_u64()).unwrap_or(150) as u32;
        let keep_bg = extra.get("keep_bg").and_then(|v| v.as_bool()).unwrap_or(true);
        if let Some(parent) = ctx.output.parent() {
            std::fs::create_dir_all(parent)?;
        }
        ctx.report(0.1).await;

        if src == Src::Unknown {
            return Err(AppError::unsupported(format!(
                "暂不支持的文档格式：{}",
                ctx.input.extension().and_then(|s| s.to_str()).unwrap_or("?")
            )));
        }

        // ── 1) 文本族互转：内置实现 ────────────────────────────
        let is_text_src = matches!(src, Src::Txt | Src::Md | Src::Html);
        let text_target = match target.as_str() {
            "txt" => Some(Src::Txt),
            "md" => Some(Src::Md),
            "html" | "htm" => Some(Src::Html),
            _ => None,
        };

        // 统一的「抽取正文」：文本族 + Office 三件套 + PDF 全部走内置实现
        let extract_builtin = |s: Src| -> Result<String, AppError> {
            match s {
                Src::Txt => read_text(&ctx.input),
                Src::Md => Ok(md_to_text(&read_text(&ctx.input)?)),
                Src::Html => Ok(html_to_text(&read_text(&ctx.input)?)),
                Src::Docx => doc_builtin::docx_to_text(&ctx.input),
                Src::Odt => doc_builtin::odt_to_text(&ctx.input),
                Src::Rtf => doc_builtin::rtf_to_text(&ctx.input),
                Src::Pdf => doc_builtin::pdf_to_text(&ctx.input),
                Src::Unknown => Err(AppError::unsupported("未知文档格式")),
            }
        };
        let builtin_can_extract = matches!(
            src,
            Src::Txt | Src::Md | Src::Html | Src::Docx | Src::Odt | Src::Rtf | Src::Pdf
        );

        if is_text_src {
            if let Some(tt) = text_target {
                if tt == src {
                    // 同格式：直接复制（仍然算一次成功转换）
                    std::fs::copy(&ctx.input, &ctx.output)?;
                } else {
                    let raw = read_text(&ctx.input)?;
                    let text = match src {
                        Src::Html => html_to_text(&raw),
                        Src::Md => md_to_text(&raw),
                        _ => raw,
                    };
                    let out = match tt {
                        Src::Txt => text,
                        Src::Md => text_to_md(&text),
                        Src::Html => text_to_html(&text),
                        _ => unreachable!(),
                    };
                    std::fs::write(&ctx.output, out)?;
                }
                ctx.note("已用内置文本引擎完成转换（无需外部工具）").await;
                let bytes = std::fs::metadata(&ctx.output).map(|m| m.len()).unwrap_or(0);
                ctx.report(1.0).await;
                return Ok(ConvertOutput::single(ctx.output.to_string_lossy().to_string(), bytes));
            }
        }

        // ── 1b) Office/PDF → txt/md/html：内置抽取（零依赖） ──
        if !is_text_src && builtin_can_extract {
            if let Some(tt) = text_target {
                ctx.report(0.3).await;
                let text = extract_builtin(src)?;
                let out = match tt {
                    Src::Txt => text,
                    Src::Md => text_to_md(&text),
                    Src::Html => text_to_html(&text),
                    _ => unreachable!(),
                };
                std::fs::write(&ctx.output, out)?;
                ctx.note("已用内置解析引擎抽取正文（无需外部工具）").await;
                let bytes = std::fs::metadata(&ctx.output).map(|m| m.len()).unwrap_or(0);
                ctx.report(1.0).await;
                return Ok(ConvertOutput::single(ctx.output.to_string_lossy().to_string(), bytes));
            }
        }

        // ── 2) 图片输出（PDF/docx/... → png/jpg，支持多页） ──────
        if target == "png" || target == "jpg" || target == "jpeg" {
            ctx.report(0.25).await;
            // 页码范围：all(默认) / first / 1-3,5
            let pages_spec = extra.get("pages").and_then(|v| v.as_str()).map(|s| s.to_string());

            let produced: Vec<PathBuf> = if src == Src::Pdf {
                pdf_to_images(&ctx, &target, dpi, pages_spec.as_deref()).await?
            } else {
                // docx/odt/rtf/html/md/txt → 先转 PDF 再栅格化（同样是内置引擎）
                let tmp_pdf = ctx.output.with_extension("__stage.pdf");
                let mut sub = ConvertContext {
                    input: ctx.input.clone(),
                    output: tmp_pdf.clone(),
                    options: ctx.options.clone(),
                    cancel: ctx.cancel.clone(),
                    progress: ctx.progress.clone(),
                    log: ctx.log.clone(),
                };
                sub.options.target_ext = "pdf".into();
                // 中转 PDF 不该把「页码范围」带下去
                sub.options.extra = serde_json::Value::Null;
                self.convert(sub).await?;
                let mut img_ctx = ConvertContext {
                    input: tmp_pdf.clone(),
                    output: ctx.output.clone(),
                    options: ctx.options.clone(),
                    cancel: ctx.cancel.clone(),
                    progress: ctx.progress.clone(),
                    log: ctx.log.clone(),
                };
                img_ctx.options.target_ext = target.clone();
                img_ctx.options.extra = serde_json::Value::Null;
                let r = pdf_to_images(&img_ctx, &target, dpi, pages_spec.as_deref()).await;
                let _ = std::fs::remove_file(&tmp_pdf);
                r?
            };

            // 外部工具（pdftoppm）只会出 png，转 jpg 时用内置图像库重编码
            let produced: Vec<PathBuf> = if (target == "jpg" || target == "jpeg")
                && produced.iter().any(|p| p.extension().and_then(|s| s.to_str()) != Some("jpg"))
            {
                produced
                    .into_iter()
                    .filter_map(|p| {
                        if p.extension().and_then(|s| s.to_str()) == Some("jpg") {
                            return Some(p);
                        }
                        let img = image::open(&p).ok()?;
                        img.to_rgb8().save(&p).ok()?;
                        Some(p)
                    })
                    .collect()
            } else {
                produced
            };
            // 「保留白底」关闭时压平透明通道，避免部分查看器显示成黑块
            if !keep_bg && target == "png" {
                for p in &produced {
                    if let Ok(img) = image::open(p) {
                        let flat = image::DynamicImage::ImageRgb8(img.to_rgb8());
                        let _ = flat.save(p);
                    }
                }
            }

            let bytes: u64 = produced
                .iter()
                .filter_map(|p| std::fs::metadata(p).map(|m| m.len()).ok())
                .sum();
            ctx.report(1.0).await;
            let multi = produced.len() > 1;
            ctx.note(format!(
                "已导出 {} 张图片（{dpi} DPI）",
                produced.len()
            ))
            .await;
            if multi {
                // 多产物：主输出指向目录，方便在资源管理器里直接看全部
                let dir = produced[0]
                    .parent()
                    .map(|p| p.to_path_buf())
                    .unwrap_or_else(|| ctx.output.clone());
                return Ok(ConvertOutput {
                    output: dir.to_string_lossy().to_string(),
                    bytes,
                    extra_outputs: produced.iter().map(|p| p.to_string_lossy().to_string()).collect(),
                });
            }
            return Ok(ConvertOutput::single(
                produced[0].to_string_lossy().to_string(),
                bytes,
            ));
        }

        // ── 3) → PDF ──────────────────────────────────────────
        if target == "pdf" {
            if let Some(_) = find_tool("soffice").or_else(|| find_tool("libreoffice")) {
                // 装了 LibreOffice 就用它：能保留原始排版、图片、表格
                via_libreoffice(&ctx, "pdf").await?;
            } else if builtin_can_extract {
                // 内置引擎：抽取正文 → printpdf 排版（真文字，中文自动嵌系统字体）
                let text = extract_builtin(src)?;
                let stem = ctx.input.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "document".into());
                ctx.report(0.5).await;
                doc_builtin::write_pdf(&text, &stem, &ctx.output)?;
                ctx.note("已用内置 PDF 引擎生成（文字可选中/可搜索，中文已内嵌字体子集）").await;
            } else {
                return Err(need(
                    "LibreOffice",
                    "把该文档转成 PDF 需要 LibreOffice（soffice）",
                ));
            }
            let bytes = std::fs::metadata(&ctx.output).map(|m| m.len()).unwrap_or(0);
            ctx.report(1.0).await;
            return Ok(ConvertOutput::single(ctx.output.to_string_lossy().to_string(), bytes));
        }

        // ── 4) PDF → docx/odt/rtf：需要 LibreOffice（要保留版式）──
        //    PDF → txt/md/html 已在上面的内置抽取里处理完
        if src == Src::Pdf {
            let filter = match target.as_str() {
                "docx" => "docx:MS Word 2007 XML",
                "odt" => "odt",
                "rtf" => "rtf",
                other => return Err(AppError::unsupported(format!("暂不支持 PDF → .{other}"))),
            };
            via_libreoffice(&ctx, filter).await?;
            let bytes = std::fs::metadata(&ctx.output).map(|m| m.len()).unwrap_or(0);
            ctx.report(1.0).await;
            return Ok(ConvertOutput::single(ctx.output.to_string_lossy().to_string(), bytes));
        }

        // ── 5) docx/odt/rtf ↔ md/html/txt：pandoc 优先，其次 LibreOffice ──
        let pandoc_from = match src {
            Src::Docx => Some("docx"),
            Src::Rtf => Some("rtf"),
            Src::Odt => None,
            _ => None,
        };
        let pandoc_to = match text_target {
            Some(Src::Txt) => Some("plain"),
            Some(Src::Md) => Some("gfm"),
            Some(Src::Html) => Some("html"),
            _ => None,
        };
        if let (Some(f), Some(t)) = (pandoc_from, pandoc_to) {
            if find_tool("pandoc").is_some() {
                via_pandoc(&ctx, f, t).await?;
                let bytes = std::fs::metadata(&ctx.output).map(|m| m.len()).unwrap_or(0);
                ctx.report(1.0).await;
                return Ok(ConvertOutput::single(ctx.output.to_string_lossy().to_string(), bytes));
            }
        }
        let lo_filter = match text_target {
            Some(Src::Txt) => Some("txt:Text (encoded):UTF8"),
            Some(Src::Html) => Some("html"),
            Some(Src::Md) => Some("html"), // md 用 html 中转后再剥标签
            _ => match target.as_str() {
                "docx" => Some("docx:MS Word 2007 XML"),
                "odt" => Some("odt"),
                "rtf" => Some("rtf"),
                _ => None,
            },
        };
        match lo_filter {
            Some(f) if find_tool("soffice").is_some() || find_tool("libreoffice").is_some() => {
                via_libreoffice(&ctx, f).await?;
                if matches!(text_target, Some(Src::Md)) {
                    let raw = std::fs::read_to_string(&ctx.output)?;
                    std::fs::write(&ctx.output, md_from_plain(&html_to_text(&raw)))?;
                }
                let bytes = std::fs::metadata(&ctx.output).map(|m| m.len()).unwrap_or(0);
                ctx.report(1.0).await;
                return Ok(ConvertOutput::single(ctx.output.to_string_lossy().to_string(), bytes));
            }
            _ => {}
        }

        Err(need(
            "pandoc 或 LibreOffice",
            &format!("{src:?} → .{target} 需要其中之一，请安装后重试"),
        ))
    }
}

/// LibreOffice 输出的纯文本 → Markdown（标题/列表能猜则猜，猜不出就是纯文本）
fn md_from_plain(text: &str) -> String {
    let mut out = String::new();
    for line in text.lines() {
        let t = line.trim_end();
        if t.is_empty() {
            out.push('\n');
            continue;
        }
        // 短行且全大写/像标题 → 加 # 前缀（保守策略）
        if t.chars().count() <= 30 && t.chars().all(|c| !c.is_ascii_punctuation()) && t.contains(' ') {
            out.push_str(&format!("## {t}\n\n"));
        } else {
            out.push_str(t);
            out.push('\n');
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_strips_tags_and_entities() {
        let html = "<html><body><h1>Title</h1><p>Hello &amp; <b>World</b></p><script>bad()</script></body></html>";
        let text = html_to_text(html);
        assert!(text.contains("Title"));
        assert!(text.contains("Hello & World"));
        assert!(!text.contains("<h1>"));
        assert!(!text.contains("bad()"));
    }

    #[test]
    fn md_strips_markers() {
        let md = "# Title\n\n**bold** and `code`\n\n- item one\n\n[link](http://x)\n\n---\n";
        let text = md_to_text(md);
        assert!(text.contains("Title"));
        assert!(text.contains("bold and code"));
        assert!(text.contains("item one"));
        assert!(!text.contains("**"));
        assert!(!text.contains("http://x"));
    }

    #[test]
    fn builtin_pdf_is_generated_and_extractable() {
        // 内置引擎生成的 PDF 应能被内置解析器读回文字（round-trip）
        let dir = std::env::temp_dir().join(format!("sf_pdf_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let out = dir.join("a.pdf");
        let text = (1..=120)
            .map(|i| format!("Line {i} of ScrollFormat built-in PDF"))
            .collect::<Vec<_>>()
            .join("\n");
        doc_builtin::write_pdf(&text, "t", &out).unwrap();
        let bytes = std::fs::read(&out).unwrap();
        assert!(bytes.len() > 500);
        let back = doc_builtin::pdf_to_text(&out).unwrap();
        assert!(back.contains("Line 1"), "{back}");
        assert!(back.contains("Line 120"), "{back}");
        let _ = std::fs::remove_dir_all(dir);
    }
    #[test]
    fn page_range_spec_is_parsed() {
        // 默认 / all → 全部页
        assert_eq!(parse_page_range(None, 5), vec![0, 1, 2, 3, 4]);
        assert_eq!(parse_page_range(Some(""), 3), vec![0, 1, 2]);
        assert_eq!(parse_page_range(Some("all"), 2), vec![0, 1]);
        // 首页
        assert_eq!(parse_page_range(Some("first"), 4), vec![0]);
        assert_eq!(parse_page_range(Some("1"), 4), vec![0]);
        // 区间 + 离散页码
        assert_eq!(parse_page_range(Some("1-3"), 6), vec![0, 1, 2]);
        assert_eq!(parse_page_range(Some("1-3,5"), 6), vec![0, 1, 2, 4]);
        // 开区间：到末页
        assert_eq!(parse_page_range(Some("2-"), 3), vec![1, 2]);
        // 越界要被夹住，不能 panic
        assert_eq!(parse_page_range(Some("1-99"), 3), vec![0, 1, 2]);
        // 单页越界 → 夹到末页（而不是退回全部）
        assert_eq!(parse_page_range(Some("9"), 3), vec![2]);
        assert_eq!(parse_page_range(Some("99"), 3), vec![2]);
        // 去重 + 排序
        assert_eq!(parse_page_range(Some("3,1,3"), 3), vec![0, 2]);
        // 完全非法 → 退回全部页
        assert_eq!(parse_page_range(Some("abc"), 2), vec![0, 1]);
        // 0 页时不能崩
        assert!(parse_page_range(Some("all"), 0).is_empty());
    }

    #[test]
    fn multi_page_names_are_numbered() {
        assert_eq!(page_file_name("doc", 0, 1, "png"), "doc.png");
        assert_eq!(page_file_name("doc", 0, 12, "png"), "doc-01.png");
        assert_eq!(page_file_name("doc", 8, 12, "jpg"), "doc-09.jpg");
    }

    #[test]
    fn detect_src_maps_extensions() {
        assert_eq!(detect_src(Path::new("a.md")), Src::Md);
        assert_eq!(detect_src(Path::new("a.PDF")), Src::Pdf);
        assert_eq!(detect_src(Path::new("a.xyz")), Src::Unknown);
    }
}
