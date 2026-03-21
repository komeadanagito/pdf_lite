pub mod commands;
pub mod compress;

pub fn run() {
  tauri::Builder::default()
    .plugin(tauri_plugin_dialog::init())
    .invoke_handler(tauri::generate_handler![
      commands::compress_pdf,
      commands::get_pdf_info
    ])
    .run(tauri::generate_context!())
    .expect("failed to run tauri application");
}
