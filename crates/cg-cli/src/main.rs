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
            tracing::info!("Executing query");
            tracing::info!("Database: {}", db);
            tracing::info!("JSON output: {}", json);
            // TODO: Implement query execution
            println!("Query: {}", query);
            println!("Query command not yet implemented");
        }
        Commands::Find { command } => match command {
            FindCommands::Symbol { pattern, db, limit } => {
                tracing::info!("Finding symbols matching: {}", pattern);
                tracing::info!("Database: {}", db);
                tracing::info!("Limit: {:?}", limit);
                // TODO: Implement symbol search
                println!("Find symbol command not yet implemented");
            }
            FindCommands::Callers { symbol, db } => {
                tracing::info!("Finding callers of: {}", symbol);
                tracing::info!("Database: {}", db);
                // TODO: Implement caller search
                println!("Find callers command not yet implemented");
            }
        },
    }

    Ok(())
}
