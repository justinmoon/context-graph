use crate::{db::Database, fs::Project, git, model::{EdgeType, Node, NodeType}, parser, Result};
use rayon::prelude::*;
use std::collections::HashSet;
use std::path::PathBuf;
use tracing::{info, warn};

pub struct IngestOptions {
    pub db_path: String,
    pub project_path: String,
    pub threads: Option<usize>,
    pub clean: bool,
    pub incremental: bool,
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
    let project_root = &project.root;
    
    // Determine which files to process
    let files = if options.incremental && !options.clean {
        // Try incremental ingestion
        if let Some(changed_files) = get_incremental_files(&mut db, project_root)? {
            info!("Incremental mode: processing {} changed files", changed_files.len());
            changed_files
        } else {
            info!("Incremental mode unavailable, falling back to full ingestion");
            project.find_typescript_files()?
        }
    } else {
        project.find_typescript_files()?
    };
    
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

    // Store current commit hash if in a git repo
    if git::is_git_repo(project_root) {
        if let Ok(commit) = git::get_current_commit(project_root) {
            db.set_metadata("last_commit", &commit)?;
            info!("Stored commit hash: {}", &commit[..8]);
        }
    }

    info!("Ingestion complete");
    info!("  Files processed: {}", stats.files_processed);
    info!("  Symbols created: {}", stats.symbols_created);
    info!("  Edges created: {}", stats.edges_created);

    Ok(stats)
}

/// Get list of changed files for incremental ingestion
/// Returns None if incremental ingestion is not possible
fn get_incremental_files(
    db: &mut Database,
    project_root: &PathBuf,
) -> Result<Option<Vec<PathBuf>>> {
    // Check if this is a git repo
    if !git::is_git_repo(project_root) {
        return Ok(None);
    }

    // Get last commit from metadata
    let last_commit = match db.get_metadata("last_commit")? {
        Some(commit) => commit,
        None => return Ok(None), // No previous ingestion
    };

    // Get current commit
    let current_commit = git::get_current_commit(project_root)?;

    // If commits are the same, no changes
    if last_commit == current_commit {
        info!("No changes since last ingestion (commit: {})", &current_commit[..8]);
        return Ok(Some(Vec::new()));
    }

    // Get changed files
    let changed_files = git::get_changed_files(project_root, &last_commit, &current_commit)?;

    // Filter for TypeScript files only
    let ts_extensions: HashSet<&str> = ["ts", "tsx", "d.ts"].iter().copied().collect();
    let ts_changed_files: Vec<PathBuf> = changed_files
        .into_iter()
        .filter(|path| {
            path.extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ts_extensions.contains(ext))
                .unwrap_or(false)
        })
        .collect();

    info!(
        "Changed since {}: {} files",
        &last_commit[..8],
        ts_changed_files.len()
    );

    Ok(Some(ts_changed_files))
}

pub struct IngestStats {
    pub files_processed: usize,
    pub symbols_created: usize,
    pub edges_created: usize,
}
