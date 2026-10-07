//! 纯内置的文档解析与 PDF 生成（不依赖任何外部可执行文件）
//!
//! - `docx/odt`：本质是 ZIP + XML，用 zip + 手写标签流解析出正文
//! - `rtf`：剥离控制字，解析 `\'hh` 与 `\uNNNN` 转义
//! - `pdf → 文本`：pdf-extract（纯 Rust，含 ToUnicode/字体映射）
//! - `文本 → pdf`：printpdf（真文字可选中/可搜索），中文自动加载系统 CJK 字体并做子集化

use std::fs::File;
use std::io::{BufWriter, Read, Write};
use std::path::Path;

use printpdf::*;
use scroll_format_core::AppError;
use unicode_normalization::UnicodeNormalization;

/// Unicode 规范化（NFKC）
///
/// PDF 的 ToUnicode 表常把「文/行」这类字映射成康熙部首或兼容汉字
/// （⽂ U+2F0x、⾏ U+2F1x、U+F900 段），显示看着一样但复制出来是错的、
/// 搜索也匹配不上。NFKC 会把它们统一回标准汉字。
fn nfkc(s: &str) -> String {
    s.nfkc().collect()
}

// ═══════════════════════════════════════════════════════════
// 通用工具
// ═══════════════════════════════════════════════════════════

fn read_zip_entry(path: &Path, name: &str) -> Result<String, AppError> {
    let file = File::open(path).map_err(|e| AppError::io(format!("打开失败：{e}")))?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| AppError::io(format!("不是有效的 Office 文件（ZIP 解析失败）：{e}")))?;
    // 有些工具（以及 Windows PowerShell 的 Compress-Archive）会把条目名写成
    // 反斜杠，违反 ZIP 规范；这里做归一化 + 后缀匹配，兼容这类文件
    let want = name.replace('\\', "/");
    let mut index = None;
    for i in 0..zip.len() {
        let n = match zip.by_index(i) {
            Ok(e) => e.name().replace('\\', "/"),
            Err(_) => continue,
        };
        if n == want || n.ends_with(&format!("/{want}")) {
            index = Some(i);
            break;
        }
    }
    let idx = index.ok_or_else(|| AppError::conv(format!("文件里找不到 {name}，可能不是标准的 Office 文档")))?;
    let mut entry = zip.by_index(idx).map_err(|e| AppError::io(format!("读取压缩条目失败：{e}")))?;
    let mut buf = Vec::new();
    entry.read_to_end(&mut buf).map_err(|e| AppError::io(e.to_string()))?;
    Ok(String::from_utf8_lossy(&buf).to_string())
}

