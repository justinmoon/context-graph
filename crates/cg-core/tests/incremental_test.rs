use cg_core::db::Database;
use cg_core::ingest::{ingest, IngestOptions};
use cg_core::model::NodeType;
use std::fs;
use std::process::Command;
use tempfile::TempDir;

/// Helper to create a test git repository with TypeScript files
fn create_test_git_repo(dir: &TempDir) -> std::io::Result<()> {
    let repo_path = dir.path();
    
    // Initialize git repo
    Command::new("git")
        .arg("init")
        .current_dir(repo_path)
        .output()
        .expect("git init failed");

    // Configure git
    Command::new("git")
        .args(&["config", "user.email", "test@example.com"])
        .current_dir(repo_path)
        .output()
        .expect("git config email failed");

    Command::new("git")
        .args(&["config", "user.name", "Test User"])
        .current_dir(repo_path)
        .output()
        .expect("git config name failed");

    // Create initial TypeScript file
    fs::write(
        repo_path.join("file1.ts"),
        "export function hello() { return 'world'; }",
    )?;

    // Initial commit
    Command::new("git")
        .args(&["add", "."])
        .current_dir(repo_path)
        .output()
        .expect("git add failed");

    Command::new("git")
        .args(&["commit", "-m", "Initial commit"])
        .current_dir(repo_path)
        .output()
        .expect("git commit failed");

    Ok(())
}

#[test]
fn test_first_ingest_stores_commit() {
    let project_dir = TempDir::new().unwrap();
    let db_dir = TempDir::new().unwrap();
    let db_path = db_dir.path().join("test.db");

    create_test_git_repo(&project_dir).unwrap();

    // First ingestion
    let options = IngestOptions {
        db_path: db_path.to_str().unwrap().to_string(),
        project_path: project_dir.path().to_str().unwrap().to_string(),
        threads: Some(2),
        clean: true,
        incremental: false,
    };

    let stats = ingest(options).expect("Ingestion should succeed");
    assert_eq!(stats.files_processed, 1, "Should process 1 file");

    // Check that commit hash was stored
    let mut db = Database::new(db_path.to_str().unwrap()).unwrap();
    let commit = db.get_metadata("last_commit").unwrap();
    assert!(commit.is_some(), "Commit hash should be stored");
    assert_eq!(commit.unwrap().len(), 40, "Commit hash should be 40 chars");
}

#[test]
fn test_incremental_with_no_changes() {
    let project_dir = TempDir::new().unwrap();
    let db_dir = TempDir::new().unwrap();
    let db_path = db_dir.path().join("test.db");

    create_test_git_repo(&project_dir).unwrap();

    // First ingestion
    let options = IngestOptions {
        db_path: db_path.to_str().unwrap().to_string(),
        project_path: project_dir.path().to_str().unwrap().to_string(),
        threads: Some(2),
        clean: true,
        incremental: false,
    };
    ingest(options).expect("First ingestion should succeed");

    // Second ingestion with incremental mode (no changes)
    let options = IngestOptions {
        db_path: db_path.to_str().unwrap().to_string(),
        project_path: project_dir.path().to_str().unwrap().to_string(),
        threads: Some(2),
        clean: false,
        incremental: true,
    };

    let stats = ingest(options).expect("Incremental ingestion should succeed");
    assert_eq!(stats.files_processed, 0, "Should process 0 files (no changes)");
    assert_eq!(stats.symbols_created, 0, "Should create 0 symbols (no changes)");
}

#[test]
fn test_incremental_with_modified_file() {
    let project_dir = TempDir::new().unwrap();
    let db_dir = TempDir::new().unwrap();
    let db_path = db_dir.path().join("test.db");

    create_test_git_repo(&project_dir).unwrap();

    // First ingestion
    let options = IngestOptions {
        db_path: db_path.to_str().unwrap().to_string(),
        project_path: project_dir.path().to_str().unwrap().to_string(),
        threads: Some(2),
        clean: true,
        incremental: false,
    };
    ingest(options).expect("First ingestion should succeed");

    // Modify the file
    fs::write(
        project_dir.path().join("file1.ts"),
        "export function hello() { return 'world'; }\nexport function goodbye() { return 'bye'; }",
    ).unwrap();

    // Commit the change
    Command::new("git")
        .args(&["add", "file1.ts"])
        .current_dir(project_dir.path())
        .output()
        .expect("git add failed");

    Command::new("git")
        .args(&["commit", "-m", "Modify file1"])
        .current_dir(project_dir.path())
        .output()
        .expect("git commit failed");

    // Incremental ingestion
    let options = IngestOptions {
        db_path: db_path.to_str().unwrap().to_string(),
        project_path: project_dir.path().to_str().unwrap().to_string(),
        threads: Some(2),
        clean: false,
        incremental: true,
    };

    let stats = ingest(options).expect("Incremental ingestion should succeed");
    assert_eq!(stats.files_processed, 1, "Should reprocess the modified file");
    assert!(stats.symbols_created >= 2, "Should extract symbols from modified file");

    // Verify the new function was added
    let mut db = Database::new(db_path.to_str().unwrap()).unwrap();
    let functions = db.find_nodes_by_type(&NodeType::Function).unwrap();
    let goodbye = functions.iter().find(|f| f.name == "goodbye");
    assert!(goodbye.is_some(), "Should find the new 'goodbye' function");
}

