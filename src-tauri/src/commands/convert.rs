use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::State;
use crate::state::AppState;

#[derive(Debug, Serialize, Deserialize)]
pub struct ConvertOptionsDto {
    pub output_format: String,
    pub output_dir: Option<String>,
    pub bit_depth: Option<u16>,
    pub preserve_tags: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct ConvertResult {
    pub input: String,
    pub output: String,
    pub success: bool,
    pub error: Option<String>,
}

/// Convert a list of audio files to the specified format
#[tauri::command]
pub async fn convert_files(
    _state: State<'_, AppState>,
    files: Vec<String>,
    options: ConvertOptionsDto,
) -> Result<Vec<ConvertResult>, String> {
    use tt_core::convert::{ConvertOptions, OutputFormat, convert_file};

    let output_format = match options.output_format.as_str() {
        "wav" => OutputFormat::Wav,
        other => return Err(format!("Unsupported output format: {}", other)),
    };

    let opts = ConvertOptions {
        output_format,
        output_dir: options.output_dir.map(PathBuf::from),
        bit_depth: options.bit_depth.unwrap_or(16),
        preserve_tags: options.preserve_tags.unwrap_or(true),
    };

    let mut results = Vec::new();

    for file in &files {
        let path = PathBuf::from(file);
        match convert_file(&path, &opts, None) {
            Ok(output) => {
                results.push(ConvertResult {
                    input: file.clone(),
                    output: output.to_string_lossy().to_string(),
                    success: true,
                    error: None,
                });
            }
            Err(e) => {
                results.push(ConvertResult {
                    input: file.clone(),
                    output: String::new(),
                    success: false,
                    error: Some(e.to_string()),
                });
            }
        }
    }

    Ok(results)
}

/// Get supported output formats
#[tauri::command]
pub async fn convert_get_formats(
    _state: State<'_, AppState>,
) -> Result<Vec<serde_json::Value>, String> {
    use tt_core::convert::OutputFormat;

    let formats: Vec<serde_json::Value> = OutputFormat::all()
        .iter()
        .map(|f| {
            serde_json::json!({
                "id": f.extension(),
                "name": f.display_name(),
                "extension": f.extension(),
            })
        })
        .collect();

    Ok(formats)
}
