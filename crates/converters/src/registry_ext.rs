use std::sync::Arc;

use scroll_format_core::{ConverterRegistry, FormatKind};

use crate::{AudioConverter, ImageConverter, StubConverter, VideoConverter};

pub fn build_default_registry() -> ConverterRegistry {
    let mut reg = ConverterRegistry::new();
    reg.register(FormatKind::Image, Arc::new(ImageConverter));
    reg.register(FormatKind::Audio, Arc::new(AudioConverter));
    reg.register(FormatKind::Video, Arc::new(VideoConverter));
    reg.register(
        FormatKind::Document,
        Arc::new(StubConverter {
            id: "document",
            name: "文档转换器 (pandoc/libreoffice)",
            kind: FormatKind::Document,
            inputs: &["pdf", "docx", "doc", "odt", "rtf", "txt", "md", "html", "tex"],
            outputs: &["pdf", "docx", "odt", "rtf", "txt", "md", "html"],
        }),
    );
    reg.register(
        FormatKind::Ebook,
        Arc::new(StubConverter {
            id: "ebook",
            name: "电子书转换器 (calibre)",
            kind: FormatKind::Ebook,
            inputs: &["epub", "mobi", "azw3", "pdf", "txt", "html", "docx"],
            outputs: &["epub", "mobi", "azw3", "pdf", "txt", "html"],
        }),
    );
    reg.register(
        FormatKind::Data,
        Arc::new(StubConverter {
            id: "data",
            name: "数据转换器",
            kind: FormatKind::Data,
            inputs: &["json", "xml", "csv", "xlsx", "yaml", "yml", "toml", "sql", "parquet"],
            outputs: &["json", "xml", "csv", "xlsx", "yaml", "toml", "sql", "parquet"],
        }),
    );
    reg.register(
        FormatKind::Archive,
        Arc::new(StubConverter {
            id: "archive",
            name: "压缩包转换器 (7z)",
            kind: FormatKind::Archive,
            inputs: &["zip", "7z", "tar", "gz", "zst", "rar", "bz2", "xz"],
            outputs: &["zip", "7z", "tar", "gz", "zst"],
        }),
    );
    reg.register(
        FormatKind::Font,
        Arc::new(StubConverter {
            id: "font",
            name: "字体转换器",
            kind: FormatKind::Font,
            inputs: &["ttf", "otf", "woff", "woff2", "eot"],
            outputs: &["ttf", "otf", "woff", "woff2"],
        }),
    );
    reg
}
