use crate::model::{Edge, EdgeType, Node, NodeType};
use crate::Result;
use streaming_iterator::StreamingIterator;
use tree_sitter::{Parser, Query, QueryCursor};
use tracing::debug;

pub struct ParsedFile {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
}

pub fn parse_typescript_file(path: &str, content: &str) -> Result<ParsedFile> {
    let mut parser = Parser::new();
    let language = tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into();
    parser.set_language(&language)?;

    let tree = parser.parse(content, None)
        .ok_or_else(|| anyhow::anyhow!("Failed to parse file"))?;

    let mut nodes = Vec::new();
    let edges = Vec::new();

    // Extract functions
    extract_functions(path, content, &tree, &mut nodes)?;
    
    // Extract classes
    extract_classes(path, content, &tree, &mut nodes)?;
    
    // Extract interfaces
    extract_interfaces(path, content, &tree, &mut nodes)?;
    
    // Extract imports
    extract_imports(path, content, &tree, &mut nodes)?;

    debug!("Parsed {}: {} nodes", path, nodes.len());

    Ok(ParsedFile { nodes, edges })
}

fn extract_functions(path: &str, content: &str, tree: &tree_sitter::Tree, nodes: &mut Vec<Node>) -> Result<()> {
    let query = Query::new(
        &tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        r#"
        (function_declaration
          name: (identifier) @name) @function

        (lexical_declaration
          (variable_declarator
            name: (identifier) @name
            value: [(arrow_function) (function_expression)])) @function
        "#,
    )?;

    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(&query, tree.root_node(), content.as_bytes());

    while let Some(match_) = matches.next() {
        let function_node = match_.captures.iter()
            .find(|c| query.capture_names()[c.index as usize] == "function")
            .map(|c| c.node);
        
        let name_node = match_.captures.iter()
            .find(|c| query.capture_names()[c.index as usize] == "name")
            .map(|c| c.node);

        if let (Some(func), Some(name)) = (function_node, name_node) {
            let func_name = &content[name.byte_range()];
            let body = &content[func.byte_range()];
            
            let node = Node::new(
                NodeType::Function,
                func_name.to_string(),
                path.to_string(),
            )
            .with_body(body.to_string())
            .with_lines(
                func.start_position().row as u32,
                func.end_position().row as u32,
            );

            nodes.push(node);
        }
    }

    Ok(())
}

fn extract_classes(path: &str, content: &str, tree: &tree_sitter::Tree, nodes: &mut Vec<Node>) -> Result<()> {
    let query = Query::new(
        &tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        r#"
        (class_declaration
          name: (type_identifier) @name) @class
        "#,
    )?;

    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(&query, tree.root_node(), content.as_bytes());

    while let Some(match_) = matches.next() {
        let class_node = match_.captures.iter()
            .find(|c| query.capture_names()[c.index as usize] == "class")
            .map(|c| c.node);
        
        let name_node = match_.captures.iter()
            .find(|c| query.capture_names()[c.index as usize] == "name")
            .map(|c| c.node);

        if let (Some(class), Some(name)) = (class_node, name_node) {
            let class_name = &content[name.byte_range()];
            let body = &content[class.byte_range()];
            
            let node = Node::new(
                NodeType::Class,
                class_name.to_string(),
                path.to_string(),
            )
            .with_body(body.to_string())
            .with_lines(
                class.start_position().row as u32,
                class.end_position().row as u32,
            );

            nodes.push(node);
        }
    }

    Ok(())
}