#[test]
#[ignore] // FIXME: Path canonicalization issues on macOS (/tmp -> /private/tmp)
fn test_incremental_with_deleted_file() {
    let project_dir = TempDir::new().unwrap();
    let db_dir = TempDir::new().unwrap();
    let db_path = db_dir.path().join("test.db");

    create_test_git_repo(&project_dir).unwrap();

    // Add another file
    fs::write(
        project_dir.path().join("file2.ts"),
        "export function foo() { return 42; }",
    ).unwrap();

    Command::new("git")
        .args(&["add", "file2.ts"])
        .current_dir(project_dir.path())
        .output()
        .expect("git add failed");

    Command::new("git")
        .args(&["commit", "-m", "Add file2"])
        .current_dir(project_dir.path())
        .output()
        .expect("git commit failed");

    // First ingestion
    let options = IngestOptions {
        db_path: db_path.to_str().unwrap().to_string(),
        project_path: project_dir.path().to_str().unwrap().to_string(),
        threads: Some(2),
        clean: true,
        incremental: false,
    };
    ingest(options).expect("First ingestion should succeed");

    // Verify file2 is in the database
    let mut db = Database::new(db_path.to_str().unwrap()).unwrap();
    let files_before = db.find_nodes_by_type(&NodeType::File).unwrap();
    let file2_before = files_before.iter().find(|f| f.name.contains("file2.ts"));
    assert!(file2_before.is_some(), "file2.ts should be in database");

    // Delete file2
    fs::remove_file(project_dir.path().join("file2.ts")).unwrap();

    Command::new("git")
        .args(&["add", "-A"])
        .current_dir(project_dir.path())
        .output()
        .expect("git add failed");

    Command::new("git")
        .args(&["commit", "-m", "Delete file2"])
        .current_dir(project_dir.path())
        .output()
        .expect("git commit failed");

    // Incremental ingestion
    let options = IngestOptions {
        db_path: db_path.to_str().unwrap().to_string(),
        project_path: project_dir.path().to_str().unwrap().to_string(),
        threads: Some(2),
        clean: false,
        incremental: true,
    };

    let stats = ingest(options).expect("Incremental ingestion should succeed");
    assert_eq!(stats.files_processed, 0, "Should not process any files");

    // Verify file2 is no longer in the database
    let files_after = db.find_nodes_by_type(&NodeType::File).unwrap();
    println!("Files after deletion:");
    for f in &files_after {
        println!("  {}", f.name);
    }
    let file2_after = files_after.iter().find(|f| f.name.contains("file2.ts"));
    assert!(file2_after.is_none(), "file2.ts should be removed from database");
}

#[test]
#[ignore] // FIXME: Path canonicalization issues on macOS (/tmp -> /private/tmp)
fn test_incremental_with_renamed_file() {
    let project_dir = TempDir::new().unwrap();
    let db_dir = TempDir::new().unwrap();
    let db_path = db_dir.path().join("test.db");

    create_test_git_repo(&project_dir).unwrap();

    // First ingestion
    let options = IngestOptions {
        db_path: db_path.to_str().unwrap().to_string(),
        project_path: project_dir.path().to_str().unwrap().to_string(),
        threads: Some(2),
        clean: true,
        incremental: false,
    };
    ingest(options).expect("First ingestion should succeed");

    // Verify file1.ts is in the database
    let mut db = Database::new(db_path.to_str().unwrap()).unwrap();
    let files_before = db.find_nodes_by_type(&NodeType::File).unwrap();
    assert_eq!(files_before.len(), 1, "Should have 1 file");

    // Rename file1.ts to renamed.ts
    Command::new("git")
        .args(&["mv", "file1.ts", "renamed.ts"])
        .current_dir(project_dir.path())
        .output()
        .expect("git mv failed");

    Command::new("git")
        .args(&["commit", "-m", "Rename file1 to renamed"])
        .current_dir(project_dir.path())
        .output()
        .expect("git commit failed");

    // Incremental ingestion
    let options = IngestOptions {
        db_path: db_path.to_str().unwrap().to_string(),
        project_path: project_dir.path().to_str().unwrap().to_string(),
        threads: Some(2),
        clean: false,
        incremental: true,
    };

    let stats = ingest(options).expect("Incremental ingestion should succeed");
    assert_eq!(stats.files_processed, 1, "Should process the renamed file");

    // Verify old file is gone and new file exists
    let files_after = db.find_nodes_by_type(&NodeType::File).unwrap();
    assert_eq!(files_after.len(), 1, "Should still have 1 file");
    
    let old_file = files_after.iter().find(|f| f.name.contains("file1.ts"));
    assert!(old_file.is_none(), "Old filename should be removed");

    let new_file = files_after.iter().find(|f| f.name.contains("renamed.ts"));
    assert!(new_file.is_some(), "New filename should exist");
}

#[test]
fn test_fallback_on_unreachable_commit() {
    let project_dir = TempDir::new().unwrap();
    let db_dir = TempDir::new().unwrap();
    let db_path = db_dir.path().join("test.db");

    create_test_git_repo(&project_dir).unwrap();

    // First ingestion
    let options = IngestOptions {
        db_path: db_path.to_str().unwrap().to_string(),
        project_path: project_dir.path().to_str().unwrap().to_string(),
        threads: Some(2),
        clean: true,
        incremental: false,
    };
    ingest(options).expect("First ingestion should succeed");

    // Manually set an invalid commit hash
    let mut db = Database::new(db_path.to_str().unwrap()).unwrap();
    db.set_metadata("last_commit", "0000000000000000000000000000000000000000").unwrap();

    // Incremental ingestion should fall back to full ingestion
    let options = IngestOptions {
        db_path: db_path.to_str().unwrap().to_string(),
        project_path: project_dir.path().to_str().unwrap().to_string(),
        threads: Some(2),
        clean: false,
        incremental: true,
    };

    let stats = ingest(options).expect("Should fallback to full ingestion");
    assert_eq!(stats.files_processed, 1, "Should process all files (fallback)");
}
