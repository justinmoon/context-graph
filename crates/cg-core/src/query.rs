use crate::{db::Database, model::Node, Result};
use kuzu::Value;
use serde_json::json;

pub fn execute_query(db_path: &str, query: &str) -> Result<Vec<serde_json::Value>> {
    let db = Database::new(db_path)?;
    let conn = db.get_connection()?;
    
    let mut results = Vec::new();
    let query_result = conn.query(query)?;
    
    for row in query_result {
        let mut row_data = Vec::new();
        for i in 0..row.len() {
            row_data.push(kuzu_value_to_json(&row[i]));
        }
        results.push(json!(row_data));
    }
    
    Ok(results)
}

pub fn find_symbol(db_path: &str, pattern: &str, limit: Option<usize>) -> Result<Vec<Node>> {
    let db = Database::new(db_path)?;
    
    let limit_clause = limit.map(|l| format!(" LIMIT {}", l)).unwrap_or_default();
    
    // Use regex for case-insensitive pattern matching
    let query = format!(
        "MATCH (n:Node) WHERE n.name =~ '(?i).*{}.*' RETURN n.id, n.node_type, n.name, n.file, n.body, n.start_line, n.end_line ORDER BY n.name{}",
        regex::escape(pattern),
        limit_clause
    );
    
    let conn = db.get_connection()?;
    let mut nodes = Vec::new();
    
    for row in conn.query(&query)? {
        let node = parse_node_from_row(&row)?;
        nodes.push(node);
    }
    
    Ok(nodes)
}

pub fn find_callers(db_path: &str, symbol: &str) -> Result<Vec<(Node, String)>> {
    let db = Database::new(db_path)?;
    
    // Find all nodes that call the given symbol
    // Match nodes either by name or by ID
    let query = format!(
        "MATCH (caller:Node)-[e:Edge {{edge_type: 'Calls'}}]->(callee:Node) \
         WHERE callee.name = '{}' OR callee.id = '{}' \
         RETURN caller.id, caller.node_type, caller.name, caller.file, caller.body, caller.start_line, caller.end_line, callee.name \
         ORDER BY caller.name",
        symbol.replace('\'', "''"),
        symbol.replace('\'', "''")
    );
    
    let conn = db.get_connection()?;
    let mut results = Vec::new();
    
    for row in conn.query(&query)? {
        let caller = parse_node_from_row(&row)?;
        let callee_name = if let Value::String(s) = &row[7] {
            s.clone()
        } else {
            String::new()
        };
        results.push((caller, callee_name));
    }
    
    Ok(results)
}

fn parse_node_from_row(row: &[Value]) -> Result<Node> {
    let id = if let Value::String(s) = &row[0] {
        s.clone()
    } else {
        String::new()
    };
    
    let node_type_str = if let Value::String(s) = &row[1] {
        s.as_str()
    } else {
        ""
    };
    
    let name = if let Value::String(s) = &row[2] {
        s.clone()
    } else {
        String::new()
    };
    
    let file = if let Value::String(s) = &row[3] {
        s.clone()
    } else {
        String::new()
    };
    
    let body = if let Value::String(s) = &row[4] {
        s.clone()
    } else {
        String::new()
    };
    
    let start_line = if let Value::Int32(i) = &row[5] {
        if i >= &0 {
            Some(*i as u32)
        } else {
            None
        }
    } else {
        None
    };
    
    let end_line = if let Value::Int32(i) = &row[6] {
        if i >= &0 {
            Some(*i as u32)
        } else {
            None
        }
    } else {
        None
    };
    
    // Parse node_type string back to enum
    use crate::model::NodeType;
    let node_type = match node_type_str {
        "Repository" => NodeType::Repository,
        "Language" => NodeType::Language,
        "File" => NodeType::File,
        "Directory" => NodeType::Directory,
        "Function" => NodeType::Function,
        "Class" => NodeType::Class,
        "Interface" => NodeType::Interface,
        "DataModel" => NodeType::DataModel,
        "Trait" => NodeType::Trait,
        "Var" => NodeType::Var,
        "Import" => NodeType::Import,
        "Library" => NodeType::Library,
        "Endpoint" => NodeType::Endpoint,
        "Request" => NodeType::Request,
        "Page" => NodeType::Page,
        "Instance" => NodeType::Instance,
        _ => return Err(anyhow::anyhow!("Unknown node type: {}", node_type_str)),
    };
    
    Ok(Node {
        id,
        node_type,
        name,
        file,
        body,
        start_line,
        end_line,
        meta: std::collections::HashMap::new(),
    })
}

fn kuzu_value_to_json(value: &Value) -> serde_json::Value {
    match value {
        Value::Null(_) => json!(null),
        Value::Bool(b) => json!(b),
        Value::Int64(i) => json!(i),
        Value::Int32(i) => json!(i),
        Value::Int16(i) => json!(i),
        Value::Int8(i) => json!(i),
        Value::UInt64(i) => json!(i),
        Value::UInt32(i) => json!(i),
        Value::UInt16(i) => json!(i),
        Value::UInt8(i) => json!(i),
        Value::Float(f) => json!(f),
        Value::Double(f) => json!(f),
        Value::String(s) => json!(s),
        Value::Date(d) => json!(d.to_string()),
        Value::Timestamp(t) => json!(t.to_string()),
        Value::Interval(i) => json!(i.to_string()),
        _ => json!(format!("{:?}", value)),
    }
}