fn extract_interfaces(path: &str, content: &str, tree: &tree_sitter::Tree, nodes: &mut Vec<Node>) -> Result<()> {
    let query = Query::new(
        &tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        r#"
        (interface_declaration
          name: (type_identifier) @name) @interface
        "#,
    )?;

    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(&query, tree.root_node(), content.as_bytes());

    while let Some(match_) = matches.next() {
        let interface_node = match_.captures.iter()
            .find(|c| query.capture_names()[c.index as usize] == "interface")
            .map(|c| c.node);
        
        let name_node = match_.captures.iter()
            .find(|c| query.capture_names()[c.index as usize] == "name")
            .map(|c| c.node);

        if let (Some(interface), Some(name)) = (interface_node, name_node) {
            let interface_name = &content[name.byte_range()];
            let body = &content[interface.byte_range()];
            
            let node = Node::new(
                NodeType::Interface,
                interface_name.to_string(),
                path.to_string(),
            )
            .with_body(body.to_string())
            .with_lines(
                interface.start_position().row as u32,
                interface.end_position().row as u32,
            );

            nodes.push(node);
        }
    }

    Ok(())
}

fn extract_imports(path: &str, content: &str, tree: &tree_sitter::Tree, nodes: &mut Vec<Node>) -> Result<()> {
    let query = Query::new(
        &tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        r#"
        (import_statement) @import
        "#,
    )?;

    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(&query, tree.root_node(), content.as_bytes());

    let mut import_statements = Vec::new();
    while let Some(match_) = matches.next() {
        if let Some(import) = match_.captures.first() {
            let import_text = &content[import.node.byte_range()];
            import_statements.push(import_text);
        }
    }

    if !import_statements.is_empty() {
        let node = Node::new(
            NodeType::Import,
            "imports".to_string(),
            path.to_string(),
        ).with_body(import_statements.join("\n"));
        
        nodes.push(node);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_functions() -> Result<()> {
        let content = r#"
function myFunction() {
    console.log("hello");
}

const arrowFunc = () => {
    return 42;
};

async function asyncFunc() {
    return await fetch("/api");
}
        "#;

        let result = parse_typescript_file("test.ts", content)?;
        
        let functions: Vec<_> = result.nodes.iter()
            .filter(|n| matches!(n.node_type, NodeType::Function))
            .collect();
        
        assert_eq!(functions.len(), 3);
        
        let names: Vec<_> = functions.iter().map(|n| n.name.as_str()).collect();
        assert!(names.contains(&"myFunction"));
        assert!(names.contains(&"arrowFunc"));
        assert!(names.contains(&"asyncFunc"));

        Ok(())
    }

    #[test]
    fn test_parse_classes() -> Result<()> {
        let content = r#"
class MyClass {
    constructor() {}
    
    myMethod() {
        return "hello";
    }
}

export class ExportedClass {
    value: number = 0;
}
        "#;

        let result = parse_typescript_file("test.ts", content)?;
        
        let classes: Vec<_> = result.nodes.iter()
            .filter(|n| matches!(n.node_type, NodeType::Class))
            .collect();
        
        assert_eq!(classes.len(), 2);
        
        let names: Vec<_> = classes.iter().map(|n| n.name.as_str()).collect();
        assert!(names.contains(&"MyClass"));
        assert!(names.contains(&"ExportedClass"));

        Ok(())
    }

    #[test]
    fn test_parse_interfaces() -> Result<()> {
        let content = r#"
interface Person {
    name: string;
    age: number;
}

export interface User extends Person {
    id: number;
}
        "#;

        let result = parse_typescript_file("test.ts", content)?;
        
        let interfaces: Vec<_> = result.nodes.iter()
            .filter(|n| matches!(n.node_type, NodeType::Interface))
            .collect();
        
        assert_eq!(interfaces.len(), 2);
        
        let names: Vec<_> = interfaces.iter().map(|n| n.name.as_str()).collect();
        assert!(names.contains(&"Person"));
        assert!(names.contains(&"User"));

        Ok(())
    }

    #[test]
    fn test_parse_imports() -> Result<()> {
        let content = r#"
import { foo, bar } from "./module";
import * as React from "react";
import type { MyType } from "./types";
        "#;

        let result = parse_typescript_file("test.ts", content)?;
        
        let imports: Vec<_> = result.nodes.iter()
            .filter(|n| matches!(n.node_type, NodeType::Import))
            .collect();
        
        assert_eq!(imports.len(), 1);
        assert!(imports[0].body.contains("import { foo, bar }"));
        assert!(imports[0].body.contains("import * as React"));

        Ok(())
    }
}
