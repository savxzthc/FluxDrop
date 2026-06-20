use crate::archive;
use crate::events;
use crate::file_utils::format_file_size;
use crate::history::{self, HistoryEntry, RepeatTarget};
use crate::network::{self, NetworkAddress, PREFERRED_PORT};
use crate::receive::{ReceiveInfo, ReceiveSession, ReceiveStatusInfo};
use crate::server;
use crate::settings::{self, AppSettings, ALLOWED_EXPIRATION_MINUTES};
use crate::share::{ShareInfo, ShareSession, ShareStatus, ShareStatusInfo};
use crate::state::AppState;
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
use qrcode::render::svg;
use qrcode::QrCode;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use tauri::{AppHandle, Emitter, Manager, State};

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct CreateShareOptions {
    pub expiration_minutes: Option<u32>,
    pub single_use: Option<bool>,
    pub approval_required: Option<bool>,
}

#[tauri::command]
pub async fn create_share(
    file_paths: Vec<String>,
    options: Option<CreateShareOptions>,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<ShareInfo, String> {
    create_share_impl(file_paths, options, state.inner().clone(), app).await
}

async fn create_share_impl(
    file_paths: Vec<String>,
    options: Option<CreateShareOptions>,
    app_state: AppState,
    app: AppHandle,
) -> Result<ShareInfo, String> {
    if file_paths.is_empty() || file_paths.iter().any(|path| path.trim().is_empty()) {
        return Err("Choose at least one file or folder before starting a share.".to_string());
    }

    let selected_paths = file_paths.into_iter().map(Into::into).collect();
    let prepared = tokio::task::spawn_blocking(move || archive::prepare_share(selected_paths))
        .await
        .map_err(|_| "FluxDrop could not finish inspecting the selected items.".to_string())??;

    let saved_settings = app_state.read().await.settings.clone();
    let options = options.unwrap_or_default();
    let expiration_minutes = options
        .expiration_minutes
        .unwrap_or(saved_settings.expiration_minutes);
    if !ALLOWED_EXPIRATION_MINUTES.contains(&expiration_minutes) {
        return Err("Link expiration must be 5, 10, 30, or 60 minutes.".to_string());
    }
    let single_use = options.single_use.unwrap_or(saved_settings.single_use);
    let approval_required = options
        .approval_required
        .unwrap_or(saved_settings.approval_required);
    let addresses = network::list_network_addresses();
    let ip = network::configured_ip_address(&addresses, saved_settings.preferred_lan_ip.as_deref());

    let endpoints = ensure_server(&app_state, &app, ip).await?;

    let mut session = ShareSession::new_with_payload(
        prepared.payload,
        prepared.source_paths,
        prepared.safe_file_name,
        prepared.original_file_name,
        prepared.file_size,
        prepared.mime_type,
        prepared.file_count,
        prepared.is_archive,
        expiration_minutes,
        single_use,
        approval_required,
    );
    session.status = ShareStatus::Ready;
    let download_url = format!("https://{}/d/{}", endpoints.secure, session.token);
    let qr_svg = build_qr_svg(&onboarding_url(endpoints.onboarding, &download_url))?;

    let info = ShareInfo {
        id: session.id,
        token: session.token.clone(),
        file_name: session.safe_file_name.clone(),
        file_size: session.file_size,
        file_size_human: format_file_size(session.file_size),
        mime_type: session.mime_type.clone(),
        file_count: session.file_count,
        is_archive: session.is_archive,
        download_url,
        qr_svg,
        expires_at: session.expires_at,
        local_ip: endpoints.secure.ip().to_string(),
        port: endpoints.secure.port(),
        status: session.status.clone(),
    };

    let (previous_share, previous_receive) = {
        let mut guard = app_state.write().await;
        let previous_share = guard.current_share.take().map(|mut share| {
            share.cancel();
            share
        });
        let previous_receive = guard.receive_session.take().map(|mut receive| {
            receive.cancelled = true;
            receive.status = ShareStatus::Cancelled;
            receive
        });
        guard.detected_addresses = addresses;
        guard.current_share = Some(session);
        guard.last_request_status = Some("Share created; waiting for phone.".to_string());
        (previous_share, previous_receive)
    };
    if let Some(share) = previous_share.as_ref() {
        let _ = history::record_share(&app_state, share).await;
    }
    if let Some(receive) = previous_receive.as_ref() {
        let _ = history::record_receive(&app_state, receive).await;
    }

    events::emit_share_status(Some(&app), "share_created", &info);
    Ok(info)
}

#[tauri::command]
pub async fn cancel_share(state: State<'_, AppState>, app: AppHandle) -> Result<(), String> {
    cancel_active_share(state.inner().clone(), app).await
}

pub async fn cancel_active_share(app_state: AppState, app: AppHandle) -> Result<(), String> {
    let (status_info, terminal_share) = {
        let mut guard = app_state.write().await;
        let local_address = guard
            .server
            .as_ref()
            .map(|server| server.address.to_string());
        let last_request_status = Some("Share cancelled by the PC.".to_string());
        guard.last_request_status = last_request_status.clone();
        if let Some(share) = guard.current_share.as_mut() {
            share.cancel();
            (
                Some(share.status_info(local_address, last_request_status)),
                Some(share.clone()),
            )
        } else {
            (None, None)
        }
    };

    if let Some(share) = terminal_share.as_ref() {
        let _ = history::record_share(&app_state, share).await;
    }
    if let Some(info) = status_info {
        events::emit_share_status(Some(&app), "share_cancelled", &info);
    }
    Ok(())
}

#[tauri::command]
pub async fn get_share_status(
    state: State<'_, AppState>,
) -> Result<Option<ShareStatusInfo>, String> {
    let guard = state.read().await;
    let local_address = guard
        .server
        .as_ref()
        .map(|server| server.address.to_string());
    Ok(guard
        .current_share
        .as_ref()
        .map(|share| share.status_info(local_address, guard.last_request_status.clone())))
}

#[tauri::command]
pub async fn get_network_addresses(
    state: State<'_, AppState>,
) -> Result<Vec<NetworkAddress>, String> {
    let addresses = network::list_network_addresses();
    let mut guard = state.write().await;
    guard.detected_addresses = addresses.clone();
    Ok(addresses)
}

#[tauri::command]
pub async fn start_receive(
    destination_folder: String,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<ReceiveInfo, String> {
    start_receive_impl(destination_folder, state.inner().clone(), app).await
}

async fn start_receive_impl(
    destination_folder: String,
    app_state: AppState,
    app: AppHandle,
) -> Result<ReceiveInfo, String> {
    let destination = tokio::fs::canonicalize(destination_folder)
        .await
        .map_err(|_| "The selected destination folder could not be found.".to_string())?;
    let metadata = tokio::fs::metadata(&destination)
        .await
        .map_err(|_| "FluxDrop could not read the destination folder.".to_string())?;
    if !metadata.is_dir() {
        return Err("Choose a folder where phone uploads should be saved.".to_string());
    }

    let settings = app_state.read().await.settings.clone();
    let addresses = network::list_network_addresses();
    let ip = network::configured_ip_address(&addresses, settings.preferred_lan_ip.as_deref());
    let endpoints = ensure_server(&app_state, &app, ip).await?;
    let session = ReceiveSession::new(
        destination,
        settings.expiration_minutes,
        settings.approval_required,
        settings.max_upload_bytes,
    );
    let upload_url = format!("https://{}/u/{}", endpoints.secure, session.token);
    let info = ReceiveInfo {
        id: session.id,
        token: session.token.clone(),
        upload_url: upload_url.clone(),
        qr_svg: build_qr_svg(&onboarding_url(endpoints.onboarding, &upload_url))?,
        destination_folder_name: session
            .destination_folder
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("Selected folder")
            .to_string(),
        expires_at: session.expires_at,
        local_ip: endpoints.secure.ip().to_string(),
        port: endpoints.secure.port(),
        max_upload_bytes: session.max_upload_bytes,
        max_upload_size_human: format_file_size(session.max_upload_bytes),
        status: session.status.clone(),
    };

    let (previous_share, previous_receive) = {
        let mut guard = app_state.write().await;
        let previous_share = guard.current_share.take().map(|mut share| {
            share.cancel();
            share
        });
        let previous_receive = guard.receive_session.take().map(|mut receive| {
            receive.cancelled = true;
            receive.status = ShareStatus::Cancelled;
            receive
        });
        guard.receive_session = Some(session);
        guard.detected_addresses = addresses;
        guard.last_request_status = Some("Receive mode started; waiting for phone.".to_string());
        (previous_share, previous_receive)
    };
    if let Some(share) = previous_share.as_ref() {
        let _ = history::record_share(&app_state, share).await;
    }
    if let Some(receive) = previous_receive.as_ref() {
        let _ = history::record_receive(&app_state, receive).await;
    }
    events::emit_share_status(Some(&app), "receive_created", &info);
    Ok(info)
}

#[tauri::command]
pub async fn get_receive_status(
    state: State<'_, AppState>,
) -> Result<Option<ReceiveStatusInfo>, String> {
    let guard = state.read().await;
    let local_address = guard
        .server
        .as_ref()
        .map(|server| server.address.to_string());
    Ok(guard
        .receive_session
        .as_ref()
        .map(|receive| receive.status_info(local_address, guard.last_request_status.clone())))
}

#[tauri::command]
pub async fn cancel_receive(state: State<'_, AppState>, app: AppHandle) -> Result<(), String> {
    cancel_active_receive(state.inner().clone(), app).await
}

pub async fn cancel_active_receive(app_state: AppState, app: AppHandle) -> Result<(), String> {
    let (info, terminal_receive) = {
        let mut guard = app_state.write().await;
        let local_address = guard
            .server
            .as_ref()
            .map(|server| server.address.to_string());
        let receive = guard
            .receive_session
            .as_mut()
            .ok_or_else(|| "There is no active receive link.".to_string())?;
        receive.cancelled = true;
        receive.status = ShareStatus::Cancelled;
        (
            receive.status_info(
                local_address,
                Some("Receive mode cancelled on the PC.".to_string()),
            ),
            receive.clone(),
        )
    };
    let _ = history::record_receive(&app_state, &terminal_receive).await;
    events::emit_share_status(Some(&app), "receive_cancelled", &info);
    Ok(())
}

#[tauri::command]
pub async fn get_transfer_history(state: State<'_, AppState>) -> Result<Vec<HistoryEntry>, String> {
    Ok(state
        .read()
        .await
        .history
        .iter()
        .map(|record| record.public_entry())
        .collect())
}

#[tauri::command]
pub async fn clear_transfer_history(state: State<'_, AppState>) -> Result<(), String> {
    let mut guard = state.write().await;
    if let Some(path) = guard.history_path.as_deref() {
        history::save(path, &[])?;
    }
    guard.history.clear();
    Ok(())
}

#[derive(Debug, Serialize)]
#[serde(tag = "direction", content = "transfer", rename_all = "snake_case")]
pub enum RepeatedTransfer {
    Send(ShareInfo),
    Receive(ReceiveInfo),
}

#[tauri::command]
pub async fn repeat_transfer(
    history_id: uuid::Uuid,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<RepeatedTransfer, String> {
    let app_state = state.inner().clone();
    let repeat = {
        let guard = app_state.read().await;
        guard
            .history
            .iter()
            .find(|entry| entry.id == history_id)
            .map(|entry| entry.repeat.clone())
            .ok_or_else(|| "That transfer is no longer in history.".to_string())?
    };

    match repeat {
        RepeatTarget::Send { paths } => {
            if paths.is_empty() || paths.iter().any(|path| !path.exists()) {
                return Err(
                    "One or more original files are no longer available at their saved locations."
                        .to_string(),
                );
            }
            let paths = paths
                .into_iter()
                .map(|path| path.to_string_lossy().into_owned())
                .collect();
            create_share_impl(paths, None, app_state, app)
                .await
                .map(RepeatedTransfer::Send)
        }
        RepeatTarget::Receive { destination_folder } => {
            if !destination_folder.is_dir() {
                return Err("The original destination folder is no longer available.".to_string());
            }
            start_receive_impl(
                destination_folder.to_string_lossy().into_owned(),
                app_state,
                app,
            )
            .await
            .map(RepeatedTransfer::Receive)
        }
    }
}

#[tauri::command]
pub async fn approve_upload(state: State<'_, AppState>, app: AppHandle) -> Result<(), String> {
    set_upload_approval(state.inner().clone(), app, true).await
}

#[tauri::command]
pub async fn deny_upload(state: State<'_, AppState>, app: AppHandle) -> Result<(), String> {
    set_upload_approval(state.inner().clone(), app, false).await
}

#[tauri::command]
pub async fn take_pending_shell_share(
    state: State<'_, AppState>,
) -> Result<Option<Vec<String>>, String> {
    Ok(state.write().await.ready_shell_paths.take())
}

const SHELL_SHARE_DEBOUNCE_MS: u64 = 500;

pub fn queue_shell_share(app: AppHandle, app_state: AppState, paths: Vec<String>) {
    if paths.is_empty() {
        focus_main_window(&app);
        return;
    }
    tauri::async_runtime::spawn(async move {
        let epoch = {
            let mut guard = app_state.write().await;
            guard.pending_shell_paths.extend(paths);
            guard.shell_share_epoch = guard.shell_share_epoch.wrapping_add(1);
            guard.shell_share_epoch
        };
        tokio::time::sleep(std::time::Duration::from_millis(SHELL_SHARE_DEBOUNCE_MS)).await;
        let ready = {
            let mut guard = app_state.write().await;
            if guard.shell_share_epoch != epoch || guard.pending_shell_paths.is_empty() {
                return;
            }
            let drained = std::mem::take(&mut guard.pending_shell_paths);
            guard.ready_shell_paths = Some(drained.clone());
            drained
        };
        focus_main_window(&app);
        let _ = app.emit("shell_share", ready);
    });
}

fn focus_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn apply_global_hotkey(app: &AppHandle, enabled: bool) {
    use tauri_plugin_global_shortcut::GlobalShortcutExt;
    let shortcut = crate::shell_integration::global_shortcut();
    let manager = app.global_shortcut();
    if enabled {
        let _ = manager.register(shortcut);
    } else {
        let _ = manager.unregister(shortcut);
    }
}

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> Result<AppSettings, String> {
    Ok(state.read().await.settings.clone())
}

#[tauri::command]
pub async fn update_settings(
    new_settings: AppSettings,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<AppSettings, String> {
    new_settings.validate()?;
    let app_state = state.inner().clone();
    let addresses = network::list_network_addresses();
    if let Some(configured) = new_settings.preferred_lan_ip.as_deref() {
        if !addresses.iter().any(|address| address.ip == configured) {
            return Err(
                "The selected LAN adapter is no longer available. Refresh the list and choose another."
                    .to_string(),
            );
        }
    }
    let selected_ip =
        network::configured_ip_address(&addresses, new_settings.preferred_lan_ip.as_deref());
    let (path, old_server_ip, previous_shell_integration, previous_global_hotkey) = {
        let guard = app_state.read().await;
        (
            guard.settings_path.clone(),
            guard.server.as_ref().map(|server| server.address.ip()),
            guard.settings.shell_integration,
            guard.settings.global_hotkey,
        )
    };
    let path = path.ok_or_else(|| "FluxDrop settings storage is not initialized.".to_string())?;
    settings::save(&path, &new_settings)?;

    if new_settings.shell_integration != previous_shell_integration {
        crate::shell_integration::apply_registration(new_settings.shell_integration)?;
    }
    if new_settings.global_hotkey != previous_global_hotkey {
        apply_global_hotkey(&app, new_settings.global_hotkey);
    }

    if old_server_ip.is_some_and(|current| current != selected_ip) {
        let (server, share_info, receive_info, terminal_share, terminal_receive) = {
            let mut guard = app_state.write().await;
            let old_address = guard
                .server
                .as_ref()
                .map(|server| server.address.to_string());
            let message =
                "LAN adapter changed; the active transfer was cancelled and the server restarted."
                    .to_string();
            let share_info = guard.current_share.as_mut().map(|share| {
                share.cancel();
                share.status_info(old_address.clone(), Some(message.clone()))
            });
            let terminal_share = guard.current_share.clone();
            let receive_info = guard.receive_session.as_mut().map(|receive| {
                receive.cancelled = true;
                receive.status = ShareStatus::Cancelled;
                receive.status_info(old_address, Some(message.clone()))
            });
            let terminal_receive = guard.receive_session.clone();
            guard.last_request_status = Some(message);
            (
                guard.server.take(),
                share_info,
                receive_info,
                terminal_share,
                terminal_receive,
            )
        };
        if let Some(share) = terminal_share.as_ref() {
            let _ = history::record_share(&app_state, share).await;
        }
        if let Some(receive) = terminal_receive.as_ref() {
            let _ = history::record_receive(&app_state, receive).await;
        }
        if let Some(server) = server {
            server.stop().await;
        }
        let port = network::select_available_port(selected_ip, PREFERRED_PORT)
            .map_err(|_| "FluxDrop could not find a port on the selected adapter.".to_string())?;
        let handle = server::start_server(app_state.clone(), Some(app.clone()), selected_ip, port)
            .await
            .map_err(|err| format!("FluxDrop could not restart on the selected adapter: {err}"))?;
        app_state.write().await.server = Some(handle);
        if let Some(info) = share_info {
            events::emit_share_status(Some(&app), "share_cancelled", &info);
        }
        if let Some(info) = receive_info {
            events::emit_share_status(Some(&app), "receive_cancelled", &info);
        }
    }

    let mut guard = app_state.write().await;
    guard.settings = new_settings.clone();
    guard.detected_addresses = addresses;
    Ok(new_settings)
}

#[tauri::command]
pub async fn approve_download(state: State<'_, AppState>, app: AppHandle) -> Result<(), String> {
    set_approval_state(state.inner().clone(), app, true).await
}

#[tauri::command]
pub async fn deny_download(state: State<'_, AppState>, app: AppHandle) -> Result<(), String> {
    set_approval_state(state.inner().clone(), app, false).await
}

fn build_qr_svg(download_url: &str) -> Result<String, String> {
    let code = QrCode::new(download_url.as_bytes())
        .map_err(|_| "FluxDrop could not generate the QR code.".to_string())?;
    Ok(code
        .render::<svg::Color>()
        .min_dimensions(360, 360)
        .quiet_zone(true)
        .dark_color(svg::Color("#0f172a"))
        .light_color(svg::Color("#ffffff"))
        .build())
}

#[derive(Debug, Clone, Copy)]
struct ServerEndpoints {
    secure: SocketAddr,
    onboarding: SocketAddr,
}

fn onboarding_url(onboarding: SocketAddr, secure_url: &str) -> String {
    format!(
        "http://{onboarding}/connect#{}",
        utf8_percent_encode(secure_url, NON_ALPHANUMERIC)
    )
}

async fn ensure_server(
    app_state: &AppState,
    app: &AppHandle,
    ip: std::net::IpAddr,
) -> Result<ServerEndpoints, String> {
    if let Some(endpoints) = app_state
        .read()
        .await
        .server
        .as_ref()
        .map(|server| ServerEndpoints {
            secure: server.address,
            onboarding: server.onboarding_address,
        })
    {
        return Ok(endpoints);
    }
    let port = network::select_available_port(ip, PREFERRED_PORT)
        .map_err(|_| "FluxDrop could not find an available local port.".to_string())?;
    let handle = server::start_server(app_state.clone(), Some(app.clone()), ip, port)
        .await
        .map_err(|err| format!("FluxDrop could not start the local server: {err}"))?;
    let endpoints = ServerEndpoints {
        secure: handle.address,
        onboarding: handle.onboarding_address,
    };
    app_state.write().await.server = Some(handle);
    Ok(endpoints)
}

async fn set_approval_state(
    app_state: AppState,
    app: AppHandle,
    approved: bool,
) -> Result<(), String> {
    let (status_info, terminal_share) = {
        let mut guard = app_state.write().await;
        let local_address = guard
            .server
            .as_ref()
            .map(|server| server.address.to_string());
        let last_request_status = Some(if approved {
            "Download approved on the PC.".to_string()
        } else {
            "Download denied on the PC.".to_string()
        });
        guard.last_request_status = last_request_status.clone();

        let share = guard
            .current_share
            .as_mut()
            .ok_or_else(|| "There is no active share to approve or deny.".to_string())?;
        if !matches!(share.status, ShareStatus::AwaitingApproval) {
            return Err("There is no pending download request to approve or deny.".to_string());
        }
        if approved {
            share.approve();
        } else {
            share.deny(false);
        }
        (
            share.status_info(local_address, last_request_status),
            (!approved).then(|| share.clone()),
        )
    };

    if let Some(share) = terminal_share.as_ref() {
        let _ = history::record_share(&app_state, share).await;
    }
    events::emit_share_status(
        Some(&app),
        if approved {
            "download_approved"
        } else {
            "download_denied"
        },
        &status_info,
    );
    Ok(())
}

async fn set_upload_approval(
    app_state: AppState,
    app: AppHandle,
    approved: bool,
) -> Result<(), String> {
    let (info, terminal_receive) = {
        let mut guard = app_state.write().await;
        let local_address = guard
            .server
            .as_ref()
            .map(|server| server.address.to_string());
        let receive = guard
            .receive_session
            .as_mut()
            .ok_or_else(|| "There is no active upload request.".to_string())?;
        if !matches!(receive.status, ShareStatus::AwaitingApproval) {
            return Err("There is no pending upload request to approve or deny.".to_string());
        }
        if approved {
            receive.approve();
        } else {
            receive.deny(false);
        }
        (
            receive.status_info(
                local_address,
                Some(if approved {
                    "Phone upload approved on the PC.".to_string()
                } else {
                    "Phone upload denied on the PC.".to_string()
                }),
            ),
            (!approved).then(|| receive.clone()),
        )
    };
    if let Some(receive) = terminal_receive.as_ref() {
        let _ = history::record_receive(&app_state, receive).await;
    }
    events::emit_share_status(
        Some(&app),
        if approved {
            "upload_approved"
        } else {
            "upload_denied"
        },
        &info,
    );
    Ok(())
}
