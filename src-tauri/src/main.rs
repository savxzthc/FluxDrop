#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use fluxdrop_lib::state::AppState;

fn main() {
    let state = AppState::new();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(state.clone())
        .invoke_handler(tauri::generate_handler![
            fluxdrop_lib::commands::create_share,
            fluxdrop_lib::commands::cancel_share,
            fluxdrop_lib::commands::get_share_status,
            fluxdrop_lib::commands::get_network_addresses,
            fluxdrop_lib::commands::approve_download,
            fluxdrop_lib::commands::deny_download
        ])
        .setup(move |app| {
            fluxdrop_lib::share::spawn_expiration_task(state.clone(), app.handle().clone());
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("failed to run FluxDrop");
}
