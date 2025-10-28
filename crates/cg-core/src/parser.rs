use crate::model::{Edge, EdgeType, Node, NodeType};
use crate::Result;
use streaming_iterator::StreamingIterator;
use tree_sitter::{Parser, Query, QueryCursor};
use tracing::debug;

pub struct ParsedFile {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub import_edges: Vec<(String, String)>, // (from_file, to_file) pairs
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

    // Extract call edges between functions (within this file)
    let functions: Vec<Node> = nodes.iter()
        .filter(|n| matches!(n.node_type, NodeType::Function))
        .cloned()
        .collect();
    let mut edges = extract_calls(path, content, &tree, &functions)?;

    // Extract constructor call edges (new ClassName())
    let classes: Vec<Node> = nodes.iter()
        .filter(|n| matches!(n.node_type, NodeType::Class))
        .cloned()
        .collect();
    edges.extend(extract_constructor_calls(path, content, &tree, &functions, &classes)?);

    // Extract extends and implements edges for classes and interfaces
    let classes_and_interfaces: Vec<Node> = nodes.iter()
        .filter(|n| matches!(n.node_type, NodeType::Class | NodeType::Interface))
        .cloned()
        .collect();
    edges.extend(extract_extends_implements(path, content, &tree, &classes_and_interfaces)?);

    // Extract file-to-file import edges
    let import_edges = extract_import_edges(path, content, &tree)?;

    debug!("Parsed {}: {} nodes, {} edges, {} import edges", path, nodes.len(), edges.len(), import_edges.len());

    Ok(ParsedFile { nodes, edges, import_edges })
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
        (import_statement
            source: (string
                (string_fragment) @source_path)) @import
        "#,
    )?;

    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(&query, tree.root_node(), content.as_bytes());

    let mut import_paths = Vec::new();
    while let Some(match_) = matches.next() {
        if let Some(source_path_capture) = match_.captures.iter()
            .find(|c| query.capture_names()[c.index as usize] == "source_path")
        {
            let source_path = &content[source_path_capture.node.byte_range()];
            import_paths.push(source_path.to_string());
        }
    }

    if !import_paths.is_empty() {
        // Store import metadata node for legacy compatibility
        let node = Node::new(
            NodeType::Import,
            format!("{} imports", import_paths.len()),
            path.to_string(),
        );
        
        nodes.push(node);
    }

    Ok(())
}

fn extract_import_edges(
    path: &str,
    content: &str,
    tree: &tree_sitter::Tree,
) -> Result<Vec<(String, String)>> {
    // Returns vec of (from_file, to_file) tuples
    let query = Query::new(
        &tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        r#"
        (import_statement
            source: (string
                (string_fragment) @source_path))
        "#,
    )?;

    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(&query, tree.root_node(), content.as_bytes());

    let mut import_paths = Vec::new();
    while let Some(match_) = matches.next() {
        if let Some(source_path_capture) = match_.captures.iter()
            .find(|c| query.capture_names()[c.index as usize] == "source_path")
        {
            let source_path = &content[source_path_capture.node.byte_range()];
            let resolved_path = resolve_import_path(path, source_path)?;
            if let Some(resolved) = resolved_path {
                import_paths.push((path.to_string(), resolved));
            }
        }
    }

    Ok(import_paths)
}

fn resolve_import_path(from_file: &str, import_path: &str) -> Result<Option<String>> {
    use std::path::{Path, PathBuf};
    
    // Skip node_modules and external packages
    if !import_path.starts_with('.') {
        return Ok(None);
    }
    
    let from_dir = Path::new(from_file).parent().unwrap_or(Path::new(""));
    let mut target_path = from_dir.join(import_path);
    
    // Try different extensions
    let extensions = ["", ".ts", ".tsx", "/index.ts", "/index.tsx"];
    for ext in &extensions {
        let mut candidate = target_path.clone();
        if !ext.is_empty() {
            candidate = PathBuf::from(format!("{}{}", target_path.display(), ext));
        }
        
        if let Ok(canonical) = std::fs::canonicalize(&candidate) {
            return Ok(Some(canonical.display().to_string()));
        }
    }
    
    Ok(None)
}

