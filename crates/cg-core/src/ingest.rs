use crate::Result;

pub struct IngestOptions {
    pub db_path: String,
    pub project_path: String,
    pub threads: Option<usize>,
    pub clean: bool,
}

pub fn ingest(_options: IngestOptions) -> Result<IngestStats> {
    // TODO: Implement ingestion pipeline
    Ok(IngestStats {
        files_processed: 0,
        symbols_created: 0,
        edges_created: 0,
    })
}

pub struct IngestStats {
    pub files_processed: usize,
    pub symbols_created: usize,
    pub edges_created: usize,
}
