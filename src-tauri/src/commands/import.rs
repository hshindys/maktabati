//! Import helpers — preview parsers for external library exports.

use maktabati_metadata::goodreads::{GoodreadsRow, parse_goodreads_csv};

use crate::error::{AppErrorDto, CmdResult};

#[specta::specta]
#[tauri::command]
pub async fn goodreads_parse(text: String) -> CmdResult<Vec<GoodreadsRow>> {
    parse_goodreads_csv(&text).map_err(|e| AppErrorDto::new("import", e.to_string()))
}
