//! Offline `OpenAPI` 3.1 document export using the exact `web` router builder without
//! constructing live runtime state, database pools, or external provider clients.

use std::io::Write;

/// Error returned when rendering or writing the offline `OpenAPI` specification fails.
#[derive(Debug, thiserror::Error)]
pub enum OpenApiExportError {
    /// Failed to serialize the `OpenAPI` document to JSON.
    #[error("failed to serialize OpenAPI specification")]
    Serialize(#[from] serde_json::Error),
    /// Failed to write the rendered `OpenAPI` document to the output sink.
    #[error("failed to write OpenAPI specification")]
    Io(#[from] std::io::Error),
}

/// Renders the canonical `OpenAPI` 3.1 JSON document offline with zero runtime configuration
/// or backend acquisition.
///
/// # Errors
///
/// Returns [`OpenApiExportError::Serialize`] if JSON serialization fails.
pub fn export_openapi_json() -> Result<String, OpenApiExportError> {
    Ok(web::api::openapi::render_openapi_json()?)
}

/// Writes the canonical `OpenAPI` 3.1 JSON document to `writer`.
///
/// # Errors
///
/// Returns [`OpenApiExportError`] if serialization or I/O fails.
pub fn write_openapi_json<W: Write>(mut writer: W) -> Result<(), OpenApiExportError> {
    let json = export_openapi_json()?;
    writer.write_all(json.as_bytes())?;
    writer.flush()?;
    Ok(())
}
