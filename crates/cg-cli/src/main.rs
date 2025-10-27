use anyhow::Result;
use clap::{Parser, Subcommand};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Parser)]
#[command(name = "cg")]
#[command(about = "A lightweight tool for ingesting TypeScript code into a Kuzu graph", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Ingest TypeScript workspace into graph database
    Ingest {
        /// Path to the database directory
        #[arg(long, default_value = ".cg")]
        db: String,

        /// Path to the project directory
        #[arg(long, default_value = ".")]
        project: String,

        /// Number of threads for parallel processing
        #[arg(long)]
        threads: Option<usize>,

        /// Clean the database before ingestion
        #[arg(long)]
        clean: bool,
    },

    /// Execute a raw SQL query against the graph
    Query {
        /// SQL query to execute
        query: String,

        /// Path to the database directory
        #[arg(long, default_value = ".cg")]
        db: String,

        /// Output results as JSON
        #[arg(long)]
        json: bool,
    },

    /// Find symbols or relationships
    Find {
        #[command(subcommand)]
        command: FindCommands,
    },
}

#[derive(Subcommand)]
enum FindCommands {
    /// Find symbols by name pattern
    Symbol {
        /// Pattern to search for (SQL ILIKE)
        pattern: String,

        /// Path to the database directory
        #[arg(long, default_value = ".cg")]
        db: String,

        /// Maximum number of results
        #[arg(long)]
        limit: Option<usize>,
    },

    /// Find callers of a symbol
    Callers {
        /// Symbol ID or name
        symbol: String,

        /// Path to the database directory
        #[arg(long, default_value = ".cg")]
        db: String,
    },
}

fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "cg=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Ingest {
            db,
            project,
            threads,
            clean,
        } => {
            let options = cg_core::ingest::IngestOptions {
                db_path: db,
                project_path: project,
                threads,
                clean,
            };
            
            match cg_core::ingest::ingest(options) {
                Ok(stats) => {
                    println!("✓ Ingestion complete!");
                    println!("  Files processed: {}", stats.files_processed);
                    println!("  Symbols created: {}", stats.symbols_created);
                    println!("  Edges created: {}", stats.edges_created);
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Commands::Query { query, db, json } => {
            match cg_core::query::execute_query(&db, &query) {
                Ok(results) => {
                    if json {
                        println!("{}", serde_json::to_string_pretty(&results)?);
                    } else {
                        println!("Results: {} rows", results.len());
                        for (i, row) in results.iter().enumerate() {
                            println!("Row {}: {}", i, row);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Error executing query: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Commands::Find { command } => match command {
            FindCommands::Symbol { pattern, db, limit } => {
                match cg_core::query::find_symbol(&db, &pattern, limit) {
                    Ok(nodes) => {
                        if nodes.is_empty() {
                            println!("No symbols found matching: {}", pattern);
                        } else {
                            println!("Found {} symbol(s) matching '{}':\n", nodes.len(), pattern);
                            for node in nodes {
                                println!("  {} ({})", node.name, node.node_type.as_str());
                                println!("    File: {}", node.file);
                                if let (Some(start), Some(end)) = (node.start_line, node.end_line) {
                                    println!("    Lines: {}-{}", start, end);
                                }
                                println!();
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Error finding symbols: {}", e);
                        std::process::exit(1);
                    }
                }
            }
            FindCommands::Callers { symbol, db } => {
                match cg_core::query::find_callers(&db, &symbol) {
                    Ok(callers) => {
                        if callers.is_empty() {
                            println!("No callers found for: {}", symbol);
                        } else {
                            println!("Found {} caller(s) of '{}':\n", callers.len(), symbol);
                            for (caller, callee_name) in callers {
                                println!("  {} calls {}", caller.name, callee_name);
                                println!("    File: {}", caller.file);
                                if let (Some(start), Some(end)) = (caller.start_line, caller.end_line) {
                                    println!("    Lines: {}-{}", start, end);
                                }
                                println!();
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Error finding callers: {}", e);
                        std::process::exit(1);
                    }
                }
            }
        },
    }

    Ok(())
}
