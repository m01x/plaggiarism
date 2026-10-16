mod commands;
mod robocopy;
mod state;

use commands::{
    cancel_robocopy, disk_free_space, poll_scan, read_text_file, run_robocopy, scan_robocopy,
    validate_paths, write_text_file, ScanHolder,
};
use state::shared_child;
use state::RobocopyState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(RobocopyState {
            child: shared_child(),
        })
        .manage(ScanHolder::default())
        .invoke_handler(tauri::generate_handler![
            validate_paths,
            scan_robocopy,
            poll_scan,
            run_robocopy,
            cancel_robocopy,
            disk_free_space,
            write_text_file,
            read_text_file,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
