use std::path::PathBuf;

use tauri::AppHandle;

use crate::compress::{compress_pdf_at_path, get_pdf_info_for_path, CompressResult, CompressionMode, PdfInfo};
use crate::image_compress;
use crate::video_compress;

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

#[derive(serde::Serialize)]
pub struct FileSizeResult {
  pub size: u64,
}

#[tauri::command]
pub async fn get_file_size(path: String) -> Result<FileSizeResult, String> {
  let meta = std::fs::metadata(&path).map_err(|e| e.to_string())?;
  Ok(FileSizeResult { size: meta.len() })
}

#[tauri::command]
pub async fn compress_image(app: AppHandle, path: String, mode: u8) -> Result<CompressResult, String> {
  let mode = CompressionMode::from_u8(mode)?;
  let path = PathBuf::from(path);
  let app_handle = app.clone();

  tauri::async_runtime::spawn_blocking(move || image_compress::compress_image_at_path(&app_handle, &path, mode))
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn compress_video(app: AppHandle, path: String, mode: u8) -> Result<CompressResult, String> {
  let mode = CompressionMode::from_u8(mode)?;
  let path = PathBuf::from(path);
  let app_handle = app.clone();

  tauri::async_runtime::spawn_blocking(move || video_compress::compress_video_at_path(&app_handle, &path, mode))
    .await
    .map_err(|error| error.to_string())?
}
