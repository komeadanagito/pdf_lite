pub mod commands;
pub mod compress;
pub mod image_compress;
pub mod video_compress;

pub fn run() {
  tauri::Builder::default()
    .plugin(tauri_plugin_dialog::init())
    .invoke_handler(tauri::generate_handler![
      commands::compress_pdf,
      commands::get_pdf_info,
      commands::get_file_size,
      commands::compress_image,
      commands::compress_video,
    ])
    .run(tauri::generate_context!())
    .expect("failed to run tauri application");
}
