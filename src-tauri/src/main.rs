#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use fluxdrop_lib::shell_integration;
use fluxdrop_lib::state::AppState;
use tauri::{Emitter, Manager, WindowEvent};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            let paths = shell_integration::extract_send_paths(&args);
            if let Some(state) = app.try_state::<AppState>() {
                fluxdrop_lib::commands::queue_shell_share(
                    app.clone(),
                    state.inner().clone(),
                    paths,
                );
            } else if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    if shortcut == &shell_integration::global_shortcut()
                        && event.state() == ShortcutState::Pressed
                    {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.unminimize();
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                        let _ = app.emit("shell_focus", ());
                    }
                })
                .build(),
        )
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
            fluxdrop_lib::commands::get_transfer_history,
            fluxdrop_lib::commands::clear_transfer_history,
            fluxdrop_lib::commands::repeat_transfer,
            fluxdrop_lib::commands::approve_download,
            fluxdrop_lib::commands::deny_download,
            fluxdrop_lib::commands::take_pending_shell_share
        ])
        .setup(|app| {
            let config_dir = app.path().app_config_dir()?;
            let settings_path = config_dir.join(fluxdrop_lib::settings::SETTINGS_FILE_NAME);
            let history_path = config_dir.join(fluxdrop_lib::history::HISTORY_FILE_NAME);
            let settings = fluxdrop_lib::settings::load(&settings_path).unwrap_or_else(|err| {
                eprintln!("{err}; secure defaults will be used for this run.");
                fluxdrop_lib::settings::AppSettings::default()
            });
            let history = fluxdrop_lib::history::load(&history_path).unwrap_or_else(|err| {
                eprintln!("{err}; transfer history will start empty for this run.");
                Vec::new()
            });
            let shell_integration_enabled = settings.shell_integration;
            let global_hotkey_enabled = settings.global_hotkey;
            let state =
                AppState::with_storage(settings, Some(settings_path), history, Some(history_path));
            app.manage(state.clone());

            let startup_paths =
                shell_integration::extract_send_paths(&std::env::args().collect::<Vec<_>>());
            if !startup_paths.is_empty() {
                tauri::async_runtime::block_on(async {
                    state.write().await.ready_shell_paths = Some(startup_paths);
                });
            }
            if shell_integration_enabled {
                if let Err(err) = shell_integration::apply_registration(true) {
                    eprintln!("{err}; the right-click Send menu was not refreshed.");
                }
            }
            if global_hotkey_enabled {
                let _ = app
                    .global_shortcut()
                    .register(shell_integration::global_shortcut());
            }

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
