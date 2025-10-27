use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum NodeType {
    Repository,
    Language,
    File,
    Directory,
    Function,
    Class,
    Interface,
    DataModel,
    Trait,
    Var,
    Import,
    Library,
    Endpoint,
    Request,
    Page,
    Instance,
}

impl NodeType {
    pub fn as_str(&self) -> &'static str {
        match self {
            NodeType::Repository => "Repository",
            NodeType::Language => "Language",
            NodeType::File => "File",
            NodeType::Directory => "Directory",
            NodeType::Function => "Function",
            NodeType::Class => "Class",
            NodeType::Interface => "Interface",
            NodeType::DataModel => "DataModel",
            NodeType::Trait => "Trait",
            NodeType::Var => "Var",
            NodeType::Import => "Import",
            NodeType::Library => "Library",
            NodeType::Endpoint => "Endpoint",
            NodeType::Request => "Request",
            NodeType::Page => "Page",
            NodeType::Instance => "Instance",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum EdgeType {
    Contains,
    Calls,
    Imports,
    Handler,
    Renders,
    Implements,
    Uses,
    Of,
    Operand,
}

impl EdgeType {
    pub fn as_str(&self) -> &'static str {
        match self {
            EdgeType::Contains => "Contains",
            EdgeType::Calls => "Calls",
            EdgeType::Imports => "Imports",
            EdgeType::Handler => "Handler",
            EdgeType::Renders => "Renders",
            EdgeType::Implements => "Implements",
            EdgeType::Uses => "Uses",
            EdgeType::Of => "Of",
            EdgeType::Operand => "Operand",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub id: String,
    pub node_type: NodeType,
    pub name: String,
    pub file: String,
    pub body: String,
    pub start_line: Option<u32>,
    pub end_line: Option<u32>,
    pub meta: HashMap<String, String>,
}

impl Node {
    pub fn new(node_type: NodeType, name: String, file: String) -> Self {
        let id = Self::generate_id(&node_type, &name, &file, None, None);
        Self {
            id,
            node_type,
            name,
            file,
            body: String::new(),
            start_line: None,
            end_line: None,
            meta: HashMap::new(),
        }
    }

    pub fn with_body(mut self, body: String) -> Self {
        self.body = body;
        self
    }

    pub fn with_lines(mut self, start_line: u32, end_line: u32) -> Self {
        self.start_line = Some(start_line);
        self.end_line = Some(end_line);
        self.id = Self::generate_id(&self.node_type, &self.name, &self.file, Some(start_line), Some(end_line));
        self
    }

    pub fn with_meta(mut self, key: String, value: String) -> Self {
        self.meta.insert(key, value);
        self
    }

    fn generate_id(node_type: &NodeType, name: &str, file: &str, start_line: Option<u32>, end_line: Option<u32>) -> String {
        let mut hasher = blake3::Hasher::new();
        hasher.update(node_type.as_str().as_bytes());
        hasher.update(name.as_bytes());
        hasher.update(file.as_bytes());
        if let Some(sl) = start_line {
            hasher.update(&sl.to_le_bytes());
        }
        if let Some(el) = end_line {
            hasher.update(&el.to_le_bytes());
        }
        hasher.finalize().to_hex().to_string()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    pub from_id: String,
    pub to_id: String,
    pub edge_type: EdgeType,
}
