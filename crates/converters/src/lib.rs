pub mod image;
pub mod av;
pub mod stub;
pub mod registry_ext;

use std::path::{Path, PathBuf};

/// 生成不覆盖的输出路径：name.ext -> name (1).ext ...
pub fn unique_output_path(dir: &Path, stem: &str, ext: &str) -> PathBuf {
    let mut candidate = dir.join(format!("{stem}.{ext}"));
    let mut i = 1u32;
    while candidate.exists() {
        candidate = dir.join(format!("{stem} ({i}).{ext}"));
        i += 1;
    }
    candidate
}

pub use image::ImageConverter;
pub use av::{AudioConverter, VideoConverter};
pub use stub::StubConverter;
pub use registry_ext::build_default_registry;
