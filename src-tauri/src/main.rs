#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use fluxdrop_lib::state::AppState;
use tauri::{Manager, WindowEvent};

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            fluxdrop_lib::commands::create_share,
            fluxdrop_lib::commands::cancel_share,
            fluxdrop_lib::commands::get_share_status,
            fluxdrop_lib::commands::get_network_addresses,
            fluxdrop_lib::commands::start_receive,
            fluxdrop_lib::commands::get_receive_status,
            fluxdrop_lib::commands::cancel_receive,
            fluxdrop_lib::commands::approve_upload,
            fluxdrop_lib::commands::deny_upload,
            fluxdrop_lib::commands::get_settings,
            fluxdrop_lib::commands::update_settings,
            fluxdrop_lib::commands::approve_download,
            fluxdrop_lib::commands::deny_download
        ])
        .setup(|app| {
            let settings_path = app
                .path()
                .app_config_dir()?
                .join(fluxdrop_lib::settings::SETTINGS_FILE_NAME);
            let settings = fluxdrop_lib::settings::load(&settings_path).unwrap_or_else(|err| {
                eprintln!("{err}; secure defaults will be used for this run.");
                fluxdrop_lib::settings::AppSettings::default()
            });
            let state = AppState::with_settings(settings, Some(settings_path));
            app.manage(state.clone());
            fluxdrop_lib::tray::setup(app)?;
            fluxdrop_lib::share::spawn_expiration_task(state.clone(), app.handle().clone());
            Ok(())
        })
        .on_menu_event(fluxdrop_lib::tray::handle_menu_event)
        .on_tray_icon_event(fluxdrop_lib::tray::handle_tray_event)
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .run(tauri::generate_context!())
        .expect("failed to run FluxDrop");
}
