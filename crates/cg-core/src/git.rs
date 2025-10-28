use crate::Result;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, PartialEq)]
pub enum FileChangeType {
    Added,
    Modified,
    Deleted,
    Renamed { old_path: PathBuf },
}

#[derive(Debug, Clone)]
pub struct FileChange {
    pub path: PathBuf,
    pub change_type: FileChangeType,
}

/// Get the current git commit hash for a repository
pub fn get_current_commit(repo_path: &Path) -> Result<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_path)
        .arg("rev-parse")
        .arg("HEAD")
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!("Failed to get git commit: {}", stderr));
    }

    let commit = String::from_utf8(output.stdout)?
        .trim()
        .to_string();

    Ok(commit)
}

/// Get list of changed files between two commits with their change status
pub fn get_file_changes(
    repo_path: &Path,
    from_commit: &str,
    to_commit: &str,
) -> Result<Vec<FileChange>> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_path)
        .arg("diff")
        .arg("--name-status")
        .arg(format!("{}..{}", from_commit, to_commit))
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!("Failed to get changed files: {}", stderr));
    }

    let mut changes = Vec::new();
    for line in String::from_utf8(output.stdout)?.lines() {
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() < 2 {
            continue;
        }

        let status = parts[0];
        
        // Helper to canonicalize paths to match what's stored in DB
        let canonicalize_path = |rel_path: &str| -> PathBuf {
            let full_path = repo_path.join(rel_path);
            // Try to canonicalize, but fall back to the full path if it fails
            // (e.g., for deleted files that no longer exist)
            std::fs::canonicalize(&full_path).unwrap_or(full_path)
        };
        
        let change = match status.chars().next() {
            Some('A') => FileChange {
                path: canonicalize_path(parts[1]),
                change_type: FileChangeType::Added,
            },
            Some('M') => FileChange {
                path: canonicalize_path(parts[1]),
                change_type: FileChangeType::Modified,
            },
            Some('D') => FileChange {
                path: canonicalize_path(parts[1]),
                change_type: FileChangeType::Deleted,
            },
            Some('R') if parts.len() >= 3 => {
                // Rename: R100 old_path new_path
                FileChange {
                    path: canonicalize_path(parts[2]),
                    change_type: FileChangeType::Renamed {
                        old_path: canonicalize_path(parts[1]),
                    },
                }
            }
            _ => continue, // Skip other statuses (C for copy, etc.)
        };

        changes.push(change);
    }

    Ok(changes)
}

/// Get list of changed files between two commits (simplified, no status)
pub fn get_changed_files(
    repo_path: &Path,
    from_commit: &str,
    to_commit: &str,
) -> Result<Vec<PathBuf>> {
    let changes = get_file_changes(repo_path, from_commit, to_commit)?;
    Ok(changes.into_iter().map(|c| c.path).collect())
}

/// Check if a path is in a git repository
pub fn is_git_repo(path: &Path) -> bool {
    Command::new("git")
        .arg("-C")
        .arg(path)
        .arg("rev-parse")
        .arg("--git-dir")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_get_current_commit() {
        // Test on this repository
        let repo_path = env::current_dir().unwrap();
        let repo_root = repo_path
            .ancestors()
            .find(|p| p.join(".git").exists())
            .unwrap();

        let result = get_current_commit(repo_root);
        assert!(result.is_ok(), "Should get current commit");
        
        let commit = result.unwrap();
        assert_eq!(commit.len(), 40, "Git commit hash should be 40 characters");
    }

    #[test]
    fn test_is_git_repo() {
        let repo_path = env::current_dir().unwrap();
        let repo_root = repo_path
            .ancestors()
            .find(|p| p.join(".git").exists())
            .unwrap();

        assert!(is_git_repo(repo_root), "Should be a git repo");
        
        let temp_dir = std::env::temp_dir();
        assert!(!is_git_repo(&temp_dir), "Temp dir should not be a git repo");
    }
}
