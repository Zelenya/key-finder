use crate::domain::errors::AppError;
use serde_json::Value;
use std::path::Path;

/// Parse importer-provided JSON, accepting strict JSON first and JSON5 as a fallback.
pub(crate) fn parse_json_loose(path: &Path, content: &str) -> Result<Value, AppError> {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return Ok(Value::Array(Vec::new()));
    }

    match serde_json::from_str::<Value>(trimmed) {
        Ok(value) => Ok(value),
        Err(json_err) => match json5::from_str::<Value>(trimmed) {
            Ok(value) => Ok(value),
            Err(json5_err) => Err(AppError::InvalidImporterSource {
                path: path.to_path_buf(),
                message: format!("can't parse json: {} (json5 fallback: {})", json_err, json5_err),
            }),
        },
    }
}