/// XML 文本抽取：段落/换行/制表符 → 纯文本，跳过标签本体
fn xml_to_text(xml: &str) -> String {
    let mut out = String::with_capacity(xml.len());
    let mut skip_depth = 0usize;
    let mut skip_tag = String::new();
    let mut tab_pending = false;
    let bytes = xml.as_bytes();
    let mut i = 0usize;
    while i < xml.len() {
        if bytes[i] == b'<' {
            let end = xml[i..].find('>').map(|p| i + p).unwrap_or_else(|| xml.len().saturating_sub(1));
            let inner_end = end.max(i + 1);
            let raw = &xml[i + 1..inner_end];
            // 注释 / CDATA / XML 声明
            if raw.starts_with('?') || raw.starts_with('!') {
                i = inner_end + 1;
                continue;
            }
            let is_close = raw.starts_with('/');
            let body = raw.trim_start_matches('/').trim();
            let name: String = body.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == ':').collect();
            if skip_tag.is_empty() {
                if name == "script" || name == "style" || name == "head" {
                    skip_tag = name.clone();
                    skip_depth = 1;
                }
            } else if name == skip_tag {
                skip_depth = if is_close { skip_depth.saturating_sub(1) } else { skip_depth + 1 };
                if skip_depth == 0 {
                    skip_tag.clear();
                }
            }
            // 段落类标签 → 换行；制表 → 对齐空格
            if skip_tag.is_empty() && !is_close {
                match name.as_str() {
                    "w:p" | "w:br" | "text:p" | "text:line-break" | "w:tr" | "w:tc" => out.push('\n'),
                    "w:tab" | "text:tab" => tab_pending = true,
                    "w:cr" => out.push('\n'),
                    _ => {}
                }
            }
            i = inner_end + 1;
            continue;
        }
        if !skip_tag.is_empty() {
            i += 1;
            continue;
        }
        let ch = xml[i..].chars().next().unwrap_or(' ');
        if tab_pending {
            // 表格/缩进里的制表符折算成空格
            for _ in 0..4 {
                out.push(' ');
            }
            tab_pending = false;
            i += ch.len_utf8();
            continue;
        }
        if ch == '\n' || ch == '\r' || ch == '\t' {
            i += 1;
            continue;
        }
        out.push(ch);
        i += ch.len_utf8();
    }
    // 实体解码 + 压缩空行
    let text = nfkc(&out
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&"));
    let mut lines: Vec<String> = Vec::new();
    for l in text.lines() {
        let l = l.trim_end();
        if l.is_empty() {
            if lines.last().map(|p: &String| !p.is_empty()).unwrap_or(false) {
                lines.push(String::new());
            }
        } else {
            lines.push(l.to_string());
        }
    }
    while lines.last().map(|l| l.is_empty()).unwrap_or(false) {
        lines.pop();
    }
    lines.join("\n")
}

pub fn docx_to_text(path: &Path) -> Result<String, AppError> {
    let xml = read_zip_entry(path, "word/document.xml")?;
    Ok(xml_to_text(&xml))
}

pub fn odt_to_text(path: &Path) -> Result<String, AppError> {
    let xml = read_zip_entry(path, "content.xml")?;
    Ok(xml_to_text(&xml))
}

