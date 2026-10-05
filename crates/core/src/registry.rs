use std::collections::HashMap;
use std::sync::Arc;

use crate::converter::Converter;
use crate::format::detect_kind;
use crate::format::FormatKind;

#[derive(Default, Clone)]
pub struct ConverterRegistry {
    by_kind: HashMap<FormatKind, Vec<Arc<dyn Converter>>>,
}

impl ConverterRegistry {
    pub fn new() -> Self { Self::default() }
    pub fn register(&mut self, kind: FormatKind, c: Arc<dyn Converter>) {
        self.by_kind.entry(kind).or_default().push(c);
    }
    pub fn for_kind(&self, kind: FormatKind) -> Vec<Arc<dyn Converter>> {
        self.by_kind.get(&kind).cloned().unwrap_or_default()
    }
    pub fn pick(&self, input: &std::path::Path) -> Option<Arc<dyn Converter>> {
        let kind = detect_kind(input)?;
        self.by_kind.get(&kind)?.first().cloned()
    }
    pub fn all(&self) -> Vec<Arc<dyn Converter>> {
        self.by_kind.values().flatten().cloned().collect()
    }
}
