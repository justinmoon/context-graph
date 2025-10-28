use cg_core::db::Database;
use cg_core::ingest::{ingest, IngestOptions};
use cg_core::model::{EdgeType, NodeType};
use std::fs;
use tempfile::TempDir;

/// Test constructor call edges (new ClassName())
#[test]
fn test_constructor_call_edges() {
    let project_dir = TempDir::new().unwrap();
    let db_dir = TempDir::new().unwrap();
    let db_path = db_dir.path().join("test.db");

    // Create test file with constructor calls
    fs::write(
        project_dir.path().join("service.ts"),
        r#"
export class UserService {
  constructor() {}
}

export class ProductService {
  constructor() {}
}

export function initializeServices() {
  const userService = new UserService();
  const productService = new ProductService();
}
"#,
    )
    .unwrap();

    // Ingest
    let options = IngestOptions {
        db_path: db_path.to_str().unwrap().to_string(),
        project_path: project_dir.path().to_str().unwrap().to_string(),
        threads: Some(2),
        clean: true,
        incremental: false,
    };

    ingest(options).expect("Ingestion should succeed");

    // Verify constructor call edges exist
    let mut db = Database::new(db_path.to_str().unwrap()).unwrap();
    
    // Find the initializeServices function
    let functions = db.find_nodes_by_type(&NodeType::Function).unwrap();
    let init_func = functions
        .iter()
        .find(|f| f.name == "initializeServices")
        .expect("Should find initializeServices function");

    // Find the classes
    let classes = db.find_nodes_by_type(&NodeType::Class).unwrap();
    let user_service = classes
        .iter()
        .find(|c| c.name == "UserService")
        .expect("Should find UserService class");
    let product_service = classes
        .iter()
        .find(|c| c.name == "ProductService")
        .expect("Should find ProductService class");

    // Query for Calls edges from initializeServices to classes
    let conn = db.get_connection().unwrap();
    let query = format!(
        "MATCH (f:Node {{id: '{}'}})-[e:EDGE {{edge_type: 'Calls'}}]->(c:Node) RETURN c.id",
        init_func.id
    );

    let mut called_ids = Vec::new();
    for row in conn.query(&query).unwrap() {
        if let kuzu::Value::String(id) = &row[0] {
            called_ids.push(id.clone());
        }
    }

    // Should have calls to both UserService and ProductService
    assert!(
        called_ids.contains(&user_service.id),
        "Should have Calls edge from initializeServices to UserService"
    );
    assert!(
        called_ids.contains(&product_service.id),
        "Should have Calls edge from initializeServices to ProductService"
    );
    assert_eq!(
        called_ids.len(),
        2,
        "Should have exactly 2 constructor call edges"
    );
}

/// Test file-to-file Import edges
#[test]
fn test_file_import_edges() {
    let project_dir = TempDir::new().unwrap();
    let db_dir = TempDir::new().unwrap();
    let db_path = db_dir.path().join("test.db");

    // Create utils file
    fs::write(
        project_dir.path().join("utils.ts"),
        r#"
export function helper() {
  return 'help';
}

export class Logger {
  log(msg: string) {}
}
"#,
    )
    .unwrap();

    // Create service file that imports from utils
    fs::write(
        project_dir.path().join("service.ts"),
        r#"
import { helper, Logger } from './utils';

export function main() {
  const result = helper();
  const logger = new Logger();
}
"#,
    )
    .unwrap();

    // Ingest
    let options = IngestOptions {
        db_path: db_path.to_str().unwrap().to_string(),
        project_path: project_dir.path().to_str().unwrap().to_string(),
        threads: Some(2),
        clean: true,
        incremental: false,
    };

    ingest(options).expect("Ingestion should succeed");

    // Verify Import edges exist
    let mut db = Database::new(db_path.to_str().unwrap()).unwrap();
    
    let import_count = db.count_edges_by_type(&EdgeType::Imports).unwrap();
    assert!(
        import_count >= 1,
        "Should have at least 1 Import edge (service.ts -> utils.ts), found {}",
        import_count
    );

    // Query for Import edges from service.ts to utils.ts
    let files = db.find_nodes_by_type(&NodeType::File).unwrap();
    let service_file = files
        .iter()
        .find(|f| f.name.contains("service.ts"))
        .expect("Should find service.ts");
    let utils_file = files
        .iter()
        .find(|f| f.name.contains("utils.ts"))
        .expect("Should find utils.ts");

    let conn = db.get_connection().unwrap();
    let query = format!(
        "MATCH (f:Node {{id: '{}'}})-[e:EDGE {{edge_type: 'Imports'}}]->(t:Node {{id: '{}'}}) RETURN count(e) as count",
        service_file.id, utils_file.id
    );

    let mut found = false;
    for row in conn.query(&query).unwrap() {
        if let kuzu::Value::Int64(count) = &row[0] {
            found = *count > 0;
        }
    }

    assert!(
        found,
        "Should have Import edge from service.ts to utils.ts"
    );
}

