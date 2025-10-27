use crate::Result;
use ignore::WalkBuilder;
use std::path::{Path, PathBuf};
use tracing::{debug, info};

pub struct Project {
    pub root: PathBuf,
}

impl Project {
    pub fn discover(path: &str) -> Result<Self> {
        let path = std::fs::canonicalize(path)?;
        let root = Self::find_git_root(&path).unwrap_or_else(|| path.clone());
        info!("Project root: {}", root.display());
        Ok(Self { root })
    }

    fn find_git_root(path: &Path) -> Option<PathBuf> {
        let mut current = path;
        loop {
            if current.join(".git").exists() {
                return Some(current.to_path_buf());
            }
            current = current.parent()?;
        }
    }

    pub fn find_typescript_files(&self) -> Result<Vec<PathBuf>> {
        info!("Discovering TypeScript files in {}", self.root.display());
        
        let mut files = Vec::new();
        
        for result in WalkBuilder::new(&self.root)
            .standard_filters(true)
            .build()
        {
            match result {
                Ok(entry) => {
                    let path = entry.path();
                    if let Some(ext) = path.extension() {
                        if ext == "ts" || ext == "tsx" {
                            if path.is_file() {
                                debug!("Found TypeScript file: {}", path.display());
                                files.push(path.to_path_buf());
                            }
                        }
                    }
                }
                Err(e) => debug!("Error walking directory: {}", e),
            }
        }
        
        info!("Found {} TypeScript files", files.len());
        Ok(files)
    }

    pub fn read_file(&self, path: &Path) -> Result<String> {
        Ok(std::fs::read_to_string(path)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_find_typescript_files() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let root = temp_dir.path();

        fs::write(root.join("test1.ts"), "const x = 1;")?;
        fs::write(root.join("test2.tsx"), "export const Y = 2;")?;
        fs::write(root.join("ignore.js"), "console.log('ignored');")?;
        
        fs::create_dir(root.join("src"))?;
        fs::write(root.join("src/nested.ts"), "const z = 3;")?;

        let project = Project {
            root: root.to_path_buf(),
        };
        
        let files = project.find_typescript_files()?;
        assert_eq!(files.len(), 3);
        
        let filenames: Vec<_> = files.iter()
            .filter_map(|p| p.file_name().and_then(|n| n.to_str()))
            .collect();
        
        assert!(filenames.contains(&"test1.ts"));
        assert!(filenames.contains(&"test2.tsx"));
        assert!(filenames.contains(&"nested.ts"));
        assert!(!filenames.contains(&"ignore.js"));

        Ok(())
    }

    #[test]
    fn test_find_git_root() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let root = temp_dir.path();
        
        fs::create_dir(root.join(".git"))?;
        fs::create_dir_all(root.join("src/nested"))?;

        let git_root = Project::find_git_root(&root.join("src/nested"));
        assert!(git_root.is_some());
        assert_eq!(git_root.unwrap(), root);

        Ok(())
    }
}
