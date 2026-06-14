use crate::commands;
use crate::state::AppState;
use tauri::image::Image;
use tauri::menu::{Menu, MenuEvent, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconEvent};
use tauri::{App, AppHandle, Manager, Wry};

pub const TRAY_ID: &str = "main-tray";
const OPEN_ID: &str = "open";
const CANCEL_ID: &str = "cancel";
const QUIT_ID: &str = "quit";

pub struct TrayUi {
    status_item: MenuItem<Wry>,
    cancel_item: MenuItem<Wry>,
}

pub fn setup(app: &App) -> tauri::Result<()> {
    let status_item = MenuItem::with_id(app, "status", "Status: Idle", false, None::<&str>)?;
    let open_item = MenuItem::with_id(app, OPEN_ID, "Open FluxDrop", true, None::<&str>)?;
    let cancel_item =
        MenuItem::with_id(app, CANCEL_ID, "Cancel current share", false, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, QUIT_ID, "Quit FluxDrop", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&status_item, &open_item, &cancel_item, &quit_item])?;
    let tray = app
        .tray_by_id(TRAY_ID)
        .ok_or_else(|| tauri::Error::AssetNotFound(TRAY_ID.to_string()))?;
    tray.set_menu(Some(menu))?;
    tray.set_icon(Some(state_icon([100, 116, 139, 255])))?;
    app.manage(TrayUi {
        status_item,
        cancel_item,
    });
    Ok(())
}

pub fn handle_menu_event(app: &AppHandle, event: MenuEvent) {
    match event.id().as_ref() {
        OPEN_ID => show_main_window(app),
        CANCEL_ID => {
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                let state = app.state::<AppState>().inner().clone();
                let receiving = state.read().await.receive_session.is_some();
                let _ = if receiving {
                    commands::cancel_active_receive(state, app).await
                } else {
                    commands::cancel_active_share(state, app).await
                };
            });
        }
        QUIT_ID => {
            shutdown_runtime(app);
            app.exit(0);
        }
        _ => {}
    }
}

pub fn handle_tray_event(app: &AppHandle, event: TrayIconEvent) {
    if let TrayIconEvent::Click {
        button: MouseButton::Left,
        button_state: MouseButtonState::Up,
        ..
    } = event
    {
        show_main_window(app);
    }
}

pub fn update_for_event(app: &AppHandle, event: &str) {
    let (label, color, active) = match event {
        "approval_requested" => ("Awaiting approval", [217, 119, 6, 255], true),
        "download_started" | "progress_updated" => {
            ("Transfer in progress", [4, 120, 87, 255], true)
        }
        "share_created" | "phone_connected" | "download_approved" | "receive_created"
        | "upload_approved" => ("Sharing", [29, 78, 216, 255], true),
        "upload_approval_requested" => ("Awaiting approval", [217, 119, 6, 255], true),
        "upload_started" | "upload_progress" => ("Transfer in progress", [4, 120, 87, 255], true),
        _ => ("Idle", [100, 116, 139, 255], false),
    };

    if let Some(ui) = app.try_state::<TrayUi>() {
        let _ = ui.status_item.set_text(format!("Status: {label}"));
        let _ = ui.cancel_item.set_enabled(active);
    }
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let _ = tray.set_tooltip(Some(format!("FluxDrop - {label}")));
        let _ = tray.set_icon(Some(state_icon(color)));
    }
}

fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn shutdown_runtime(app: &AppHandle) {
    let state = app.state::<AppState>().inner().clone();
    tauri::async_runtime::block_on(async move {
        let server = {
            let mut guard = state.write().await;
            if let Some(share) = guard.current_share.as_mut() {
                share.cancel();
            }
            if let Some(receive) = guard.receive_session.as_mut() {
                receive.cancelled = true;
            }
            guard.server.take()
        };
        if let Some(server) = server {
            server.stop().await;
        }
    });
}

fn state_icon(accent: [u8; 4]) -> Image<'static> {
    const SIZE: u32 = 32;
    let mut rgba = vec![0_u8; (SIZE * SIZE * 4) as usize];
    for y in 0..SIZE {
        for x in 0..SIZE {
            let index = ((y * SIZE + x) * 4) as usize;
            let dx = x as i32 - 16;
            let dy = y as i32 - 16;
            let radius_sq = dx * dx + dy * dy;
            let pixel = if radius_sq <= 13 * 13 {
                accent
            } else {
                [15, 23, 42, if radius_sq <= 15 * 15 { 255 } else { 0 }]
            };
            rgba[index..index + 4].copy_from_slice(&pixel);
        }
    }
    Image::new_owned(rgba, SIZE, SIZE)
}
