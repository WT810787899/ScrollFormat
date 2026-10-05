use thiserror::Error;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub enum ErrorCode {
    #[serde(rename = "FMT-ENV-001")]
    EnvProbe,
    #[serde(rename = "FMT-ENV-002")]
    ToolMissing,
    #[serde(rename = "FMT-CONV-001")]
    ConvertFailed,
    #[serde(rename = "FMT-CONV-002")]
    UnsupportedFormat,
    #[serde(rename = "FMT-IO-001")]
    Io,
    #[serde(rename = "FMT-IO-002")]
    OutputNotWritable,
    #[serde(rename = "FMT-PROC-001")]
    ProcessFailed,
    #[serde(rename = "FMT-PROC-002")]
    ProcessTimeout,
    #[serde(rename = "FMT-USER-001")]
    Cancelled,
    #[serde(rename = "FMT-USER-002")]
    InvalidArgs,
    #[serde(rename = "FMT-USER-003")]
    NamingInvalid,
    #[serde(rename = "FMT-USER-004")]
    PresetCorrupt,
}

impl std::fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            ErrorCode::EnvProbe => "FMT-ENV-001",
            ErrorCode::ToolMissing => "FMT-ENV-002",
            ErrorCode::ConvertFailed => "FMT-CONV-001",
            ErrorCode::UnsupportedFormat => "FMT-CONV-002",
            ErrorCode::Io => "FMT-IO-001",
            ErrorCode::OutputNotWritable => "FMT-IO-002",
            ErrorCode::ProcessFailed => "FMT-PROC-001",
            ErrorCode::ProcessTimeout => "FMT-PROC-002",
            ErrorCode::Cancelled => "FMT-USER-001",
            ErrorCode::InvalidArgs => "FMT-USER-002",
            ErrorCode::NamingInvalid => "FMT-USER-003",
            ErrorCode::PresetCorrupt => "FMT-USER-004",
        };
        write!(f, "{s}")
    }
}

#[derive(Debug, Error, Clone, serde::Serialize, serde::Deserialize)]
#[error("[{code}] {message}")]
pub struct AppError {
    pub code: ErrorCode,
    pub message: String,
}

impl AppError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self { code, message: message.into() }
    }
    pub fn io(msg: impl Into<String>) -> Self { Self::new(ErrorCode::Io, msg) }
    pub fn conv(msg: impl Into<String>) -> Self { Self::new(ErrorCode::ConvertFailed, msg) }
    pub fn unsupported(msg: impl Into<String>) -> Self { Self::new(ErrorCode::UnsupportedFormat, msg) }
    pub fn cancelled() -> Self { Self::new(ErrorCode::Cancelled, "用户已取消") }
    pub fn invalid(msg: impl Into<String>) -> Self { Self::new(ErrorCode::InvalidArgs, msg) }
    pub fn tool_missing(msg: impl Into<String>) -> Self { Self::new(ErrorCode::ToolMissing, msg) }
    pub fn process(msg: impl Into<String>) -> Self { Self::new(ErrorCode::ProcessFailed, msg) }
}

pub type Result<T> = std::result::Result<T, AppError>;

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self { AppError::io(e.to_string()) }
}
