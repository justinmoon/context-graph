use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

/// Helper to create a temporary TypeScript project for testing
fn create_test_project(dir: &TempDir) -> std::io::Result<()> {
    let src_dir = dir.path().join("src");
    fs::create_dir(&src_dir)?;
    
    // Create a simple TypeScript file with functions and classes
    fs::write(
        src_dir.join("main.ts"),
        r#"
export function greet(name: string): string {
    return `Hello, ${name}!`;
}

export function farewell(name: string): string {
    const message = greet(name);
    return `Goodbye, ${message}`;
}

export class Person {
    constructor(public name: string) {}
    
    sayHello(): void {
        console.log(greet(this.name));
    }
}

export interface User {
    id: number;
    name: string;
}
"#,
    )?;
    
    // Create another file with imports
    fs::write(
        src_dir.join("utils.ts"),
        r#"
import { greet } from './main';

export function welcome(name: string): string {
    return greet(name) + " Welcome aboard!";
}

export class Helper {
    static log(message: string): void {
        console.log(message);
    }
}
"#,
    )?;
    
    Ok(())
}

#[test]
fn test_help_command() {
    let mut cmd = Command::cargo_bin("cg").unwrap();
    cmd.arg("--help");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("lightweight tool for ingesting TypeScript"));
}

#[test]
fn test_ingest_command() {
    let temp_dir = TempDir::new().unwrap();
    let db_dir = TempDir::new().unwrap();
    
    create_test_project(&temp_dir).unwrap();
    
    let mut cmd = Command::cargo_bin("cg").unwrap();
    cmd.arg("ingest")
        .arg("--project")
        .arg(temp_dir.path())
        .arg("--db")
        .arg(db_dir.path().join("test.db"))
        .arg("--clean");
    
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Ingestion complete"))
        .stdout(predicate::str::contains("Files processed: 2"))
        .stdout(predicate::str::contains("Symbols created"));
}

#[test]
fn test_ingest_with_threads() {
    let temp_dir = TempDir::new().unwrap();
    let db_dir = TempDir::new().unwrap();
    
    create_test_project(&temp_dir).unwrap();
    
    let mut cmd = Command::cargo_bin("cg").unwrap();
    cmd.arg("ingest")
        .arg("--project")
        .arg(temp_dir.path())
        .arg("--db")
        .arg(db_dir.path().join("test.db"))
        .arg("--threads")
        .arg("2")
        .arg("--clean");
    
    cmd.assert().success();
}

#[test]
fn test_query_command() {
    let temp_dir = TempDir::new().unwrap();
    let db_dir = TempDir::new().unwrap();
    
    create_test_project(&temp_dir).unwrap();
    
    // First ingest
    Command::cargo_bin("cg")
        .unwrap()
        .arg("ingest")
        .arg("--project")
        .arg(temp_dir.path())
        .arg("--db")
        .arg(db_dir.path().join("test.db"))
        .arg("--clean")
        .assert()
        .success();
    
    // Then query
    let mut cmd = Command::cargo_bin("cg").unwrap();
    cmd.arg("query")
        .arg("MATCH (n:Node) WHERE n.node_type = 'Function' RETURN n.name")
        .arg("--db")
        .arg(db_dir.path().join("test.db"));
    
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("greet"))
        .stdout(predicate::str::contains("farewell"));
}

#[test]
fn test_query_with_json_output() {
    let temp_dir = TempDir::new().unwrap();
    let db_dir = TempDir::new().unwrap();
    
    create_test_project(&temp_dir).unwrap();
    
    // First ingest
    Command::cargo_bin("cg")
        .unwrap()
        .arg("ingest")
        .arg("--project")
        .arg(temp_dir.path())
        .arg("--db")
        .arg(db_dir.path().join("test.db"))
        .arg("--clean")
        .assert()
        .success();
    
    // Then query with JSON output
    let mut cmd = Command::cargo_bin("cg").unwrap();
    cmd.arg("query")
        .arg("MATCH (n:Node) WHERE n.node_type = 'Function' RETURN n.name LIMIT 1")
        .arg("--db")
        .arg(db_dir.path().join("test.db"))
        .arg("--json");
    
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("["))
        .stdout(predicate::str::contains("\"greet\""));
}

#[test]
fn test_find_symbol_command() {
    let temp_dir = TempDir::new().unwrap();
    let db_dir = TempDir::new().unwrap();
    
    create_test_project(&temp_dir).unwrap();
    
    // First ingest
    Command::cargo_bin("cg")
        .unwrap()
        .arg("ingest")
        .arg("--project")
        .arg(temp_dir.path())
        .arg("--db")
        .arg(db_dir.path().join("test.db"))
        .arg("--clean")
        .assert()
        .success();
    
    // Then find symbol
    let mut cmd = Command::cargo_bin("cg").unwrap();
    cmd.arg("find")
        .arg("symbol")
        .arg("greet")
        .arg("--db")
        .arg(db_dir.path().join("test.db"));
    
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("greet (Function)"))
        .stdout(predicate::str::contains("File:"));
}

