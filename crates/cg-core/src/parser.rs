use crate::model::{Edge, EdgeType, Node, NodeType};
use crate::Result;
use streaming_iterator::StreamingIterator;
use tree_sitter::{Parser, Query, QueryCursor, Node as TSNode};
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

    // Extract functions
    extract_functions(path, content, &tree, &mut nodes)?;
    
    // Extract classes
    extract_classes(path, content, &tree, &mut nodes)?;
    
    // Extract interfaces
    extract_interfaces(path, content, &tree, &mut nodes)?;
    
    // Extract imports
    extract_imports(path, content, &tree, &mut nodes)?;

    // Extract call edges between functions
    let functions: Vec<Node> = nodes.iter()
        .filter(|n| matches!(n.node_type, NodeType::Function))
        .cloned()
        .collect();
    let edges = extract_calls(path, content, &tree, &functions)?;

    debug!("Parsed {}: {} nodes, {} edges", path, nodes.len(), edges.len());

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
        // TODO: Full import bodies cause Kuzu parsing issues with nested quotes
        // Need to investigate proper Cypher string escaping or use a different storage method
        let node = Node::new(
            NodeType::Import,
            format!("{} imports", import_statements.len()),
            path.to_string(),
        );
        
        nodes.push(node);
    }

    Ok(())
}

fn extract_calls(
    _path: &str,
    content: &str,
    tree: &tree_sitter::Tree,
    functions: &[Node],
) -> Result<Vec<Edge>> {
    let query = Query::new(
        &tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        "(call_expression
            function: (identifier) @callee)",
    )?;

    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(&query, tree.root_node(), content.as_bytes());

    let mut edges = Vec::new();

    while let Some(match_) = matches.next() {
        if let Some(callee_capture) = match_.captures.first() {
            let callee_name = &content[callee_capture.node.byte_range()];
            let call_line = callee_capture.node.start_position().row;
            
            // Find the containing function
            if let Some(caller) = find_containing_function(call_line, functions) {
                // Find the callee function
                if let Some(callee) = functions.iter().find(|f| f.name == callee_name) {
                    edges.push(Edge {
                        from_id: caller.id.clone(),
                        to_id: callee.id.clone(),
                        edge_type: EdgeType::Calls,
                    });
                }
            }
        }
    }

    Ok(edges)
}

fn find_containing_function(line: usize, functions: &[Node]) -> Option<&Node> {
    functions.iter().find(|f| {
        if let (Some(start), Some(end)) = (f.start_line, f.end_line) {
            line >= start as usize && line <= end as usize
        } else {
            false
        }
    })
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
        // Import node name shows count
        assert_eq!(imports[0].name, "3 imports");

        Ok(())
    }

    #[test]
    fn test_extract_calls_edges() -> Result<()> {
        let content = r#"
function helper() {
    return "helper called";
}

function main() {
    const result = helper();
    return result;
}
        "#;

        let result = parse_typescript_file("test.ts", content)?;
        
        // Should have 2 functions
        let functions: Vec<_> = result.nodes.iter()
            .filter(|n| matches!(n.node_type, NodeType::Function))
            .collect();
        assert_eq!(functions.len(), 2);

        // Should have 1 Calls edge (main -> helper)
        let calls_edges: Vec<_> = result.edges.iter()
            .filter(|e| matches!(e.edge_type, EdgeType::Calls))
            .collect();
        assert_eq!(calls_edges.len(), 1);

        // Verify the edge connects main to helper
        let main_fn = functions.iter().find(|f| f.name == "main").unwrap();
        let helper_fn = functions.iter().find(|f| f.name == "helper").unwrap();
        
        assert_eq!(calls_edges[0].from_id, main_fn.id);
        assert_eq!(calls_edges[0].to_id, helper_fn.id);

        Ok(())
    }
}
