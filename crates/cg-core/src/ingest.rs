use crate::{db::Database, fs::Project, git, model::{Edge, EdgeType, Node, NodeType}, parser, Result};
use rayon::prelude::*;
use std::collections::HashSet;
use std::path::PathBuf;
use tracing::{debug, info, warn};

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

    // Track if any errors occurred during ingestion
    let mut had_errors = false;

    // Collect all import edges and symbol maps for processing after files are inserted
    let mut all_import_edges: Vec<(String, String)> = Vec::new();
    
    // Build symbol map: (symbol_name, file_path) -> Node
    let mut symbol_map: std::collections::HashMap<(String, String), Node> = std::collections::HashMap::new();
    
    // Build import map: (file_path, symbol_name) -> source_file_path
    let mut import_map: std::collections::HashMap<(String, String), String> = std::collections::HashMap::new();

    for (file_path, parsed) in &parsed_files {
        // Collect import edges from this file
        all_import_edges.extend(parsed.import_edges.clone());
        
        // Build symbol map (functions and classes in this file)
        for node in &parsed.nodes {
            if matches!(node.node_type, NodeType::Function | NodeType::Class) {
                symbol_map.insert((node.name.clone(), file_path.clone()), node.clone());
            }
        }
        
        // Build import map (what symbols this file imports from where)
        for import_info in &parsed.imports {
            import_map.insert(
                (file_path.clone(), import_info.symbol.clone()),
                import_info.from_file.clone()
            );
        }
    }

    for (file_path, parsed) in parsed_files {
        let file_node = Node::new(
            NodeType::File,
            file_path.clone(),
            file_path.clone(),
        );
        
        // Delete existing file and its symbols before re-inserting
        if let Err(e) = db.delete_file_and_symbols(&file_node.id) {
            warn!("Failed to delete existing file {}: {}", file_path, e);
            had_errors = true;
            continue;
        }
        
        if let Err(e) = db.insert_node(&file_node) {
            warn!("Failed to insert file node {}: {}", file_path, e);
            had_errors = true;
            continue;
        }

        for node in &parsed.nodes {
            if let Err(e) = db.insert_node(node) {
                warn!("Failed to insert node {} in {}: {}", node.name, file_path, e);
                had_errors = true;
                continue;
            }
            stats.symbols_created += 1;

            let contains_edge = crate::model::Edge {
                from_id: file_node.id.clone(),
                to_id: node.id.clone(),
                edge_type: EdgeType::Contains,
            };
            if let Err(e) = db.insert_edge(&contains_edge) {
                warn!("Failed to insert contains edge in {}: {}", file_path, e);
                had_errors = true;
                continue;
            }
            stats.edges_created += 1;
        }

        for edge in &parsed.edges {
            if let Err(e) = db.insert_edge(edge) {
                warn!("Failed to insert edge in {}: {}", file_path, e);
                had_errors = true;
                continue;
            }
            stats.edges_created += 1;
        }

        stats.files_processed += 1;
    }

    // Create Import edges between files
    for (from_file, to_file) in all_import_edges {
        let from_node = Node::new(NodeType::File, from_file.clone(), from_file.clone());
        let to_node = Node::new(NodeType::File, to_file.clone(), to_file.clone());
        
        let import_edge = Edge {
            from_id: from_node.id.clone(),
            to_id: to_node.id.clone(),
            edge_type: EdgeType::Imports,
        };
        
        if let Err(e) = db.insert_edge(&import_edge) {
            debug!("Failed to insert import edge {} -> {}: {}", from_file, to_file, e);
            // Don't fail the whole ingestion for import edges
        } else {
            stats.edges_created += 1;
        }
    }
    
    // Create cross-file call edges using import resolution
    let cross_file_edges = resolve_cross_file_calls(&symbol_map, &import_map);
    for edge in cross_file_edges {
        if let Err(e) = db.insert_edge(&edge) {
            debug!("Failed to insert cross-file call edge: {}", e);
        } else {
            stats.edges_created += 1;
        }
    }

    // Only store commit hash if ingestion was successful (no errors)
    if !had_errors && git::is_git_repo(project_root) {
        if let Ok(commit) = git::get_current_commit(project_root) {
            if let Err(e) = db.set_metadata("last_commit", &commit) {
                warn!("Failed to store commit hash: {}", e);
            } else {
                info!("Stored commit hash: {}", &commit[..8]);
            }
        }
    } else if had_errors {
        warn!("Ingestion had errors, not updating commit hash");
    }

    info!("Ingestion complete");
    info!("  Files processed: {}", stats.files_processed);
    info!("  Symbols created: {}", stats.symbols_created);
    info!("  Edges created: {}", stats.edges_created);

    Ok(stats)
}