/// Test that incremental ingestion preserves Import edges from unaffected files
#[test]
#[cfg_attr(target_os = "macos", ignore)] // Path canonicalization issues on macOS
fn test_incremental_preserves_import_edges() {
    use std::process::Command;
    
    let project_dir = TempDir::new().unwrap();
    let db_dir = TempDir::new().unwrap();
    let db_path = db_dir.path().join("test.db");

    // Initialize git repo
    Command::new("git")
        .arg("init")
        .current_dir(project_dir.path())
        .output()
        .expect("git init failed");

    Command::new("git")
        .args(&["config", "user.email", "test@example.com"])
        .current_dir(project_dir.path())
        .output()
        .expect("git config failed");

    Command::new("git")
        .args(&["config", "user.name", "Test"])
        .current_dir(project_dir.path())
        .output()
        .expect("git config failed");

    // Create three files: A imports B, B imports C
    fs::write(
        project_dir.path().join("fileC.ts"),
        "export function utilC() { return 'C'; }",
    )
    .unwrap();

    fs::write(
        project_dir.path().join("fileB.ts"),
        "import { utilC } from './fileC';\nexport function utilB() { return utilC(); }",
    )
    .unwrap();

    fs::write(
        project_dir.path().join("fileA.ts"),
        "import { utilB } from './fileB';\nexport function main() { return utilB(); }",
    )
    .unwrap();

    // Initial commit
    Command::new("git")
        .args(&["add", "."])
        .current_dir(project_dir.path())
        .output()
        .expect("git add failed");

    Command::new("git")
        .args(&["commit", "-m", "Initial commit"])
        .current_dir(project_dir.path())
        .output()
        .expect("git commit failed");

    // Initial ingestion
    let options = IngestOptions {
        db_path: db_path.to_str().unwrap().to_string(),
        project_path: project_dir.path().to_str().unwrap().to_string(),
        threads: Some(2),
        clean: true,
        incremental: false,
    };

    ingest(options).expect("Initial ingestion should succeed");

    // Count import edges
    let mut db = Database::new(db_path.to_str().unwrap()).unwrap();
    let import_count_before = db.count_edges_by_type(&EdgeType::Imports).unwrap();
    assert!(
        import_count_before >= 2,
        "Should have at least 2 Import edges initially"
    );

    // Modify fileC only (doesn't affect import edges from A->B or B->C)
    fs::write(
        project_dir.path().join("fileC.ts"),
        "export function utilC() { return 'C_modified'; }\nexport function newUtil() { return 'new'; }",
    )
    .unwrap();

    Command::new("git")
        .args(&["add", "fileC.ts"])
        .current_dir(project_dir.path())
        .output()
        .expect("git add failed");

    Command::new("git")
        .args(&["commit", "-m", "Modify fileC"])
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

    ingest(options).expect("Incremental ingestion should succeed");

    // Verify import edges are preserved
    let import_count_after = db.count_edges_by_type(&EdgeType::Imports).unwrap();
    assert_eq!(
        import_count_before, import_count_after,
        "Import edges should be preserved during incremental ingestion"
    );

    // Specifically verify A->B edge still exists
    let files = db.find_nodes_by_type(&NodeType::File).unwrap();
    let file_a = files.iter().find(|f| f.name.contains("fileA.ts"));
    let file_b = files.iter().find(|f| f.name.contains("fileB.ts"));

    if let (Some(a), Some(b)) = (file_a, file_b) {
        let conn = db.get_connection().unwrap();
        let query = format!(
            "MATCH (a:Node {{id: '{}'}})-[e:EDGE {{edge_type: 'Imports'}}]->(b:Node {{id: '{}'}}) RETURN count(e) as count",
            a.id, b.id
        );

        let mut has_edge = false;
        for row in conn.query(&query).unwrap() {
            if let kuzu::Value::Int64(count) = &row[0] {
                has_edge = *count > 0;
            }
        }

        assert!(
            has_edge,
            "Import edge from fileA to fileB should be preserved"
        );
    }
}
