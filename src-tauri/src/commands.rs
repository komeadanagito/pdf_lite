use std::path::PathBuf;

use tauri::AppHandle;

use crate::compress::{compress_pdf_at_path, get_pdf_info_for_path, CompressResult, CompressionMode, PdfInfo};

#[tauri::command]
pub async fn compress_pdf(app: AppHandle, path: String, mode: u8) -> Result<CompressResult, String> {
  let mode = CompressionMode::from_u8(mode)?;
  let path = PathBuf::from(path);
  let app_handle = app.clone();

  tauri::async_runtime::spawn_blocking(move || compress_pdf_at_path(&app_handle, &path, mode))
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn get_pdf_info(path: String) -> Result<PdfInfo, String> {
  let path = PathBuf::from(path);
  tauri::async_runtime::spawn_blocking(move || get_pdf_info_for_path(&path))
    .await
    .map_err(|error| error.to_string())?
}
