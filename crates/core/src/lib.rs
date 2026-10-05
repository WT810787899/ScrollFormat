pub mod error;
pub mod format;
pub mod task;
pub mod converter;
pub mod registry;
pub mod preset;
pub mod naming;

pub use error::{AppError, ErrorCode, Result};
pub use format::{detect_kind, Format, FormatKind, FormatInfo};
pub use task::{Task, TaskItem, TaskStatus, NewTask, ConvertOptions};
pub use converter::{Converter, ConvertContext, ConvertOutput, PreviewData};
pub use registry::ConverterRegistry;
pub use preset::{OutDirMode, Preset, CustomArg};
pub use naming::{NamingRule, ConflictPolicy, render_output_name};
