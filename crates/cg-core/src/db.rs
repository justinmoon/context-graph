use crate::{model::{Edge, EdgeType, Node, NodeType}, Result};
use kuzu::{Connection, Database as KuzuDatabase, SystemConfig, Value};
use std::path::Path;
use tracing::{debug, info};

/// Escape a string for use in Kuzu/Cypher queries
/// Kuzu uses backslash escaping for special characters
fn escape_kuzu_string(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('\'', "\\'")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

pub struct Database {
    db: Box<KuzuDatabase>,
}

impl Database {
    pub fn new(path: &str) -> Result<Self> {
        info!("Initializing Kuzu database at: {}", path);
        
        let db_path = Path::new(path);
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let system_config = SystemConfig::default();
        let db = Box::new(KuzuDatabase::new(path, system_config)?);

        let mut database = Self { db };
        database.initialize_schema()?;
        
        Ok(database)
    }

    fn get_conn(&self) -> Result<Connection<'_>> {
        Ok(Connection::new(&self.db)?)
    }

    pub fn get_connection(&self) -> Result<Connection<'_>> {
        self.get_conn()
    }

    pub fn initialize_schema(&mut self) -> Result<()> {
        info!("Initializing database schema");
        
        if !self.check_schema_exists()? {
            self.create_schema()?;
            self.set_schema_version(1)?;
            info!("Schema created successfully");
        } else {
            debug!("Schema already exists");
        }
        
        Ok(())
    }

    fn check_schema_exists(&mut self) -> Result<bool> {
        let conn = self.get_conn()?;
        let result = conn.query("MATCH (n:Node) RETURN count(n) LIMIT 1");
        Ok(result.is_ok())
    }

    fn create_schema(&mut self) -> Result<()> {
        info!("Creating node and relationship tables");
        let conn = self.get_conn()?;

        conn.query("CREATE NODE TABLE Node(id STRING, node_type STRING, name STRING, file STRING, body STRING, start_line INT32, end_line INT32, PRIMARY KEY(id))")?;
        
        conn.query("CREATE NODE TABLE Metadata(key STRING, value STRING, PRIMARY KEY(key))")?;
        
        conn.query("CREATE REL TABLE Edge(FROM Node TO Node, edge_type STRING)")?;

        Ok(())
    }

    fn set_schema_version(&mut self, version: i32) -> Result<()> {
        let conn = self.get_conn()?;
        conn.query(&format!(
            "CREATE (m:Metadata {{key: 'schema_version', value: '{}'}})",
            version
        ))?;
        Ok(())
    }

    pub fn insert_node(&mut self, node: &Node) -> Result<()> {
        let conn = self.get_conn()?;
        let start_line = node.start_line.map(|l| l as i32).unwrap_or(-1);
        let end_line = node.end_line.map(|l| l as i32).unwrap_or(-1);
        
        let query = format!(
            "CREATE (n:Node {{id: '{}', node_type: '{}', name: '{}', file: '{}', body: '{}', start_line: {}, end_line: {}}})",
            escape_kuzu_string(&node.id),
            node.node_type.as_str(),
            escape_kuzu_string(&node.name),
            escape_kuzu_string(&node.file),
            escape_kuzu_string(&node.body),
            start_line,
            end_line
        );
        
        conn.query(&query)?;
        Ok(())
    }

    pub fn insert_edge(&mut self, edge: &Edge) -> Result<()> {
        let conn = self.get_conn()?;
        let query = format!(
            "MATCH (a:Node {{id: '{}'}}), (b:Node {{id: '{}'}}) CREATE (a)-[e:Edge {{edge_type: '{}'}}]->(b)",
            escape_kuzu_string(&edge.from_id),
            escape_kuzu_string(&edge.to_id),
            edge.edge_type.as_str()
        );
        
        conn.query(&query)?;
        Ok(())
    }

    pub fn count_nodes_by_type(&mut self, node_type: &NodeType) -> Result<usize> {
        let conn = self.get_conn()?;
        let query = format!(
            "MATCH (n:Node {{node_type: '{}'}}) RETURN count(n) as count",
            node_type.as_str()
        );
        
        for row in conn.query(&query)? {
            if let Value::Int64(count) = &row[0] {
                return Ok(*count as usize);
            }
        }
        Ok(0)
    }

    pub fn count_edges_by_type(&mut self, edge_type: &EdgeType) -> Result<usize> {
        let conn = self.get_conn()?;
        let query = format!(
            "MATCH ()-[e:Edge {{edge_type: '{}'}}]->() RETURN count(e) as count",
            edge_type.as_str()
        );
        
        for row in conn.query(&query)? {
            if let Value::Int64(count) = &row[0] {
                return Ok(*count as usize);
            }
        }
        Ok(0)
    }

    pub fn find_nodes_by_type(&mut self, node_type: &NodeType) -> Result<Vec<Node>> {
        let conn = self.get_conn()?;
        let query = format!(
            "MATCH (n:Node {{node_type: '{}'}}) RETURN n.id, n.node_type, n.name, n.file, n.body, n.start_line, n.end_line",
            node_type.as_str()
        );
        
        let mut nodes = Vec::new();
        
        for row in conn.query(&query)? {
            let id = if let Value::String(s) = &row[0] { s.clone() } else { String::new() };
            let name = if let Value::String(s) = &row[2] { s.clone() } else { String::new() };
            let file = if let Value::String(s) = &row[3] { s.clone() } else { String::new() };
            let body = if let Value::String(s) = &row[4] { s.clone() } else { String::new() };
            let start_line = if let Value::Int32(i) = &row[5] { *i } else { -1 };
            let end_line = if let Value::Int32(i) = &row[6] { *i } else { -1 };
            
            nodes.push(Node {
                id,
                node_type: node_type.clone(),
                name,
                file,
                body,
                start_line: if start_line >= 0 { Some(start_line as u32) } else { None },
                end_line: if end_line >= 0 { Some(end_line as u32) } else { None },
                meta: std::collections::HashMap::new(),
            });
        }
        
        Ok(nodes)
    }

    pub fn clear(&mut self) -> Result<()> {
        info!("Clearing all data from database");
        let conn = self.get_conn()?;
        conn.query("MATCH (n:Node) DETACH DELETE n")?;
        Ok(())
    }

    pub fn delete_file_and_symbols(&mut self, file_id: &str) -> Result<()> {
        let conn = self.get_conn()?;
        
        // Delete symbols contained by this file (via Contains edges only)
        // This prevents accidentally deleting nodes connected via other edge types (Imports, Uses, etc.)
        let query = format!(
            "MATCH (f:Node {{id: '{}'}})-[e:Edge {{edge_type: 'Contains'}}]->(s:Node) DETACH DELETE s",
            escape_kuzu_string(file_id)
        );
        conn.query(&query)?;
        
        // Delete the file node itself and all its edges
        let query = format!(
            "MATCH (f:Node {{id: '{}'}}) DETACH DELETE f",
            escape_kuzu_string(file_id)
        );
        conn.query(&query)?;
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_database_init() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let db_path = temp_dir.path().join("test.db");
        let _db = Database::new(db_path.to_str().unwrap())?;
        Ok(())
    }

    #[test]
    fn test_insert_and_query_node() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let db_path = temp_dir.path().join("test.db");
        let mut db = Database::new(db_path.to_str().unwrap())?;

        let node = Node::new(
            NodeType::Function,
            "testFunction".to_string(),
            "test.ts".to_string(),
        ).with_body("function testFunction() {}".to_string())
         .with_lines(1, 3);

        db.insert_node(&node)?;

        let count = db.count_nodes_by_type(&NodeType::Function)?;
        assert_eq!(count, 1);

        let nodes = db.find_nodes_by_type(&NodeType::Function)?;
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].name, "testFunction");
        assert_eq!(nodes[0].file, "test.ts");

        Ok(())
    }

    #[test]
    fn test_insert_and_query_edge() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let db_path = temp_dir.path().join("test.db");
        let mut db = Database::new(db_path.to_str().unwrap())?;

        let node1 = Node::new(
            NodeType::Function,
            "caller".to_string(),
            "test.ts".to_string(),
        );
        let node2 = Node::new(
            NodeType::Function,
            "callee".to_string(),
            "test.ts".to_string(),
        );

        db.insert_node(&node1)?;
        db.insert_node(&node2)?;

        let edge = Edge {
            from_id: node1.id.clone(),
            to_id: node2.id.clone(),
            edge_type: EdgeType::Calls,
        };
        db.insert_edge(&edge)?;

        let count = db.count_edges_by_type(&EdgeType::Calls)?;
        assert_eq!(count, 1);

        Ok(())
    }
}
