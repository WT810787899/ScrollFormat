use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Default, Debug)]
#[serde(rename_all = "snake_case")]
pub struct NamingRule {
    #[serde(default = "default_template")]
    pub template: String,
    #[serde(default)]
    pub prefix: String,
    #[serde(default)]
    pub suffix: String,
    #[serde(default)]
    pub conflict: ConflictPolicy,
    #[serde(default = "default_index_start")]
    pub index_start: u32,
    #[serde(default)]
    pub index_width: u32,
}

fn default_template() -> String {
    "{name}".to_string()
}
fn default_index_start() -> u32 {
    1
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Default, Debug)]
#[serde(rename_all = "snake_case")]
pub enum ConflictPolicy {
    Overwrite,
    Skip,
    #[default]
    AutoRename,
}

/// 渲染输出文件主名（不含扩展名）
pub fn render_output_name(rule: &NamingRule, source: &std::path::Path, index: u32) -> crate::error::Result<String> {
    let invalid = |msg: &str| crate::error::AppError::new(crate::error::ErrorCode::NamingInvalid, msg);
    let name = source.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let mut out = String::with_capacity(rule.template.len() + 32);
    let mut rest = rule.template.as_str();
    while let Some(start) = rest.find('{') {
        out.push_str(&rest[..start]);
        let end = rest[start..].find('}').ok_or_else(|| invalid("命名规则占位符未闭合"))? + start;
        let placeholder = &rest[start + 1..end];
        let value = render_placeholder(placeholder, &name, index)?;
        out.push_str(&value);
        rest = &rest[end + 1..];
    }
    out.push_str(rest);

    let result = format!("{}{}{}", rule.prefix, out, rule.suffix).trim().to_string();
    if result.is_empty() {
        return Err(invalid("命名规则渲染结果为空"));
    }
    if result.chars().any(|c| matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|')) {
        return Err(invalid("命名规则包含非法字符"));
    }
    Ok(result)
}

fn render_placeholder(placeholder: &str, name: &str, index: u32) -> crate::error::Result<String> {
    match placeholder {
        "name" | "originalName" => Ok(name.to_string()),
        "index" => Ok(index.to_string()),
        p if p.starts_with("index:") => {
            let width: usize = p[6..].parse().map_err(|_| crate::error::AppError::new(crate::error::ErrorCode::NamingInvalid, "index 补零位数非法"))?;
            Ok(format!("{:0width$}", index, width = width))
        }
        "timestamp" => Ok(chrono::Local::now().format("%Y%m%d").to_string()),
        "timestamp_full" => Ok(chrono::Local::now().format("%Y%m%d_%H%M%S").to_string()),
        "ext" => Ok(String::new()),
        other => Err(crate::error::AppError::new(crate::error::ErrorCode::NamingInvalid, &format!("不支持的占位符 {{{other}}}"))),
    }
}
