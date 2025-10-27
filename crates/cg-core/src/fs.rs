use crate::Result;
use std::path::PathBuf;

/// Project root detection and file discovery
pub struct Project {
    pub root: PathBuf,
}

impl Project {
    pub fn discover(_path: &str) -> Result<Self> {
        // TODO: Detect git root or use cwd
        Ok(Self {
            root: PathBuf::new(),
        })
    }

    pub fn find_typescript_files(&self) -> Result<Vec<PathBuf>> {
        // TODO: Walk directory respecting .gitignore
        Ok(Vec::new())
    }
}