/// RTF → 纯文本：处理控制字、组边界、`\'hh`（ANSI 码页）与 `\uNNNN`（Unicode）
pub fn rtf_to_text(path: &Path) -> Result<String, AppError> {
    let raw = std::fs::read(path).map_err(|e| AppError::io(format!("读取失败：{e}")))?;
    let mut out = String::new();
    let mut skip_dest = false; // \*\... 目标（字体表/元数据）
    let mut pending_skip = false;
    let bytes = raw;
    let mut i = 0usize;
    let mut pending_hex: Option<u8> = None;
    while i < bytes.len() {
        let b = bytes[i];
        match b {
            b'\\' => {
                pending_hex = None;
                i += 1;
                if i >= bytes.len() {
                    break;
                }
                match bytes[i] {
                    b'\\' | b'{' | b'}' => {
                        if !skip_dest {
                            out.push(bytes[i] as char);
                        }
                        i += 1;
                    }
                    b'\'' => {
                        // \'hh
                        if i + 2 < bytes.len() {
                            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or("20");
                            if let Ok(v) = u8::from_str_radix(hex, 16) {
                                pending_hex = Some(v);
                            }
                            i += 3;
                        } else {
                            i += 1;
                        }
                    }
                    b'u' => {
                        // \uNNNN?  负数表示超出 BMP 的补充平面字符，这里按原样处理
                        i += 1;
                        let mut num = 0i64;
                        let mut digits = 0;
                        let neg = bytes.get(i) == Some(&b'-');
                        if neg {
                            i += 1;
                        }
                        while i < bytes.len() && bytes[i].is_ascii_digit() && digits < 6 {
                            num = num * 10 + (bytes[i] - b'0') as i64;
                            i += 1;
                            digits += 1;
                        }
                        if i < bytes.len() {
                            i += 1; // 吃掉结尾的占位字符
                        }
                        if !skip_dest && digits > 0 {
                            let cp = if neg { -num } else { num };
                            if let Some(c) = char::from_u32(cp.rem_euclid(0x10000) as u32) {
                                out.push(c);
                            }
                        }
                    }
                    b'*' => {
                        pending_skip = true;
                        i += 1;
                    }
                    c if c.is_ascii_alphabetic() => {
                        // 控制字：读名字 + 可选数字参数
                        let start = i;
                        while i < bytes.len() && bytes[i].is_ascii_alphabetic() {
                            i += 1;
                        }
                        let word = &bytes[start..i];
                        while i < bytes.len() && (bytes[i] == b'-' || bytes[i].is_ascii_digit()) {
                            i += 1;
                        }
                        if i < bytes.len() && bytes[i] == b' ' {
                            i += 1;
                        }
                        match word {
                            b"fonttbl" | b"colortbl" | b"stylesheet" | b"info" | b"pict" | b"object"
                            | b"themedata" | b"colorschememapping" | b"latentstyles" | b"datastore"
                            | b"listtable" | b"rsidtbl" | b"generator" | b"xmlnstbl" | b"mmathPr" => {
                                skip_dest = true;
                                pending_skip = false;
                            }
                            b"par" | b"line" | b"row" | b"sect" | b"page" => {
                                if !skip_dest {
                                    out.push('\n');
                                }
                            }
                            b"tab" => {
                                if !skip_dest {
                                    for _ in 0..4 {
                                        out.push(' ');
                                    }
                                }
                            }
                            b"cell" => {
                                if !skip_dest {
                                    out.push('\t');
                                }
                            }
                            b"lquote" => {
                                if !skip_dest {
                                    out.push('\u{2018}');
                                }
                            }
                            b"rquote" => {
                                if !skip_dest {
                                    out.push('\u{2019}');
                                }
                            }
                            b"ldblquote" => {
                                if !skip_dest {
                                    out.push('\u{201c}');
                                }
                            }
                            b"rdblquote" => {
                                if !skip_dest {
                                    out.push('\u{201d}');
                                }
                            }
                            b"endash" => {
                                if !skip_dest {
                                    out.push('\u{2013}');
                                }
                            }
                            b"emdash" => {
                                if !skip_dest {
                                    out.push('\u{2014}');
                                }
                            }
                            b"bullet" => {
                                if !skip_dest {
                                    out.push('\u{2022}');
                                }
                            }
                            _ => {}
                        }
                        if pending_skip {
                            pending_skip = false;
                        }
                    }
                    _ => {
                        i += 1;
                    }
                }
            }
            b'{' | b'}' => {
                // 组边界：结束时恢复状态（简化处理：结束即解除 skip）
                if b == b'}' {
                    skip_dest = false;
                }
                i += 1;
            }
            b'\r' | b'\n' => {
                i += 1;
            }
            _ => {
                if let Some(h) = pending_hex.take() {
                    // ANSI 码页：中文 RTF 常用 GB2312（134），这里按 UTF-8 尝试，失败则按 Latin-1
                    let s = if h >= 0x80 {
                        String::from_utf8(vec![h]).unwrap_or_else(|_| (h as char).to_string())
                    } else {
                        (h as char).to_string()
                    };
                    if !skip_dest {
                        out.push_str(&s);
                    }
                    i += 1;
                    continue;
                }
                if !skip_dest {
                    // 多字节 UTF-8 序列原样拷贝
                    let len = utf8_len(b);
                    if len == 1 {
                        out.push(b as char);
                        i += 1;
                    } else {
                        let end = (i + len).min(bytes.len());
                        out.push_str(&String::from_utf8_lossy(&bytes[i..end]));
                        i = end;
                    }
                } else {
                    i += 1;
                }
            }
        }
    }
    let mut lines: Vec<String> = Vec::new();
    for l in out.lines() {
        let l = l.trim_end().replace('\t', "    ");
        if l.trim().is_empty() {
            if lines.last().map(|p: &String| !p.trim().is_empty()).unwrap_or(false) {
                lines.push(String::new());
            }
        } else {
            lines.push(l);
        }
    }
    while lines.last().map(|l| l.trim().is_empty()).unwrap_or(false) {
        lines.pop();
    }
    Ok(lines.join("\n"))
}

