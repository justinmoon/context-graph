use crate::Result;

pub fn execute_query(_db_path: &str, _query: &str) -> Result<Vec<serde_json::Value>> {
    // TODO: Execute SQL query and return results
    Ok(Vec::new())
}

pub fn find_symbol(_db_path: &str, _pattern: &str, _limit: Option<usize>) -> Result<Vec<String>> {
    // TODO: Find symbols by pattern
    Ok(Vec::new())
}

pub fn find_callers(_db_path: &str, _symbol: &str) -> Result<Vec<String>> {
    // TODO: Find callers of a symbol
    Ok(Vec::new())
}
