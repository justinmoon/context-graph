use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileNode {
    pub id: String,
    pub path: String,
    pub hash: String,
    pub mtime: u64,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolNode {
    pub id: String,
    pub file_id: String,
    pub name: String,
    pub kind: SymbolKind,
    pub signature: Option<String>,
    pub start_line: u32,
    pub end_line: u32,
    pub export: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SymbolKind {
    Function,
    Class,
    Method,
    Interface,
    Enum,
    Variable,
    Import,
    Export,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    pub from: String,
    pub to: String,
    pub kind: EdgeKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EdgeKind {
    Contains,
    Calls,
    Imports,
}
