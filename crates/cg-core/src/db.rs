use crate::{model::{Edge, EdgeType, Node, NodeType}, Result};
use kuzu::{Connection, Database as KuzuDatabase, SystemConfig, Value};
use std::path::Path;
use tracing::{debug, info};

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
            node.id.replace("'", "\\'"),
            node.node_type.as_str(),
            node.name.replace("'", "\\'"),
            node.file.replace("'", "\\'"),
            node.body.replace("'", "\\'").replace('\n', "\\n"),
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
            edge.from_id.replace("'", "\\'"),
            edge.to_id.replace("'", "\\'"),
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
}
