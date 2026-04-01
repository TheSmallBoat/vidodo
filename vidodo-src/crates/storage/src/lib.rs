use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactLayout {
    pub root: PathBuf,
    pub traces: PathBuf,
    pub exports: PathBuf,
}

impl ArtifactLayout {
    pub fn new(root: impl AsRef<Path>) -> Self {
        let root = root.as_ref().to_path_buf();
        Self { traces: root.join("traces"), exports: root.join("exports"), root }
    }
}

#[cfg(test)]
mod tests {
    use super::ArtifactLayout;

    #[test]
    fn builds_expected_subdirectories() {
        let layout = ArtifactLayout::new("artifacts");

        assert!(layout.traces.ends_with("artifacts/traces"));
        assert!(layout.exports.ends_with("artifacts/exports"));
    }
}
