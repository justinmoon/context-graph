use crate::{db::Database, fs::Project, model::{EdgeType, Node, NodeType}, parser, Result};
use rayon::prelude::*;
use tracing::{info, warn};

pub struct IngestOptions {
    pub db_path: String,
    pub project_path: String,
    pub threads: Option<usize>,
    pub clean: bool,
}

pub fn ingest(options: IngestOptions) -> Result<IngestStats> {
    info!("Starting ingestion");
    info!("  Database: {}", options.db_path);
    info!("  Project: {}", options.project_path);
    
    let mut db = Database::new(&options.db_path)?;
    
    if options.clean {
        info!("Cleaning existing data");
        db.clear()?;
    }

    let project = Project::discover(&options.project_path)?;
    let files = project.find_typescript_files()?;
    
    info!("Processing {} files", files.len());

    let repository_node = Node::new(
        NodeType::Repository,
        project.root.display().to_string(),
        project.root.display().to_string(),
    );
    // Delete and re-create repository node
    db.delete_file_and_symbols(&repository_node.id)?;
    db.insert_node(&repository_node)?;

    let language_node = Node::new(
        NodeType::Language,
        "typescript".to_string(),
        project.root.display().to_string(),
    );
    // Delete and re-create language node
    db.delete_file_and_symbols(&language_node.id)?;
    db.insert_node(&language_node)?;

    let parse_files = || -> Vec<_> {
        files.par_iter()
        .filter_map(|file_path| {
            let path_str = file_path.display().to_string();
            match project.read_file(file_path) {
                Ok(content) => {
                    match parser::parse_typescript_file(&path_str, &content) {
                        Ok(parsed) => Some((path_str, parsed)),
                        Err(e) => {
                            warn!("Failed to parse {}: {}", path_str, e);
                            None
                        }
                    }
                }
                Err(e) => {
                    warn!("Failed to read {}: {}", path_str, e);
                    None
                }
            }
        })
        .collect()
    };

    let parsed_files = if let Some(threads) = options.threads {
        rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()?
            .install(parse_files)
    } else {
        parse_files()
    };

    let mut stats = IngestStats {
        files_processed: 0,
        symbols_created: 0,
        edges_created: 0,
    };

    for (file_path, parsed) in parsed_files {
        let file_node = Node::new(
            NodeType::File,
            file_path.clone(),
            file_path.clone(),
        );
        
        // Delete existing file and its symbols before re-inserting
        db.delete_file_and_symbols(&file_node.id)?;
        db.insert_node(&file_node)?;

        for node in &parsed.nodes {
            db.insert_node(node)?;
            stats.symbols_created += 1;

            let contains_edge = crate::model::Edge {
                from_id: file_node.id.clone(),
                to_id: node.id.clone(),
                edge_type: EdgeType::Contains,
            };
            db.insert_edge(&contains_edge)?;
            stats.edges_created += 1;
        }

        for edge in &parsed.edges {
            db.insert_edge(edge)?;
            stats.edges_created += 1;
        }

        stats.files_processed += 1;
    }

    info!("Ingestion complete");
    info!("  Files processed: {}", stats.files_processed);
    info!("  Symbols created: {}", stats.symbols_created);
    info!("  Edges created: {}", stats.edges_created);

    Ok(stats)
}

pub struct IngestStats {
    pub files_processed: usize,
    pub symbols_created: usize,
    pub edges_created: usize,
}