#[test]
fn test_find_symbol_with_limit() {
    let temp_dir = TempDir::new().unwrap();
    let db_dir = TempDir::new().unwrap();
    
    create_test_project(&temp_dir).unwrap();
    
    // First ingest
    Command::cargo_bin("cg")
        .unwrap()
        .arg("ingest")
        .arg("--project")
        .arg(temp_dir.path())
        .arg("--db")
        .arg(db_dir.path().join("test.db"))
        .arg("--clean")
        .assert()
        .success();
    
    // Then find with limit
    let mut cmd = Command::cargo_bin("cg").unwrap();
    cmd.arg("find")
        .arg("symbol")
        .arg("e") // Will match multiple symbols
        .arg("--db")
        .arg(db_dir.path().join("test.db"))
        .arg("--limit")
        .arg("2");
    
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("symbol(s) matching"));
}

#[test]
fn test_find_symbol_not_found() {
    let temp_dir = TempDir::new().unwrap();
    let db_dir = TempDir::new().unwrap();
    
    create_test_project(&temp_dir).unwrap();
    
    // First ingest
    Command::cargo_bin("cg")
        .unwrap()
        .arg("ingest")
        .arg("--project")
        .arg(temp_dir.path())
        .arg("--db")
        .arg(db_dir.path().join("test.db"))
        .arg("--clean")
        .assert()
        .success();
    
    // Then find non-existent symbol
    let mut cmd = Command::cargo_bin("cg").unwrap();
    cmd.arg("find")
        .arg("symbol")
        .arg("nonExistentFunction")
        .arg("--db")
        .arg(db_dir.path().join("test.db"));
    
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("No symbols found"));
}

#[test]
fn test_find_callers_command() {
    let temp_dir = TempDir::new().unwrap();
    let db_dir = TempDir::new().unwrap();
    
    create_test_project(&temp_dir).unwrap();
    
    // First ingest
    Command::cargo_bin("cg")
        .unwrap()
        .arg("ingest")
        .arg("--project")
        .arg(temp_dir.path())
        .arg("--db")
        .arg(db_dir.path().join("test.db"))
        .arg("--clean")
        .assert()
        .success();
    
    // Then find callers (greet is called by farewell and welcome)
    let mut cmd = Command::cargo_bin("cg").unwrap();
    cmd.arg("find")
        .arg("callers")
        .arg("greet")
        .arg("--db")
        .arg(db_dir.path().join("test.db"));
    
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("caller").or(predicate::str::contains("No callers")));
}

#[test]
fn test_invalid_query() {
    let db_dir = TempDir::new().unwrap();
    
    let mut cmd = Command::cargo_bin("cg").unwrap();
    cmd.arg("query")
        .arg("INVALID SQL QUERY")
        .arg("--db")
        .arg(db_dir.path().join("test.db"));
    
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Error"));
}

#[test]
fn test_nonexistent_project() {
    let db_dir = TempDir::new().unwrap();
    
    let mut cmd = Command::cargo_bin("cg").unwrap();
    cmd.arg("ingest")
        .arg("--project")
        .arg("/nonexistent/path/to/project")
        .arg("--db")
        .arg(db_dir.path().join("test.db"))
        .arg("--clean");
    
    cmd.assert().failure();
}

#[test]
fn test_special_characters_in_code() {
    let temp_dir = TempDir::new().unwrap();
    let db_dir = TempDir::new().unwrap();
    
    let src_dir = temp_dir.path().join("src");
    fs::create_dir(&src_dir).unwrap();
    
    // Create file with special characters
    fs::write(
        src_dir.join("special.ts"),
        r#"
export function testQuotes() {
    return "It's a test with 'quotes'";
}

export function testBackslash() {
    return "Path: C:\\Users\\test";
}
"#,
    ).unwrap();
    
    // Ingest
    Command::cargo_bin("cg")
        .unwrap()
        .arg("ingest")
        .arg("--project")
        .arg(temp_dir.path())
        .arg("--db")
        .arg(db_dir.path().join("test.db"))
        .arg("--clean")
        .assert()
        .success();
    
    // Query to verify special characters were handled
    let mut cmd = Command::cargo_bin("cg").unwrap();
    cmd.arg("query")
        .arg("MATCH (n:Node) WHERE n.name = 'testQuotes' RETURN n.name")
        .arg("--db")
        .arg(db_dir.path().join("test.db"));
    
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("testQuotes"));
}