fn utf8_len(b: u8) -> usize {
    if b < 0x80 {
        1
    } else if b >> 5 == 0b110 {
        2
    } else if b >> 4 == 0b1110 {
        3
    } else if b >> 3 == 0b11110 {
        4
    } else {
        1
    }
}

/// PDF → 纯文本（pdf-extract，纯 Rust；会处理字体 ToUnicode 映射）
pub fn pdf_to_text(path: &Path) -> Result<String, AppError> {
    let raw = pdf_extract::extract_text(path).map_err(|e| AppError::conv(format!("PDF 文本抽取失败（可能是扫描件/图片型 PDF）：{e}")))?;
    let text = nfkc(&raw);
    let mut lines: Vec<String> = Vec::new();
    for l in text.lines() {
        let l = l.trim_end();
        if l.trim().is_empty() {
            if lines.last().map(|p: &String| !p.trim().is_empty()).unwrap_or(false) {
                lines.push(String::new());
            }
        } else {
            lines.push(l.to_string());
        }
    }
    while lines.last().map(|l| l.trim().is_empty()).unwrap_or(false) {
        lines.pop();
    }
    Ok(lines.join("\n"))
}

// ═══════════════════════════════════════════════════════════
// PDF → 图片（内嵌 PDFium，Chrome 同款渲染引擎）
// ═══════════════════════════════════════════════════════════

use std::sync::{Mutex, MutexGuard, OnceLock};

// pdfium-render 0.9 只通过 prelude 导出类型（Pdfium / PdfRenderConfig / Pixels…）
use pdfium_render::prelude::*;

/// PDFium 实例很重（含全局状态 + 内部线程池），整个进程只初始化一份。
/// 它也不是完全线程安全的，所以渲染操作走 Mutex 串行执行——
/// 反正单页渲染很快，串行不影响体感。
static PDFIUM: OnceLock<Mutex<Result<Pdfium, String>>> = OnceLock::new();

/// 拿到（并首次初始化）内置 PDF 引擎，返回的 guard 必须活到渲染结束
fn pdfium() -> Result<MutexGuard<'static, Result<Pdfium, String>>, AppError> {
    let cell = PDFIUM.get_or_init(|| {
        // bundled 特性：pdfium.dll 已在编译期嵌进二进制，这里既不联网也不落盘
        Mutex::new(pdfium_bundled::bind_bundled().map_err(|e| e.to_string()))
    });
    let locked = cell.lock().map_err(|_| AppError::conv("内置 PDF 引擎锁异常"))?;
    match locked.as_ref() {
        Ok(_) => Ok(locked),
        Err(e) => Err(AppError::conv(format!("内置 PDF 渲染引擎不可用：{e}"))),
    }
}

/// 内置渲染是否可用（用于环境检测，只探测一次）
pub fn probe_pdfium() -> bool {
    pdfium().is_ok()
}

/// PDF 页数
pub fn pdf_page_count(pdf_path: &Path) -> Result<usize, AppError> {
    let guard = pdfium()?;
    let pdfium = guard
        .as_ref()
        .map_err(|e| AppError::conv(format!("内置 PDF 渲染引擎不可用：{e}")))?;
    let document = pdfium
        .load_pdf_from_file(pdf_path, None)
        .map_err(|e| AppError::conv(format!("PDF 打开失败：{e}")))?;
    Ok(document.pages().len().max(0) as usize)
}