struct IncrementalChanges {
    files_to_process: Vec<PathBuf>,
    files_to_delete: Vec<PathBuf>,
}

/// Get list of changed files for incremental ingestion
/// Returns None if incremental ingestion is not possible
fn get_incremental_files(
    db: &mut Database,
    project_root: &PathBuf,
) -> Result<Option<Vec<PathBuf>>> {
    let changes = match get_incremental_changes(db, project_root) {
        Ok(Some(changes)) => changes,
        Ok(None) => return Ok(None),
        Err(e) => {
            warn!("Incremental ingestion failed ({}), falling back to full ingestion", e);
            return Ok(None);
        }
    };

    // Delete files that were removed or renamed
    info!("Deleting {} files", changes.files_to_delete.len());
    for path in &changes.files_to_delete {
        // Convert to string representation that matches what's stored in DB
        let path_str = path.display().to_string();
        let file_id = Node::new(NodeType::File, path_str.clone(), path_str.clone()).id;
        
        info!("Attempting to delete file: {} (id: {})", path_str, file_id);
        match db.delete_file_and_symbols(&file_id) {
            Ok(_) => info!("Successfully deleted: {}", path_str),
            Err(e) => {
                // File might not exist in DB (e.g., not a TS file in previous ingest)
                warn!("Could not delete file {}: {}", path_str, e);
            }
        }
    }

    Ok(Some(changes.files_to_process))
}

/// Get incremental changes with file status
fn get_incremental_changes(
    db: &mut Database,
    project_root: &PathBuf,
) -> Result<Option<IncrementalChanges>> {
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
        return Ok(Some(IncrementalChanges {
            files_to_process: Vec::new(),
            files_to_delete: Vec::new(),
        }));
    }

    // Get changed files with status
    let file_changes = git::get_file_changes(project_root, &last_commit, &current_commit)?;

    let ts_extensions: HashSet<&str> = ["ts", "tsx", "d.ts"].iter().copied().collect();
    
    let mut files_to_process = Vec::new();
    let mut files_to_delete = Vec::new();

    for change in file_changes {
        // Check if it's a TypeScript file
        let is_ts = change.path.extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ts_extensions.contains(ext))
            .unwrap_or(false);

        if !is_ts {
            continue;
        }

        match change.change_type {
            git::FileChangeType::Added | git::FileChangeType::Modified => {
                files_to_process.push(change.path);
            }
            git::FileChangeType::Deleted => {
                files_to_delete.push(change.path);
            }
            git::FileChangeType::Renamed { old_path } => {
                // Delete the old path, process the new path
                files_to_delete.push(old_path);
                files_to_process.push(change.path);
            }
        }
    }

    info!(
        "Changed since {}: {} to process, {} to delete",
        &last_commit[..8],
        files_to_process.len(),
        files_to_delete.len()
    );

    Ok(Some(IncrementalChanges {
        files_to_process,
        files_to_delete,
    }))
}

fn resolve_cross_file_calls(
    _symbol_map: &std::collections::HashMap<(String, String), Node>,
    _import_map: &std::collections::HashMap<(String, String), String>,
) -> Vec<Edge> {
    // TODO: Cross-file call resolution
    // 
    // Current limitation: extract_calls() only creates edges for calls to functions
    // within the same file. Cross-file calls (e.g., calling an imported function)
    // are not yet resolved.
    //
    // To implement this properly, we need to:
    // 1. Store unresolved calls in ParsedFile (call sites that don't match local functions)
    // 2. After building symbol_map and import_map, resolve these calls:
    //    - For each unresolved call to symbol X in file F:
    //      a. Check if X is imported in F (use import_map)
    //      b. If yes, get source file and look up X in symbol_map
    //      c. Create Calls edge from caller to the imported function
    //
    // This requires refactoring extract_calls() to return both resolved and unresolved calls.
    
    Vec::new()
}

pub struct IngestStats {
    pub files_processed: usize,
    pub symbols_created: usize,
    pub edges_created: usize,
}