fn extract_calls(
    _path: &str,
    content: &str,
    tree: &tree_sitter::Tree,
    functions: &[Node],
) -> Result<Vec<Edge>> {
    // Query for both simple identifier calls and member expression calls
    let query = Query::new(
        &tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        r#"
        ; Simple identifier calls: foo()
        (call_expression
            function: (identifier) @callee)
        
        ; Member expression calls: obj.method(), console.log()
        (call_expression
            function: (member_expression
                property: (property_identifier) @method_name))
        "#,
    )?;

    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(&query, tree.root_node(), content.as_bytes());

    let mut edges = Vec::new();

    while let Some(match_) = matches.next() {
        // Try to find the callee capture (for simple identifier calls)
        let callee_name_opt = match_.captures.iter()
            .find(|c| query.capture_names()[c.index as usize] == "callee")
            .map(|c| &content[c.node.byte_range()]);
        
        // Try to find the method_name capture (for member expression calls)
        let method_name_opt = match_.captures.iter()
            .find(|c| query.capture_names()[c.index as usize] == "method_name")
            .map(|c| &content[c.node.byte_range()]);
        
        let callee_name = callee_name_opt.or(method_name_opt);
        
        if let Some(name) = callee_name {
            let call_line = match_.captures.first().unwrap().node.start_position().row;
            
            // Find the containing function
            if let Some(caller) = find_containing_function(call_line, functions) {
                // Find the callee function by name
                if let Some(callee) = functions.iter().find(|f| f.name == name) {
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

fn extract_constructor_calls(
    _path: &str,
    content: &str,
    tree: &tree_sitter::Tree,
    functions: &[Node],
    classes: &[Node],
) -> Result<Vec<Edge>> {
    // Query for new expressions: new ClassName()
    let query = Query::new(
        &tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        r#"
        (new_expression
            constructor: [
                (identifier) @class_name
                (member_expression
                    property: (property_identifier) @nested_class)
            ])
        "#,
    )?;

    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(&query, tree.root_node(), content.as_bytes());

    let mut edges = Vec::new();

    while let Some(match_) = matches.next() {
        // Try to find the class name capture
        let class_name_opt = match_.captures.iter()
            .find(|c| query.capture_names()[c.index as usize] == "class_name")
            .map(|c| &content[c.node.byte_range()]);
        
        // Try to find nested class (e.g., Foo.Bar in new Foo.Bar())
        let nested_class_opt = match_.captures.iter()
            .find(|c| query.capture_names()[c.index as usize] == "nested_class")
            .map(|c| &content[c.node.byte_range()]);
        
        let class_name = class_name_opt.or(nested_class_opt);
        
        if let Some(name) = class_name {
            let call_line = match_.captures.first().unwrap().node.start_position().row;
            
            // Find the containing function where the constructor is called
            if let Some(caller) = find_containing_function(call_line, functions) {
                // Find the class being instantiated
                if let Some(class) = classes.iter().find(|c| c.name == name) {
                    edges.push(Edge {
                        from_id: caller.id.clone(),
                        to_id: class.id.clone(),
                        edge_type: EdgeType::Calls,
                    });
                }
            }
        }
    }

    Ok(edges)
}

fn extract_extends_implements(
    _path: &str,
    content: &str,
    tree: &tree_sitter::Tree,
    classes_and_interfaces: &[Node],
) -> Result<Vec<Edge>> {
    // Query for class extends and implements clauses
    let query = Query::new(
        &tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        r#"
        ; Class extends
        (class_declaration
            name: (type_identifier) @class_name
            (class_heritage
                (extends_clause
                    value: (identifier) @extends_target)))
        
        ; Class implements
        (class_declaration
            name: (type_identifier) @class_name_impl
            (class_heritage
                (implements_clause
                    (type_identifier) @implements_target)))
        
        ; Interface extends
        (interface_declaration
            name: (type_identifier) @interface_name
            (extends_type_clause
                (type_identifier) @interface_extends_target))
        "#,
    )?;

    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(&query, tree.root_node(), content.as_bytes());

    let mut edges = Vec::new();

    while let Some(match_) = matches.next() {
        // Try to find class extends
        let class_name = match_.captures.iter()
            .find(|c| query.capture_names()[c.index as usize] == "class_name")
            .map(|c| &content[c.node.byte_range()]);
        let extends_target = match_.captures.iter()
            .find(|c| query.capture_names()[c.index as usize] == "extends_target")
            .map(|c| &content[c.node.byte_range()]);
        
        if let (Some(class), Some(target)) = (class_name, extends_target) {
            if let (Some(from_node), Some(to_node)) = (
                classes_and_interfaces.iter().find(|n| n.name == class),
                classes_and_interfaces.iter().find(|n| n.name == target)
            ) {
                edges.push(Edge {
                    from_id: from_node.id.clone(),
                    to_id: to_node.id.clone(),
                    edge_type: EdgeType::Implements, // Using Implements for extends as well
                });
            }
        }

        // Try to find class implements
        let class_name_impl = match_.captures.iter()
            .find(|c| query.capture_names()[c.index as usize] == "class_name_impl")
            .map(|c| &content[c.node.byte_range()]);
        let implements_target = match_.captures.iter()
            .find(|c| query.capture_names()[c.index as usize] == "implements_target")
            .map(|c| &content[c.node.byte_range()]);
        
        if let (Some(class), Some(target)) = (class_name_impl, implements_target) {
            if let (Some(from_node), Some(to_node)) = (
                classes_and_interfaces.iter().find(|n| n.name == class),
                classes_and_interfaces.iter().find(|n| n.name == target)
            ) {
                edges.push(Edge {
                    from_id: from_node.id.clone(),
                    to_id: to_node.id.clone(),
                    edge_type: EdgeType::Implements,
                });
            }
        }

        // Try to find interface extends
        let interface_name = match_.captures.iter()
            .find(|c| query.capture_names()[c.index as usize] == "interface_name")
            .map(|c| &content[c.node.byte_range()]);
        let interface_extends = match_.captures.iter()
            .find(|c| query.capture_names()[c.index as usize] == "interface_extends_target")
            .map(|c| &content[c.node.byte_range()]);
        
        if let (Some(interface), Some(target)) = (interface_name, interface_extends) {
            if let (Some(from_node), Some(to_node)) = (
                classes_and_interfaces.iter().find(|n| n.name == interface),
                classes_and_interfaces.iter().find(|n| n.name == target)
            ) {
                edges.push(Edge {
                    from_id: from_node.id.clone(),
                    to_id: to_node.id.clone(),
                    edge_type: EdgeType::Implements, // Using Implements for interface extends
                });
            }
        }
    }

    Ok(edges)
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
