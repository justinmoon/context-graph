use crate::model::{Edge, Node};
use crate::Result;

pub struct ParsedFile {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
}

pub fn parse_typescript_file(_path: &str, _content: &str) -> Result<ParsedFile> {
    // TODO: Use tree-sitter to extract symbols and relationships
    Ok(ParsedFile {
        nodes: Vec::new(),
        edges: Vec::new(),
    })
}
