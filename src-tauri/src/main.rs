use pdf_lite::commands::{compress_pdf, get_pdf_info};

fn main() {
  tauri::Builder::default()
    .plugin(tauri_plugin_dialog::init())
    .invoke_handler(tauri::generate_handler![compress_pdf, get_pdf_info])
    .run(tauri::generate_context!())
    .expect("failed to run tauri application");
}