/// 内置渲染 PDF 第 `page` 页（0 起）并编码到内存
///
/// - `dpi`：目标分辨率（150 DPI 时 A4 ≈ 1240×1754 px）
/// - `jpeg`：true → JPEG，false → PNG
pub fn render_pdf_page(
    pdf_path: &Path,
    page: usize,
    dpi: u32,
    jpeg: bool,
    quality: u8,
) -> Result<(Vec<u8>, u32, u32), AppError> {
    let guard = pdfium()?;
    let pdfium = guard
        .as_ref()
        .map_err(|e| AppError::conv(format!("内置 PDF 渲染引擎不可用：{e}")))?;
    let document = pdfium
        .load_pdf_from_file(pdf_path, None)
        .map_err(|e| AppError::conv(format!("PDF 打开失败：{e}")))?;

    let pages = document.pages();
    let total = pages.len();
    if total <= 0 {
        return Err(AppError::conv("PDF 里没有任何页面"));
    }
    let idx = i32::try_from(page).map_err(|_| AppError::conv("页码超出范围"))?;
    if idx >= total {
        return Err(AppError::conv(format!("PDF 一共 {total} 页，取不到第 {} 页", page + 1)));
    }
    let pdf_page = pages.get(idx).map_err(|e| AppError::conv(format!("取页失败：{e}")))?;

    // PDF 用户单位 = 1/72 英寸，A4 是 595×842 pt
    let w_pt = pdf_page.width().value;
    let h_pt = pdf_page.height().value;
    let scale = dpi.max(36) as f32 / 72.0;
    // Pixels 就是 i32 的别名，所以这里直接产出 i32
    let w_px = ((w_pt * scale).round() as i32).clamp(1, 12000);
    let h_px = ((h_pt * scale).round() as i32).clamp(1, 12000);

    let bitmap = pdf_page
        .render(w_px, h_px, None)
        .map_err(|e| AppError::conv(format!("页面渲染失败：{e}")))?;

    let dynamic = bitmap.as_image().map_err(|e| AppError::conv(format!("像素读取失败：{e}")))?;
    // PDF 背景默认透明，按页面语义压成白底（打印/看图时不会变黑块）
    let rgb = dynamic.to_rgb8();
    let (width, height) = rgb.dimensions();

    // 注意：本 crate 里有个本地模块叫 `image`（图片转换器），
    // 会遮蔽同名的外部 crate，所以这里必须用 `::image` 限定。
    use ::image::ImageEncoder;
    let data = if jpeg {
        let mut buf = Vec::new();
        ::image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, quality.clamp(40, 100))
            .encode_image(&rgb)
            .map_err(|e| AppError::io(e.to_string()))?;
        buf
    } else {
        let mut buf = Vec::new();
        ::image::codecs::png::PngEncoder::new(&mut buf)
            .write_image(rgb.as_raw(), width, height, ::image::ExtendedColorType::Rgb8)
            .map_err(|e| AppError::io(e.to_string()))?;
        buf
    };

    Ok((data, width, height))
}

// ═══════════════════════════════════════════════════════════
// 文本 → PDF（printpdf，真文字）
// ═══════════════════════════════════════════════════════════

/// 系统里可用的中文字体（Windows 优先，其次 Linux/macOS 常见路径）
fn cjk_font_candidates() -> Vec<&'static str> {
    if cfg!(windows) {
        vec![
            r"C:\Windows\Fonts\Deng.ttf",       // 等线
            r"C:\Windows\Fonts\simhei.ttf",     // 黑体
            r"C:\Windows\Fonts\msyh.ttc",       // 微软雅黑
            r"C:\Windows\Fonts\simsun.ttc",     // 宋体
            r"C:\Windows\Fonts\msjh.ttc",       // 微软正黑体
            r"C:\Windows\Fonts\simkai.ttf",     // 楷体
        ]
    } else if cfg!(target_os = "macos") {
        vec![
            "/System/Library/Fonts/PingFang.ttc",
            "/System/Library/Fonts/STHeiti Medium.ttc",
            "/Library/Fonts/Arial Unicode.ttf",
        ]
    } else {
        vec![
            "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/truetype/wqy/wqy-zenhei.ttc",
            "/usr/share/fonts/truetype/arphic/uming.ttc",
        ]
    }
}

