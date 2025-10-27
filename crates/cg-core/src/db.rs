use crate::{model::{Edge, EdgeType, Node, NodeType}, Result};
use kuzu::{Connection, Database as KuzuDatabase, SystemConfig};
use std::path::Path;
use tracing::{debug, info};

pub struct Database {
    conn: Connection,
    _db: KuzuDatabase,
}

impl Database {
    pub fn new(path: &str) -> Result<Self> {
        info!("Initializing Kuzu database at: {}", path);
        
        let db_path = Path::new(path);
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let system_config = SystemConfig::default();
        let db = KuzuDatabase::new(path, system_config)?;
        let conn = Connection::new(&db)?;

        let mut database = Self { conn, _db: db };
        database.initialize_schema()?;
        
        Ok(database)
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
        let result = self.conn.query("MATCH (n:Node) RETURN count(n) LIMIT 1");
        Ok(result.is_ok())
    }

    fn create_schema(&mut self) -> Result<()> {
        info!("Creating node and relationship tables");

        self.conn.query("CREATE NODE TABLE Node(id STRING, node_type STRING, name STRING, file STRING, body STRING, start_line INT32, end_line INT32, PRIMARY KEY(id))")?;
        
        self.conn.query("CREATE NODE TABLE Metadata(key STRING, value STRING, PRIMARY KEY(key))")?;
        
        self.conn.query("CREATE REL TABLE Edge(FROM Node TO Node, edge_type STRING)")?;

        Ok(())
    }

    fn set_schema_version(&mut self, version: i32) -> Result<()> {
        self.conn.query(&format!(
            "CREATE (m:Metadata {{key: 'schema_version', value: '{}'}})",
            version
        ))?;
        Ok(())
    }

    pub fn insert_node(&mut self, node: &Node) -> Result<()> {
        let start_line = node.start_line.map(|l| l as i32).unwrap_or(-1);
        let end_line = node.end_line.map(|l| l as i32).unwrap_or(-1);
        
        let query = format!(
            "CREATE (n:Node {{id: '{}', node_type: '{}', name: '{}', file: '{}', body: '{}', start_line: {}, end_line: {}}})",
            node.id.replace("'", "\\'"),
            node.node_type.as_str(),
            node.name.replace("'", "\\'"),
            node.file.replace("'", "\\'"),
            node.body.replace("'", "\\'").replace('\n', "\\n"),
            start_line,
            end_line
        );
        
        self.conn.query(&query)?;
        Ok(())
    }

    pub fn insert_edge(&mut self, edge: &Edge) -> Result<()> {
        let query = format!(
            "MATCH (a:Node {{id: '{}'}}), (b:Node {{id: '{}'}}) CREATE (a)-[e:Edge {{edge_type: '{}'}}]->(b)",
            edge.from_id.replace("'", "\\'"),
            edge.to_id.replace("'", "\\'"),
            edge.edge_type.as_str()
        );
        
        self.conn.query(&query)?;
        Ok(())
    }

    pub fn count_nodes_by_type(&mut self, node_type: &NodeType) -> Result<usize> {
        let query = format!(
            "MATCH (n:Node {{node_type: '{}'}}) RETURN count(n) as count",
            node_type.as_str()
        );
        
        let mut result = self.conn.query(&query)?;
        if let Some(row) = result.next() {
            let value = row.get(0).ok_or_else(|| anyhow::anyhow!("No value at index 0"))?;
            let count = value.get_i64().ok_or_else(|| anyhow::anyhow!("Value is not i64"))?;
            Ok(count as usize)
        } else {
            Ok(0)
        }
    }

    pub fn count_edges_by_type(&mut self, edge_type: &EdgeType) -> Result<usize> {
        let query = format!(
            "MATCH ()-[e:Edge {{edge_type: '{}'}}]->() RETURN count(e) as count",
            edge_type.as_str()
        );
        
        let mut result = self.conn.query(&query)?;
        if let Some(row) = result.next() {
            let value = row.get(0).ok_or_else(|| anyhow::anyhow!("No value at index 0"))?;
            let count = value.get_i64().ok_or_else(|| anyhow::anyhow!("Value is not i64"))?;
            Ok(count as usize)
        } else {
            Ok(0)
        }
    }

    pub fn find_nodes_by_type(&mut self, node_type: &NodeType) -> Result<Vec<Node>> {
        let query = format!(
            "MATCH (n:Node {{node_type: '{}'}}) RETURN n.id, n.node_type, n.name, n.file, n.body, n.start_line, n.end_line",
            node_type.as_str()
        );
        
        let mut result = self.conn.query(&query)?;
        let mut nodes = Vec::new();
        
        while let Some(row) = result.next() {
            let id = row.get(0).and_then(|v| v.get_str()).unwrap_or("").to_string();
            let name = row.get(2).and_then(|v| v.get_str()).unwrap_or("").to_string();
            let file = row.get(3).and_then(|v| v.get_str()).unwrap_or("").to_string();
            let body = row.get(4).and_then(|v| v.get_str()).unwrap_or("").to_string();
            let start_line = row.get(5).and_then(|v| v.get_i32()).unwrap_or(-1);
            let end_line = row.get(6).and_then(|v| v.get_i32()).unwrap_or(-1);
            
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
        self.conn.query("MATCH (n:Node) DETACH DELETE n")?;
        Ok(())
    }
}
