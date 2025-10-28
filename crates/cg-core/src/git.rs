use crate::Result;
use std::path::{Path, PathBuf};
use std::process::Command;

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

/// Get list of changed files between two commits
pub fn get_changed_files(
    repo_path: &Path,
    from_commit: &str,
    to_commit: &str,
) -> Result<Vec<PathBuf>> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_path)
        .arg("diff")
        .arg("--name-only")
        .arg(format!("{}..{}", from_commit, to_commit))
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!("Failed to get changed files: {}", stderr));
    }

    let files = String::from_utf8(output.stdout)?
        .lines()
        .map(|line| repo_path.join(line.trim()))
        .collect();

    Ok(files)
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
