use crate::model::{Edge, SymbolNode};
use crate::Result;

pub struct ParsedFile {
    pub symbols: Vec<SymbolNode>,
    pub edges: Vec<Edge>,
}

pub fn parse_typescript_file(_path: &str, _content: &str) -> Result<ParsedFile> {
    // TODO: Use tree-sitter to extract symbols and relationships
    Ok(ParsedFile {
        symbols: Vec::new(),
        edges: Vec::new(),
    })
}