fn needs_cjk(text: &str) -> bool {
    text.chars().any(|c| {
        let u = c as u32;
        (0x2E80..=0x9FFF).contains(&u)      // CJK 部首/汉字
            || (0xF900..=0xFAFF).contains(&u) // 兼容汉字
            || (0xFF00..=0xFFEF).contains(&u) // 全角标点
            || (0x3000..=0x303F).contains(&u) // CJK 标点
    })
}

/// 粗略字宽（em 为单位）：CJK/全角按 1.0，其余按常见比例
fn text_width_em(s: &str) -> f32 {
    let mut w = 0.0f32;
    for c in s.chars() {
        w += match c {
            '\u{4e00}'..='\u{9fff}' | '\u{3000}'..='\u{303f}' | '\u{ff00}'..='\u{ffef}' => 1.0,
            'i' | 'l' | 'j' | 'I' | 't' | 'f' | 'r' | '.' | ',' | ':' | ';' | '!' | '\'' | '|' | '(' | ')' | '[' | ']' => 0.32,
            'm' | 'w' | 'M' | 'W' | '@' => 0.85,
            ' ' => 0.3,
            _ => 0.55,
        };
    }
    w
}

/// 按可用宽度折行（中文按字断行，西文按词断行）
fn wrap_line(line: &str, size: f32, usable_mm: f32, cjk: bool) -> Vec<String> {
    let limit_em = usable_mm / (size * 25.4 / 72.0); // pt → mm
    if line.is_empty() {
        return vec![String::new()];
    }
    if text_width_em(line) <= limit_em {
        return vec![line.to_string()];
    }
    let mut out: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut cur_w = 0.0f32;
    if cjk {
        for ch in line.chars() {
            let cw = text_width_em(&ch.to_string());
            if cur_w + cw > limit_em && !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
                cur_w = 0.0;
            }
            cur.push(ch);
            cur_w += cw;
        }
    } else {
        for word in line.split_inclusive(' ') {
            let ww = text_width_em(word);
            if cur_w + ww > limit_em && !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
                cur_w = 0.0;
            }
            cur.push_str(word);
            cur_w += ww;
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// 文本 → PDF：可选中文（真文字，可选中/可搜索；字体子集化，避免体积爆炸）
pub fn write_pdf(text: &str, title: &str, out: &Path) -> Result<(), AppError> {
    let cjk = needs_cjk(text);
    // A4 210×297mm，边距 18mm
    let (page_w, page_h) = (210.0f32, 297.0f32);
    let margin = 18.0f32;
    let usable_w = page_w - margin * 2.0;
    let size = 11.0f32; // pt
    let leading = 16.0f32; // pt

    let (doc, _first_page, _first_layer) =
        PdfDocument::new(title.to_string(), Mm(page_w), Mm(page_h), "content");

    // 字体：中文文本用系统 CJK 字体（子集化），纯西文用内置 Helvetica
    let mut font: Option<IndirectFontRef> = None;
    if cjk {
        for cand in cjk_font_candidates() {
            let p = Path::new(cand);
            if !p.exists() {
                continue;
            }
            if let Ok(f) = File::open(p) {
                match doc.add_external_font_with_subsetting(f, true) {
                    Ok(fr) => {
                        font = Some(fr);
                        break;
                    }
                    Err(_) => continue,
                }
            }
        }
    }
    if font.is_none() {
        font = doc.add_builtin_font(BuiltinFont::Helvetica).ok();
    }
    let font = font.ok_or_else(|| AppError::conv("PDF 引擎初始化失败：无法加载任何字体"))?;

    // 预排版：按页切分
    let mut pages: Vec<Vec<String>> = Vec::new();
    let mut cur: Vec<String> = Vec::new();
    let mut y = page_h - margin;
    let bottom = margin;
    for raw_line in text.lines() {
        for seg in wrap_line(raw_line, size, usable_w, cjk) {
            let dy = leading * 25.4 / 72.0;
            if y - dy < bottom {
                pages.push(std::mem::take(&mut cur));
                y = page_h - margin;
            }
            cur.push(seg);
            y -= dy;
        }
    }
    pages.push(cur);
    if pages.is_empty() {
        pages.push(Vec::new());
    }

    for (i, page_lines) in pages.into_iter().enumerate() {
        // PdfDocument::new 已经建好第 1 页了，直接复用；
        // 后面每一页才 add_page——否则会在最前面多出一张空白页。
        let (page_idx, layer_idx) = if i == 0 {
            (_first_page, _first_layer)
        } else {
            doc.add_page(Mm(page_w), Mm(page_h), "content")
        };
        let layer = doc.get_page(page_idx).get_layer(layer_idx);
        let mut y = page_h - margin - size * 25.4 / 72.0;
        for l in page_lines {
            if !l.is_empty() {
                layer.use_text(l.clone(), size, Mm(margin), Mm(y), &font);
            }
            y -= leading * 25.4 / 72.0;
        }
    }

    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = File::create(out).map_err(|e| AppError::io(format!("无法写入 PDF：{e}")))?;
    let mut w = BufWriter::new(file);
    doc.save(&mut w).map_err(|e| AppError::conv(format!("生成 PDF 失败：{e}")))?;
    w.flush().ok();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmpdir(tag: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("sf_{tag}_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// 用 zip crate 造一个最小 docx，验证解析链路
    fn make_docx(dir: &Path) -> std::path::PathBuf {
        let path = dir.join("t.docx");
        let f = File::create(&path).unwrap();
        let mut zw = zip::ZipWriter::new(f);
        let opts: zip::write::FileOptions<'_, ()> = zip::write::FileOptions::default();
        zw.start_file("[Content_Types].xml", opts).unwrap();
        zw.write_all(b"<?xml version=\"1.0\"?><Types/>").unwrap();
        zw.start_file("word/document.xml", opts).unwrap();
        zw.write_all(
            br#"<?xml version="1.0"?><w:document xmlns:w="x"><w:body>
              <w:p><w:r><w:t>Hello DOCX</w:t></w:r></w:p>
              <w:p><w:r><w:t>Second &amp; last</w:t></w:r></w:p>
            </w:body></w:document>"#,
        )
        .unwrap();
        zw.finish().unwrap();
        path
    }

    fn make_odt(dir: &Path) -> std::path::PathBuf {
        let path = dir.join("t.odt");
        let f = File::create(&path).unwrap();
        let mut zw = zip::ZipWriter::new(f);
        let opts: zip::write::FileOptions<'_, ()> = zip::write::FileOptions::default();
        zw.start_file("mimetype", opts).unwrap();
        zw.write_all(b"application/vnd.oasis.opendocument.text").unwrap();
        zw.start_file("content.xml", opts).unwrap();
        zw.write_all(br#"<?xml version="1.0"?><office:document-content><office:body><office:text>
            <text:p>Hello ODT</text:p><text:p>line two</text:p>
          </office:text></office:body></office:document-content>"#)
        .unwrap();
        zw.finish().unwrap();
        path
    }

    #[test]
    fn docx_text_is_extracted() {
        let d = tmpdir("docx");
        let p = make_docx(&d);
        let t = docx_to_text(&p).unwrap();
        assert!(t.contains("Hello DOCX"), "{t}");
        assert!(t.contains("Second & last"), "{t}");
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn odt_text_is_extracted() {
        let d = tmpdir("odt");
        let p = make_odt(&d);
        let t = odt_to_text(&p).unwrap();
        assert!(t.contains("Hello ODT"), "{t}");
        assert!(t.contains("line two"), "{t}");
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn rtf_control_words_are_stripped() {
        let d = tmpdir("rtf");
        let p = d.join("t.rtf");
        let rtf = br"{\rtf1\ansi\deff0{\fonttbl{\f0 Times;}}\fs24 Hello \b RTF\b0  world\par second line\par}";
        std::fs::write(&p, rtf).unwrap();
        let t = rtf_to_text(&p).unwrap();
        assert!(t.contains("Hello"), "{t}");
        assert!(t.contains("world"), "{t}");
        assert!(t.contains("second line"), "{t}");
        assert!(!t.contains("fonttbl"), "{t}");
        assert!(!t.contains("\\"), "{t}");
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn pdf_roundtrip_keeps_chinese() {
        // 中文 PDF：验证「系统 CJK 字体嵌入 + 子集化 + ToUnicode 映射」这条链路
        let d = tmpdir("pdf_cjk");
        let pdf = d.join("cn.pdf");
        let src = "中文测试：格式卷轴\n第二行内容 with ASCII 123";
        write_pdf(src, "cn", &pdf).unwrap();
        let size = std::fs::metadata(&pdf).map(|m| m.len()).unwrap_or(0);
        assert!(size > 1000, "PDF 太小，可能没嵌入字体: {size}");
        let back = pdf_to_text(&pdf).unwrap();
        assert!(back.contains("中文"), "回读丢失中文: {back}");
        assert!(back.contains("格式卷轴"), "回读丢失中文: {back}");
        assert!(back.contains("ASCII 123"), "回读丢失西文: {back}");
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn pdfium_renders_pdf_to_png_and_jpeg() {
        // 内置光栅化闭环：内置引擎写 PDF → 内置 PDFium 渲染成 PNG/JPEG
        let d = tmpdir("raster");
        let pdf = d.join("r.pdf");
        write_pdf("Raster test page\n第二行文字 ABC 123", "r", &pdf).unwrap();

        let pages = pdf_page_count(&pdf).unwrap();
        assert_eq!(pages, 1, "应当只有 1 页");

        let (png, w, h) = render_pdf_page(&pdf, 0, 72, false, 92).unwrap();
        // A4 @72DPI = 595×842
        assert!(w > 500 && w < 700, "宽度不对: {w}");
        assert!(h > 800 && h < 900, "高度不对: {h}");
        assert_eq!(&png[1..4], b"PNG", "PNG 魔数不对");

        let (jpg, _, _) = render_pdf_page(&pdf, 0, 150, true, 90).unwrap();
        assert_eq!(&jpg[..2], &[0xFF, 0xD8], "JPEG 魔数不对");
        // 150 DPI 下应该比 72 DPI 体积不小（内容更多）
        assert!(jpg.len() > 1000, "JPEG 太小: {}", jpg.len());

        // 越界页码要给出中文报错而不是 panic
        let err = render_pdf_page(&pdf, 5, 72, false, 92).unwrap_err();
        assert!(err.to_string().contains("取不到"), "{err}");

        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn wrap_respects_width() {
        let long = "这是一段很长的中文文本用来测试自动换行是否按宽度正确折行处理";
        let out = wrap_line(long, 11.0, 100.0, true);
        assert!(out.len() > 1, "应该被折成多行");
        for l in &out {
            assert!(text_width_em(l) * 11.0 * 25.4 / 72.0 <= 100.0 + 11.0 * 25.4 / 72.0, "行宽超限: {l}");
        }
    }

    #[test]
    fn pdf_roundtrip_keeps_text() {
        let d = tmpdir("pdf");
        let pdf = d.join("a.pdf");
        let src = "Line one\nLine two with numbers 12345";
        write_pdf(src, "roundtrip", &pdf).unwrap();
        assert!(std::fs::metadata(&pdf).map(|m| m.len()).unwrap_or(0) > 300);
        // pdf-extract 读回来（printpdf 写的字体带 ToUnicode，可正确还原）
        let back = pdf_to_text(&pdf).unwrap();
        assert!(back.contains("Line one"), "{back}");
        assert!(back.contains("12345"), "{back}");
        let _ = std::fs::remove_dir_all(d);
    }
}
