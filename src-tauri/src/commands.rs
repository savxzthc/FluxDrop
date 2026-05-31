use crate::events;
use crate::file_utils::{format_file_size, sanitize_filename};
use crate::network::{self, NetworkAddress, PREFERRED_PORT};
use crate::server;
use crate::share::{ShareInfo, ShareSession, ShareStatus, ShareStatusInfo};
use crate::state::AppState;
use qrcode::render::svg;
use qrcode::QrCode;
use std::path::PathBuf;
use tauri::{AppHandle, State};

#[tauri::command]
pub async fn create_share(
    file_path: String,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<ShareInfo, String> {
    if file_path.trim().is_empty() {
        return Err("Choose a file before starting a share.".to_string());
    }

    let canonical_path = validate_file_path(&file_path).await?;
    let metadata = tokio::fs::metadata(&canonical_path)
        .await
        .map_err(|_| "FluxDrop could not read the selected file metadata.".to_string())?;
    let original_file_name = canonical_path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "The selected file does not have a readable name.".to_string())?
        .to_string();
    let safe_file_name = sanitize_filename(&original_file_name);
    let file_size = metadata.len();
    let mime_type = mime_guess::from_path(&canonical_path)
        .first_or_octet_stream()
        .essence_str()
        .to_string();

    let app_state = state.inner().clone();
    let addresses = network::list_network_addresses();
    let ip = network::preferred_ip_address(&addresses);

    let address = {
        let existing = app_state
            .read()
            .await
            .server
            .as_ref()
            .map(|server| server.address);
        if let Some(address) = existing {
            address
        } else {
            let port = network::select_available_port(ip, PREFERRED_PORT)
                .map_err(|_| "FluxDrop could not find an available local port.".to_string())?;
            let handle = server::start_server(app_state.clone(), Some(app.clone()), ip, port)
                .await
                .map_err(|err| format!("FluxDrop could not start the local server: {err}"))?;
            let address = handle.address;
            let mut guard = app_state.write().await;
            guard.server = Some(handle);
            address
        }
    };

    let mut session = ShareSession::new(
        canonical_path,
        safe_file_name,
        original_file_name,
        file_size,
        mime_type,
    );
    session.status = ShareStatus::Ready;
    let download_url = format!(
        "http://{}:{}/d/{}",
        address.ip(),
        address.port(),
        session.token
    );
    let qr_svg = build_qr_svg(&download_url)?;

    let info = ShareInfo {
        id: session.id,
        token: session.token.clone(),
        file_name: session.safe_file_name.clone(),
        file_size: session.file_size,
        file_size_human: format_file_size(session.file_size),
        mime_type: session.mime_type.clone(),
        download_url,
        qr_svg,
        expires_at: session.expires_at,
        local_ip: address.ip().to_string(),
        port: address.port(),
        status: session.status.clone(),
    };

    {
        let mut guard = app_state.write().await;
        guard.detected_addresses = addresses;
        guard.current_share = Some(session);
        guard.last_request_status = Some("Share created; waiting for phone.".to_string());
    }

    events::emit_share_status(Some(&app), "share_created", &info);
    Ok(info)
}

#[tauri::command]
pub async fn cancel_share(state: State<'_, AppState>, app: AppHandle) -> Result<(), String> {
    let app_state = state.inner().clone();
    let status_info = {
        let mut guard = app_state.write().await;
        let local_address = guard
            .server
            .as_ref()
            .map(|server| server.address.to_string());
        let last_request_status = Some("Share cancelled by the PC.".to_string());
        guard.last_request_status = last_request_status.clone();
        if let Some(share) = guard.current_share.as_mut() {
            share.cancel();
            Some(share.status_info(local_address, last_request_status))
        } else {
            None
        }
    };

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
pub async fn approve_download(state: State<'_, AppState>, app: AppHandle) -> Result<(), String> {
    set_approval_state(state.inner().clone(), app, true).await
}

#[tauri::command]
pub async fn deny_download(state: State<'_, AppState>, app: AppHandle) -> Result<(), String> {
    set_approval_state(state.inner().clone(), app, false).await
}

async fn validate_file_path(file_path: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(file_path);
    let canonical_path = tokio::fs::canonicalize(path)
        .await
        .map_err(|_| "The selected file could not be found.".to_string())?;
    let metadata = tokio::fs::metadata(&canonical_path)
        .await
        .map_err(|_| "FluxDrop could not read the selected file.".to_string())?;

    if metadata.is_dir() {
        return Err(
            "Choose a single file. Folders are not supported in FluxDrop v0.1.".to_string(),
        );
    }
    if !metadata.is_file() {
        return Err("The selected item is not a regular file.".to_string());
    }
    tokio::fs::File::open(&canonical_path)
        .await
        .map_err(|_| "FluxDrop does not have permission to read the selected file.".to_string())?;
    Ok(canonical_path)
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

async fn set_approval_state(
    app_state: AppState,
    app: AppHandle,
    approved: bool,
) -> Result<(), String> {
    let status_info = {
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
        share.approved = approved;
        share.status = if approved {
            ShareStatus::Approved
        } else {
            ShareStatus::Denied
        };
        share.status_info(local_address, last_request_status)
    };

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
