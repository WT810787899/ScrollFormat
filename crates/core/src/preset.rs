use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::format::FormatKind;
use crate::naming::NamingRule;
use crate::task::ConvertOptions;

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum OutDirMode {
    ProjectDefault,
    SourceDir,
    Custom,
}

impl Default for OutDirMode {
    fn default() -> Self {
        OutDirMode::ProjectDefault
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "snake_case")]
pub struct Preset {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub builtin: bool,
    #[serde(default)]
    pub kind: Option<FormatKind>,
    pub options: ConvertOptions,
    #[serde(default)]
    pub out_dir_mode: OutDirMode,
    #[serde(default)]
    pub out_dir: Option<String>,
    #[serde(default)]
    pub naming: Option<NamingRule>,
    #[serde(default)]
    pub custom_args: Vec<CustomArg>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Serialize, Deserialize, Clone, Default, Debug)]
#[serde(rename_all = "snake_case")]
pub struct CustomArg {
    pub key: String,
    pub value: String,
}
