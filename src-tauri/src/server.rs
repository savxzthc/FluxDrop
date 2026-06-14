use crate::events;
use crate::events::EventHandle;
use crate::file_utils::{
    content_disposition_filename, escape_html, format_file_size, sanitize_filename,
};
use crate::receive::{ReceiveSession, ReceiveStatusInfo};
use crate::share::{ArchiveEntrySource, SharePayload, ShareSession, ShareStatus, ShareStatusInfo};
use crate::state::{AppState, ServerHandle};
use crate::tls;
use async_stream::stream;
use async_zip::{Compression, ZipEntryBuilder};
use axum::body::Body;
use axum::extract::{ConnectInfo, DefaultBodyLimit, Multipart, Path, State};
use axum::http::{header, HeaderMap, HeaderName, HeaderValue, Request, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use bytes::Bytes;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::io;
use std::net::{IpAddr, SocketAddr, TcpListener as StdTcpListener};
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_util::io::ReaderStream;
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
pub struct HttpState {
    pub app_state: AppState,
    pub app: Option<EventHandle>,
}

#[derive(Debug, Clone)]
struct ShareSnapshot {
    token: String,
    payload: SharePayload,
    safe_file_name: String,
    file_size: u64,
    file_size_human: String,
    mime_type: String,
    expires_at: DateTime<Utc>,
    single_use: bool,
    sender_name: String,
    status: ShareStatus,
    approval_required: bool,
    file_count: usize,
    is_archive: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InvalidReason {
    NotFound,
    Expired,
    Cancelled,
    Completed,
    Denied,
    ApprovalTimedOut,
    ApprovalRequired,
    RateLimited,
}

#[derive(Serialize)]
struct ShareMetadata {
    app: &'static str,
    file_name: String,
    file_size: u64,
    file_size_human: String,
    mime_type: String,
    expires_at: DateTime<Utc>,
    single_use: bool,
    sender_name: String,
    status: ShareStatus,
    file_count: usize,
    is_archive: bool,
}

#[derive(Serialize)]
struct ErrorBody {
    error: &'static str,
    message: &'static str,
}

#[derive(Debug, Deserialize)]
struct UploadRequestMetadata {
    file_name: String,
    file_size: u64,
    mime_type: Option<String>,
}

#[derive(Debug, Clone)]
struct ReceiveSnapshot {
    token: String,
    destination_folder: std::path::PathBuf,
    max_upload_bytes: u64,
    file_name: Option<String>,
    declared_size: Option<u64>,
}

pub async fn start_server(
    app_state: AppState,
    app: Option<EventHandle>,
    ip: IpAddr,
    port: u16,
) -> std::io::Result<ServerHandle> {
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
    let config_dir = app_state
        .read()
        .await
        .settings_path
        .as_deref()
        .and_then(std::path::Path::parent)
        .map(std::path::Path::to_path_buf)
        .ok_or_else(|| io::Error::other("FluxDrop settings storage is not initialized"))?;
    let certificate = tls::ensure_certificate(&config_dir, ip).map_err(io::Error::other)?;
    let rustls_config = axum_server::tls_rustls::RustlsConfig::from_pem_file(
        certificate.cert_path,
        certificate.key_path,
    )
    .await?;

    let listener = StdTcpListener::bind(SocketAddr::new(ip, port))?;
    listener.set_nonblocking(true)?;
    let address = listener.local_addr()?;
    let onboarding_listener = StdTcpListener::bind(SocketAddr::new(ip, 0))?;
    onboarding_listener.set_nonblocking(true)?;
    let onboarding_address = onboarding_listener.local_addr()?;

    let shutdown = CancellationToken::new();
    let router = build_router(app_state, app);
    let onboarding_router = build_onboarding_router();
    let tls_handle = axum_server::Handle::new();
    let onboarding_handle = axum_server::Handle::new();
    let tls_server = axum_server::from_tcp_rustls(listener, rustls_config)?;
    let onboarding_server = axum_server::from_tcp(onboarding_listener)?;
    let tls_shutdown = shutdown.clone();
    let onboarding_shutdown = shutdown.clone();

    #[cfg(not(test))]
    let task = tauri::async_runtime::spawn(async move {
        let shutdown_handle = tls_handle.clone();
        tokio::spawn(async move {
            tls_shutdown.cancelled().await;
            shutdown_handle.graceful_shutdown(Some(Duration::from_secs(5)));
        });
        let service = router.into_make_service_with_connect_info::<SocketAddr>();
        let result = tls_server.handle(tls_handle).serve(service).await;
        if let Err(err) = result {
            eprintln!("FluxDrop HTTPS server stopped unexpectedly: {err}");
        }
    });

    #[cfg(test)]
    let task = tokio::spawn(async move {
        let shutdown_handle = tls_handle.clone();
        tokio::spawn(async move {
            tls_shutdown.cancelled().await;
            shutdown_handle.graceful_shutdown(Some(Duration::from_secs(5)));
        });
        let service = router.into_make_service_with_connect_info::<SocketAddr>();
        let result = tls_server.handle(tls_handle).serve(service).await;
        if let Err(err) = result {
            eprintln!("FluxDrop HTTPS server stopped unexpectedly: {err}");
        }
    });

    #[cfg(not(test))]
    let onboarding_task = tauri::async_runtime::spawn(async move {
        let shutdown_handle = onboarding_handle.clone();
        tokio::spawn(async move {
            onboarding_shutdown.cancelled().await;
            shutdown_handle.graceful_shutdown(Some(Duration::from_secs(5)));
        });
        let service = onboarding_router.into_make_service_with_connect_info::<SocketAddr>();
        let result = onboarding_server
            .handle(onboarding_handle)
            .serve(service)
            .await;
        if let Err(err) = result {
            eprintln!("FluxDrop certificate onboarding server stopped unexpectedly: {err}");
        }
    });

    #[cfg(test)]
    let onboarding_task = tokio::spawn(async move {
        let shutdown_handle = onboarding_handle.clone();
        tokio::spawn(async move {
            onboarding_shutdown.cancelled().await;
            shutdown_handle.graceful_shutdown(Some(Duration::from_secs(5)));
        });
        let service = onboarding_router.into_make_service_with_connect_info::<SocketAddr>();
        let result = onboarding_server
            .handle(onboarding_handle)
            .serve(service)
            .await;
        if let Err(err) = result {
            eprintln!("FluxDrop certificate onboarding server stopped unexpectedly: {err}");
        }
    });

    Ok(ServerHandle {
        address,
        onboarding_address,
        shutdown,
        task: Some(task),
        onboarding_task: Some(onboarding_task),
    })
}

pub fn build_router(app_state: AppState, app: Option<EventHandle>) -> Router {
    Router::new()
        .route("/", get(root))
        .route("/d/:token", get(download_page))
        .route("/api/share/:token", get(share_api))
        .route("/download/:token", get(download_file).head(download_head))
        .route("/u/:token", get(upload_page))
        .route("/upload.js", get(upload_script))
        .route("/api/upload-request/:token", post(upload_request))
        .route("/api/upload-status/:token", get(upload_status))
        .route("/upload/:token", post(upload_file))
        .route("/health", get(health))
        .with_state(HttpState { app_state, app })
        .layer(DefaultBodyLimit::disable())
        .layer(middleware::from_fn(security_headers))
}

pub fn build_onboarding_router() -> Router {
    Router::new()
        .route("/connect", get(onboarding_page))
        .route("/connect.js", get(onboarding_script))
        .route("/health", get(health))
        .layer(middleware::from_fn(security_headers))
}

async fn root() -> Html<&'static str> {
    Html("<!doctype html><html><head><meta charset=\"utf-8\"><title>FluxDrop</title></head><body><h1>FluxDrop is running</h1><p>The local transfer server is ready.</p></body></html>")
}

async fn onboarding_page() -> Html<&'static str> {
    Html(ONBOARDING_HTML)
}

async fn onboarding_script() -> Response {
    (
        [(
            header::CONTENT_TYPE,
            "application/javascript; charset=utf-8",
        )],
        ONBOARDING_SCRIPT,
    )
        .into_response()
}

async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "status": "ok" }))
}

async fn download_page(
    State(state): State<HttpState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Path(token): Path<String>,
) -> Response {
    match validate_token(&state, &token, addr.ip(), false).await {
        Ok(_) => {
            let (snapshot, approval_requested) = {
                let mut guard = state.app_state.write().await;
                let local_address = guard
                    .server
                    .as_ref()
                    .map(|server| server.address.to_string());
                let request_status = "Phone opened the download page.".to_string();
                guard.last_request_status = Some(request_status.clone());
                let share = guard
                    .current_share
                    .as_mut()
                    .expect("share validated before connection update");
                let approval_requested = share.mark_phone_connected(addr.ip());
                let info = share.status_info(local_address, Some(request_status));
                let snapshot = snapshot_from_share(share);
                events::emit_share_status(
                    state.app.as_ref(),
                    if approval_requested {
                        "approval_requested"
                    } else {
                        "phone_connected"
                    },
                    &info,
                );
                (snapshot, approval_requested)
            };
            if approval_requested {
                spawn_approval_timeout(state.clone(), snapshot.token.clone());
            }
            Html(mobile_download_html(&snapshot)).into_response()
        }
        Err(reason) => error_page_response(reason),
    }
}

async fn share_api(
    State(state): State<HttpState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Path(token): Path<String>,
) -> Response {
    match validate_token(&state, &token, addr.ip(), false).await {
        Ok(snapshot) => Json(ShareMetadata {
            app: "FluxDrop",
            file_name: snapshot.safe_file_name,
            file_size: snapshot.file_size,
            file_size_human: snapshot.file_size_human,
            mime_type: snapshot.mime_type,
            expires_at: snapshot.expires_at,
            single_use: snapshot.single_use,
            sender_name: snapshot.sender_name,
            status: snapshot.status,
            file_count: snapshot.file_count,
            is_archive: snapshot.is_archive,
        })
        .into_response(),
        Err(_) => (
            StatusCode::NOT_FOUND,
            Json(ErrorBody {
                error: "not_found",
                message: "This link is invalid or has expired.",
            }),
        )
            .into_response(),
    }
}

async fn download_head(
    State(state): State<HttpState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Path(token): Path<String>,
) -> Response {
    match validate_token(&state, &token, addr.ip(), true).await {
        Ok(snapshot) => response_builder_for_download(&snapshot)
            .body(Body::empty())
            .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response()),
        Err(reason) => error_page_response(reason),
    }
}

async fn download_file(
    State(state): State<HttpState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Path(token): Path<String>,
) -> Response {
    let snapshot = match validate_token(&state, &token, addr.ip(), true).await {
        Ok(snapshot) => snapshot,
        Err(reason) => return error_page_response(reason),
    };

    {
        let mut guard = state.app_state.write().await;
        let local_address = guard
            .server
            .as_ref()
            .map(|server| server.address.to_string());
        if let Some(share) = guard.current_share.as_mut() {
            share.mark_download_started(addr.ip());
            let info = share.status_info(local_address, Some("Download started.".to_string()));
            guard.last_request_status = Some("Download started.".to_string());
            events::emit_share_status(state.app.as_ref(), "download_started", &info);
        }
    }

    let body = match snapshot.payload.clone() {
        SharePayload::SingleFile { path } => {
            let file = match tokio::fs::File::open(path).await {
                Ok(file) => file,
                Err(_) => {
                    set_error_status(
                        &state,
                        "FluxDrop could not open the selected file for download.",
                    )
                    .await;
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Html(error_html(
                            "Transfer Error",
                            "FluxDrop could not open the file.",
                        )),
                    )
                        .into_response();
                }
            };
            single_file_body(state.clone(), &snapshot, file)
        }
        SharePayload::ZipArchive { entries } => zip_archive_body(state.clone(), &snapshot, entries),
    };

    response_builder_for_download(&snapshot)
        .body(body)
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

async fn upload_page(
    State(state): State<HttpState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Path(token): Path<String>,
) -> Response {
    match validate_receive_token(&state, &token, addr.ip(), false).await {
        Ok(snapshot) => {
            let mut guard = state.app_state.write().await;
            let local_address = guard
                .server
                .as_ref()
                .map(|server| server.address.to_string());
            guard.last_request_status = Some("Phone opened the upload page.".to_string());
            if let Some(receive) = guard.receive_session.as_mut() {
                receive.client_ip = Some(addr.ip());
                if matches!(receive.status, ShareStatus::Ready) {
                    receive.status = ShareStatus::PhoneConnected;
                }
                let info = receive.status_info(
                    local_address,
                    Some("Phone opened the upload page.".to_string()),
                );
                events::emit_share_status(state.app.as_ref(), "upload_phone_connected", &info);
            }
            Html(mobile_upload_html(&snapshot)).into_response()
        }
        Err(reason) => error_page_response(reason),
    }
}

async fn upload_script() -> Response {
    (
        [(
            header::CONTENT_TYPE,
            "application/javascript; charset=utf-8",
        )],
        UPLOAD_SCRIPT,
    )
        .into_response()
}

async fn upload_request(
    State(state): State<HttpState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Path(token): Path<String>,
    Json(metadata): Json<UploadRequestMetadata>,
) -> Response {
    let snapshot = match validate_receive_token(&state, &token, addr.ip(), false).await {
        Ok(snapshot) => snapshot,
        Err(reason) => return api_error_response(reason),
    };
    if metadata.file_name.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "missing_filename"})),
        )
            .into_response();
    }
    if metadata.file_size > snapshot.max_upload_bytes {
        return (
            StatusCode::PAYLOAD_TOO_LARGE,
            Json(serde_json::json!({
                "error": "too_large",
                "message": format!("The selected file exceeds the {} limit.", format_file_size(snapshot.max_upload_bytes))
            })),
        )
            .into_response();
    }

    let safe_name = sanitize_filename(&metadata.file_name);
    let (info, approval_requested) = {
        let mut guard = state.app_state.write().await;
        let local_address = guard
            .server
            .as_ref()
            .map(|server| server.address.to_string());
        let receive = guard
            .receive_session
            .as_mut()
            .expect("receive session validated before metadata update");
        if matches!(
            receive.status,
            ShareStatus::AwaitingApproval | ShareStatus::Uploading | ShareStatus::Completed
        ) {
            return (
                StatusCode::CONFLICT,
                Json(serde_json::json!({"error": "request_in_progress"})),
            )
                .into_response();
        }
        let approval_requested =
            receive.request_approval(addr.ip(), safe_name, metadata.file_size, metadata.mime_type);
        let message = if approval_requested {
            "Phone requested approval to upload a file."
        } else {
            "Phone upload request accepted without approval."
        }
        .to_string();
        let info = receive.status_info(local_address, Some(message.clone()));
        guard.last_request_status = Some(message);
        (info, approval_requested)
    };

    events::emit_share_status(
        state.app.as_ref(),
        if approval_requested {
            "upload_approval_requested"
        } else {
            "upload_approved"
        },
        &info,
    );
    if approval_requested {
        spawn_upload_approval_timeout(state.clone(), token);
    }
    (StatusCode::ACCEPTED, Json(info)).into_response()
}

async fn upload_status(
    State(state): State<HttpState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Path(token): Path<String>,
) -> Response {
    if !valid_token_shape(&token) {
        return api_error_response(InvalidReason::NotFound);
    }
    let mut guard = state.app_state.write().await;
    let receive = match guard.receive_session.as_ref() {
        Some(receive) if receive.token == token => receive,
        _ => {
            if !guard.rate_limiter.check_invalid_attempt(addr.ip()) {
                return api_error_response(InvalidReason::RateLimited);
            }
            return api_error_response(InvalidReason::NotFound);
        }
    };
    let reason = if receive.cancelled {
        Some(InvalidReason::Cancelled)
    } else if receive.is_expired() {
        Some(InvalidReason::Expired)
    } else {
        None
    };
    if let Some(reason) = reason {
        return api_error_response(reason);
    }
    guard.rate_limiter.clear(addr.ip());
    let local_address = guard
        .server
        .as_ref()
        .map(|server| server.address.to_string());
    Json(
        guard
            .receive_session
            .as_ref()
            .expect("receive checked")
            .status_info(local_address, guard.last_request_status.clone()),
    )
    .into_response()
}

async fn upload_file(
    State(state): State<HttpState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Path(token): Path<String>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Response {
    let snapshot = match validate_receive_token(&state, &token, addr.ip(), true).await {
        Ok(snapshot) => snapshot,
        Err(reason) => return api_error_response(reason),
    };
    let overhead_allowance = 1024 * 1024_u64;
    if headers
        .get(header::CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .is_some_and(|length| length > snapshot.max_upload_bytes.saturating_add(overhead_allowance))
    {
        return (
            StatusCode::PAYLOAD_TOO_LARGE,
            Json(serde_json::json!({"error": "too_large"})),
        )
            .into_response();
    }

    {
        let mut guard = state.app_state.write().await;
        let local_address = guard
            .server
            .as_ref()
            .map(|server| server.address.to_string());
        if let Some(receive) = guard.receive_session.as_mut() {
            receive.status = ShareStatus::Uploading;
            receive.bytes_received = 0;
            let info =
                receive.status_info(local_address, Some("Phone upload started.".to_string()));
            events::emit_share_status(state.app.as_ref(), "upload_started", &info);
        }
    }

    let field = loop {
        match multipart.next_field().await {
            Ok(Some(field)) if field.name() == Some("file") => break field,
            Ok(Some(_)) => continue,
            Ok(None) => {
                set_upload_error(&state, "The phone did not include a file.").await;
                return (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({"error": "missing_file"})),
                )
                    .into_response();
            }
            Err(_) => {
                set_upload_error(&state, "FluxDrop could not parse the upload body.").await;
                return (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({"error": "invalid_multipart"})),
                )
                    .into_response();
            }
        }
    };

    let upload_name = sanitize_filename(field.file_name().unwrap_or("upload"));
    if snapshot.file_name.as_deref() != Some(upload_name.as_str()) {
        set_upload_error(
            &state,
            "The uploaded filename did not match the approved request.",
        )
        .await;
        return (
            StatusCode::CONFLICT,
            Json(serde_json::json!({"error": "metadata_mismatch"})),
        )
            .into_response();
    }
    let final_path = unique_destination_path(&snapshot.destination_folder, &upload_name).await;
    let named_temp = match tempfile::Builder::new()
        .prefix(".fluxdrop-upload-")
        .suffix(".part")
        .tempfile_in(&snapshot.destination_folder)
    {
        Ok(file) => file,
        Err(_) => {
            set_upload_error(&state, "FluxDrop could not create a temporary upload file.").await;
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };
    let (std_file, temp_path) = named_temp.into_parts();
    let mut output = tokio::fs::File::from_std(std_file);
    let mut field = field;
    let mut received = 0_u64;
    let mut last_emit = Instant::now() - Duration::from_millis(250);

    loop {
        let chunk = match field.chunk().await {
            Ok(Some(chunk)) => chunk,
            Ok(None) => break,
            Err(_) => {
                drop(output);
                drop(temp_path);
                set_upload_error(&state, "The phone upload was interrupted.").await;
                return (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({"error": "interrupted"})),
                )
                    .into_response();
            }
        };
        received = received.saturating_add(chunk.len() as u64);
        if received > snapshot.max_upload_bytes {
            drop(output);
            drop(temp_path);
            set_upload_error(
                &state,
                "The phone upload exceeded the configured size limit.",
            )
            .await;
            return (
                StatusCode::PAYLOAD_TOO_LARGE,
                Json(serde_json::json!({"error": "too_large"})),
            )
                .into_response();
        }
        if output.write_all(&chunk).await.is_err() {
            drop(output);
            drop(temp_path);
            set_upload_error(&state, "FluxDrop could not write the incoming file.").await;
            return StatusCode::INSUFFICIENT_STORAGE.into_response();
        }
        if last_emit.elapsed() >= Duration::from_millis(150) {
            emit_upload_progress(&state, received).await;
            last_emit = Instant::now();
        }
    }

    if snapshot.declared_size != Some(received) {
        drop(output);
        drop(temp_path);
        set_upload_error(
            &state,
            "The uploaded size did not match the approved request.",
        )
        .await;
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "size_mismatch"})),
        )
            .into_response();
    }
    if output.flush().await.is_err() || output.sync_all().await.is_err() {
        drop(output);
        drop(temp_path);
        set_upload_error(
            &state,
            "FluxDrop could not finish writing the incoming file.",
        )
        .await;
        return StatusCode::INSUFFICIENT_STORAGE.into_response();
    }
    drop(output);
    let persist_result =
        tokio::task::spawn_blocking(move || temp_path.persist_noclobber(&final_path)).await;
    if !matches!(persist_result, Ok(Ok(_))) {
        set_upload_error(
            &state,
            "FluxDrop could not safely move the upload into the destination folder.",
        )
        .await;
        return StatusCode::CONFLICT.into_response();
    }

    let info = mark_upload_completed(&state.app_state, &token, received).await;
    if let Some(info) = info {
        events::emit_share_status(state.app.as_ref(), "upload_completed", &info);
    }
    Json(serde_json::json!({"status": "ok", "file_name": upload_name})).into_response()
}

async fn validate_receive_token(
    state: &HttpState,
    token: &str,
    client_ip: IpAddr,
    require_approval: bool,
) -> Result<ReceiveSnapshot, InvalidReason> {
    if !valid_token_shape(token) {
        return record_invalid(state, client_ip, InvalidReason::NotFound).await;
    }
    let mut guard = state.app_state.write().await;
    let reason = match guard.receive_session.as_ref() {
        None => Some(InvalidReason::NotFound),
        Some(receive) if receive.token != token => Some(InvalidReason::NotFound),
        Some(receive) if receive.cancelled => Some(InvalidReason::Cancelled),
        Some(receive) if receive.is_expired() => Some(InvalidReason::Expired),
        Some(receive) if matches!(receive.status, ShareStatus::Completed) => {
            Some(InvalidReason::Completed)
        }
        Some(receive) if matches!(receive.status, ShareStatus::Denied) => {
            Some(if receive.approval_timed_out {
                InvalidReason::ApprovalTimedOut
            } else {
                InvalidReason::Denied
            })
        }
        Some(receive) if require_approval && receive.approval_required && !receive.approved => {
            Some(InvalidReason::ApprovalRequired)
        }
        Some(_) => None,
    };
    if let Some(reason) = reason {
        if matches!(reason, InvalidReason::NotFound)
            && !guard.rate_limiter.check_invalid_attempt(client_ip)
        {
            return Err(InvalidReason::RateLimited);
        }
        return Err(reason);
    }
    guard.rate_limiter.clear(client_ip);
    let receive = guard.receive_session.as_ref().expect("receive checked");
    Ok(receive_snapshot(receive))
}

fn receive_snapshot(receive: &ReceiveSession) -> ReceiveSnapshot {
    ReceiveSnapshot {
        token: receive.token.clone(),
        destination_folder: receive.destination_folder.clone(),
        max_upload_bytes: receive.max_upload_bytes,
        file_name: receive.file_name.clone(),
        declared_size: receive.declared_size,
    }
}

fn spawn_upload_approval_timeout(state: HttpState, token: String) {
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(
            crate::share::APPROVAL_TIMEOUT_SECONDS as u64,
        ))
        .await;
        let info = mark_upload_approval_timed_out(&state.app_state, &token).await;
        if let Some(info) = info {
            events::emit_share_status(state.app.as_ref(), "upload_timed_out", &info);
        }
    });
}

async fn mark_upload_approval_timed_out(
    app_state: &AppState,
    token: &str,
) -> Option<ReceiveStatusInfo> {
    let mut guard = app_state.write().await;
    let local_address = guard
        .server
        .as_ref()
        .map(|server| server.address.to_string());
    let receive = guard.receive_session.as_mut()?;
    if receive.token != token
        || !matches!(receive.status, ShareStatus::AwaitingApproval)
        || receive
            .approval_deadline
            .is_some_and(|deadline| Utc::now() < deadline)
    {
        return None;
    }
    receive.deny(true);
    let message = Some("Phone upload approval timed out after 60 seconds.".to_string());
    let info = receive.status_info(local_address, message.clone());
    guard.last_request_status = message;
    Some(info)
}

async fn emit_upload_progress(state: &HttpState, bytes_received: u64) {
    let mut guard = state.app_state.write().await;
    let local_address = guard
        .server
        .as_ref()
        .map(|server| server.address.to_string());
    if let Some(receive) = guard.receive_session.as_mut() {
        receive.bytes_received = bytes_received;
        let info =
            receive.status_info(local_address, Some("Phone upload in progress.".to_string()));
        events::emit_share_status(state.app.as_ref(), "upload_progress", &info);
    }
}

async fn mark_upload_completed(
    app_state: &AppState,
    token: &str,
    bytes_received: u64,
) -> Option<ReceiveStatusInfo> {
    let mut guard = app_state.write().await;
    let local_address = guard
        .server
        .as_ref()
        .map(|server| server.address.to_string());
    let receive = guard.receive_session.as_mut()?;
    if receive.token != token {
        return None;
    }
    receive.bytes_received = bytes_received;
    receive.status = ShareStatus::Completed;
    receive.expires_at = Utc::now();
    let message = Some("Phone upload completed.".to_string());
    let info = receive.status_info(local_address, message.clone());
    guard.last_request_status = message;
    Some(info)
}

async fn set_upload_error(state: &HttpState, message: &str) {
    let mut guard = state.app_state.write().await;
    let local_address = guard
        .server
        .as_ref()
        .map(|server| server.address.to_string());
    if let Some(receive) = guard.receive_session.as_mut() {
        receive.status = ShareStatus::Error(message.to_string());
        let info = receive.status_info(local_address, Some(message.to_string()));
        events::emit_share_status(state.app.as_ref(), "upload_interrupted", &info);
    }
    guard.last_request_status = Some(message.to_string());
}

async fn unique_destination_path(folder: &std::path::Path, file_name: &str) -> std::path::PathBuf {
    let initial = folder.join(file_name);
    if !tokio::fs::try_exists(&initial).await.unwrap_or(true) {
        return initial;
    }
    let path = std::path::Path::new(file_name);
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("upload");
    let extension = path.extension().and_then(|value| value.to_str());
    for index in 2.. {
        let candidate_name = match extension {
            Some(extension) => format!("{stem} ({index}).{extension}"),
            None => format!("{stem} ({index})"),
        };
        let candidate = folder.join(candidate_name);
        if !tokio::fs::try_exists(&candidate).await.unwrap_or(true) {
            return candidate;
        }
    }
    unreachable!()
}

fn valid_token_shape(token: &str) -> bool {
    token.len() >= 22
        && token
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
}

fn api_error_response(reason: InvalidReason) -> Response {
    let (status, error) = match reason {
        InvalidReason::NotFound => (StatusCode::NOT_FOUND, "not_found"),
        InvalidReason::Expired | InvalidReason::Completed => (StatusCode::GONE, "expired"),
        InvalidReason::Cancelled => (StatusCode::GONE, "cancelled"),
        InvalidReason::Denied => (StatusCode::FORBIDDEN, "denied"),
        InvalidReason::ApprovalTimedOut => (StatusCode::REQUEST_TIMEOUT, "approval_timed_out"),
        InvalidReason::ApprovalRequired => (StatusCode::FORBIDDEN, "approval_required"),
        InvalidReason::RateLimited => (StatusCode::TOO_MANY_REQUESTS, "rate_limited"),
    };
    (status, Json(serde_json::json!({"error": error}))).into_response()
}

fn single_file_body(state: HttpState, snapshot: &ShareSnapshot, file: tokio::fs::File) -> Body {
    let app_state = state.app_state.clone();
    let token = snapshot.token.clone();
    let file_size = snapshot.file_size;
    let stream = stream! {
        let mut file = file;
        let mut buffer = vec![0_u8; 64 * 1024];
        let mut sent = 0_u64;
        let mut last_emit = Instant::now() - Duration::from_millis(250);

        loop {
            let read = match file.read(&mut buffer).await {
                Ok(read) => read,
                Err(err) => {
                    set_error_status(&state, "FluxDrop could not finish reading the selected file.").await;
                    yield Err(err);
                    return;
                }
            };
            if read == 0 {
                break;
            }
            sent = sent.saturating_add(read as u64);
            if last_emit.elapsed() >= Duration::from_millis(150) || sent == file_size {
                let info = update_progress(&app_state, &token, sent, "Download in progress.").await;
                if let Some(info) = info {
                    events::emit_share_status(state.app.as_ref(), "progress_updated", &info);
                }
                last_emit = Instant::now();
            }
            yield Ok::<Bytes, std::io::Error>(Bytes::copy_from_slice(&buffer[..read]));
        }

        let completed_info = mark_completed(&app_state, &token).await;
        if let Some(info) = completed_info {
            events::emit_share_status(state.app.as_ref(), "download_completed", &info);
        }
    };
    Body::from_stream(stream)
}

fn zip_archive_body(
    state: HttpState,
    snapshot: &ShareSnapshot,
    entries: Vec<ArchiveEntrySource>,
) -> Body {
    let (writer, reader) = tokio::io::duplex(128 * 1024);
    let token = snapshot.token.clone();
    tokio::spawn(async move {
        match write_zip_archive(&state, &token, entries, writer).await {
            Ok(()) => {
                if let Some(info) = mark_completed(&state.app_state, &token).await {
                    events::emit_share_status(state.app.as_ref(), "download_completed", &info);
                }
            }
            Err(message) => set_error_status(&state, &message).await,
        }
    });
    Body::from_stream(ReaderStream::new(reader))
}

async fn write_zip_archive(
    state: &HttpState,
    token: &str,
    entries: Vec<ArchiveEntrySource>,
    writer: tokio::io::DuplexStream,
) -> Result<(), String> {
    let mut archive = async_zip::base::write::ZipFileWriter::with_tokio(writer);
    let mut sent = 0_u64;
    let mut last_emit = Instant::now() - Duration::from_millis(250);
    let mut buffer = vec![0_u8; 64 * 1024];

    for entry in entries {
        let builder = ZipEntryBuilder::new(entry.archive_path.clone().into(), Compression::Stored);
        if entry.is_directory {
            archive
                .write_entry_whole(builder, &[])
                .await
                .map_err(|err| format!("FluxDrop could not write the ZIP directory: {err}"))?;
            continue;
        }

        let source_path = entry
            .source_path
            .ok_or_else(|| "FluxDrop found an invalid ZIP file entry.".to_string())?;
        let mut source = tokio::fs::File::open(source_path)
            .await
            .map_err(|_| "FluxDrop could not open a file while building the ZIP.".to_string())?;
        let mut entry_writer = archive
            .write_entry_stream(builder)
            .await
            .map_err(|err| format!("FluxDrop could not start a ZIP entry: {err}"))?;

        loop {
            let read = source.read(&mut buffer).await.map_err(|_| {
                "FluxDrop could not read a file while building the ZIP.".to_string()
            })?;
            if read == 0 {
                break;
            }
            futures_lite::io::AsyncWriteExt::write_all(&mut entry_writer, &buffer[..read])
                .await
                .map_err(|err| format!("FluxDrop could not stream ZIP data: {err}"))?;
            sent = sent.saturating_add(read as u64);
            if last_emit.elapsed() >= Duration::from_millis(150) {
                if let Some(info) =
                    update_progress(&state.app_state, token, sent, "Building ZIP in progress.")
                        .await
                {
                    events::emit_share_status(state.app.as_ref(), "progress_updated", &info);
                }
                last_emit = Instant::now();
            }
        }
        entry_writer
            .close()
            .await
            .map_err(|err| format!("FluxDrop could not finish a ZIP entry: {err}"))?;
    }

    archive
        .close()
        .await
        .map_err(|err| format!("FluxDrop could not finish the ZIP archive: {err}"))?;
    if let Some(info) =
        update_progress(&state.app_state, token, sent, "ZIP stream completed.").await
    {
        events::emit_share_status(state.app.as_ref(), "progress_updated", &info);
    }
    Ok(())
}

fn response_builder_for_download(snapshot: &ShareSnapshot) -> http::response::Builder {
    let builder = http::Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, snapshot.mime_type.as_str())
        .header(
            header::CONTENT_DISPOSITION,
            format!(
                "attachment; filename*=UTF-8''{}",
                content_disposition_filename(&snapshot.safe_file_name)
            ),
        )
        .header(header::CACHE_CONTROL, "no-store")
        .header(header::PRAGMA, "no-cache")
        .header(header::X_CONTENT_TYPE_OPTIONS, "nosniff");
    if snapshot.is_archive {
        builder
    } else {
        builder.header(header::CONTENT_LENGTH, snapshot.file_size.to_string())
    }
}

async fn validate_token(
    state: &HttpState,
    token: &str,
    client_ip: IpAddr,
    require_approval: bool,
) -> Result<ShareSnapshot, InvalidReason> {
    if token.len() < 22
        || !token
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
    {
        return record_invalid(state, client_ip, InvalidReason::NotFound).await;
    }

    let mut guard = state.app_state.write().await;
    let reason = match guard.current_share.as_ref() {
        None => Some(InvalidReason::NotFound),
        Some(share) if share.token != token => Some(InvalidReason::NotFound),
        Some(share) if share.cancelled => Some(InvalidReason::Cancelled),
        Some(share) if share.is_expired() => Some(InvalidReason::Expired),
        Some(share)
            if share.single_use
                && matches!(share.status, ShareStatus::Completed | ShareStatus::Expired) =>
        {
            Some(InvalidReason::Completed)
        }
        Some(share) if matches!(share.status, ShareStatus::Denied) => {
            Some(if share.approval_timed_out {
                InvalidReason::ApprovalTimedOut
            } else {
                InvalidReason::Denied
            })
        }
        Some(share) if require_approval && share.approval_required && !share.approved => {
            Some(InvalidReason::ApprovalRequired)
        }
        Some(_) => None,
    };

    if let Some(reason) = reason {
        if matches!(reason, InvalidReason::NotFound)
            && !guard.rate_limiter.check_invalid_attempt(client_ip)
        {
            guard.last_request_status =
                Some("Too many invalid link attempts from one address.".to_string());
            return Err(InvalidReason::RateLimited);
        }
        guard.last_request_status = Some(format!("Rejected request: {}", reason.label()));
        return Err(reason);
    }

    guard.rate_limiter.clear(client_ip);
    let share = guard.current_share.as_ref().expect("share checked above");
    Ok(snapshot_from_share(share))
}

async fn record_invalid<T>(
    state: &HttpState,
    client_ip: IpAddr,
    reason: InvalidReason,
) -> Result<T, InvalidReason> {
    let mut guard = state.app_state.write().await;
    if !guard.rate_limiter.check_invalid_attempt(client_ip) {
        guard.last_request_status =
            Some("Too many invalid link attempts from one address.".to_string());
        return Err(InvalidReason::RateLimited);
    }
    guard.last_request_status = Some(format!("Rejected request: {}", reason.label()));
    Err(reason)
}

fn snapshot_from_share(share: &ShareSession) -> ShareSnapshot {
    ShareSnapshot {
        token: share.token.clone(),
        payload: share.payload.clone(),
        safe_file_name: share.safe_file_name.clone(),
        file_size: share.file_size,
        file_size_human: format_file_size(share.file_size),
        mime_type: share.mime_type.clone(),
        expires_at: share.expires_at,
        single_use: share.single_use,
        sender_name: std::env::var("COMPUTERNAME")
            .or_else(|_| std::env::var("HOSTNAME"))
            .unwrap_or_else(|_| "this PC".to_string()),
        status: share.status.clone(),
        approval_required: share.approval_required,
        file_count: share.file_count,
        is_archive: share.is_archive,
    }
}

fn spawn_approval_timeout(state: HttpState, token: String) {
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(
            crate::share::APPROVAL_TIMEOUT_SECONDS as u64,
        ))
        .await;
        let info = mark_approval_timed_out(&state.app_state, &token).await;
        if let Some(info) = info {
            events::emit_share_status(state.app.as_ref(), "download_timed_out", &info);
        }
    });
}

async fn mark_approval_timed_out(app_state: &AppState, token: &str) -> Option<ShareStatusInfo> {
    let mut guard = app_state.write().await;
    let local_address = guard
        .server
        .as_ref()
        .map(|server| server.address.to_string());
    let share = guard.current_share.as_mut()?;
    if share.token != token
        || !matches!(share.status, ShareStatus::AwaitingApproval)
        || share
            .approval_deadline
            .is_some_and(|deadline| Utc::now() < deadline)
    {
        return None;
    }
    share.deny(true);
    let last_request_status = Some("Download request timed out after 60 seconds.".to_string());
    let info = share.status_info(local_address, last_request_status.clone());
    guard.last_request_status = last_request_status;
    Some(info)
}

async fn update_progress(
    app_state: &AppState,
    token: &str,
    bytes_sent: u64,
    request_status: &str,
) -> Option<ShareStatusInfo> {
    let mut guard = app_state.write().await;
    let local_address = guard
        .server
        .as_ref()
        .map(|server| server.address.to_string());
    let last_request_status = Some(request_status.to_string());
    guard.last_request_status = last_request_status.clone();
    let share = guard.current_share.as_mut()?;
    if share.token != token {
        return None;
    }
    share.update_progress(bytes_sent);
    Some(share.status_info(local_address, last_request_status))
}

async fn mark_completed(app_state: &AppState, token: &str) -> Option<ShareStatusInfo> {
    let mut guard = app_state.write().await;
    let local_address = guard
        .server
        .as_ref()
        .map(|server| server.address.to_string());
    let last_request_status = Some("Download completed.".to_string());
    guard.last_request_status = last_request_status.clone();
    let share = guard.current_share.as_mut()?;
    if share.token != token {
        return None;
    }
    share.mark_download_completed();
    Some(share.status_info(local_address, last_request_status))
}

async fn set_error_status(state: &HttpState, message: &str) {
    let mut guard = state.app_state.write().await;
    let local_address = guard
        .server
        .as_ref()
        .map(|server| server.address.to_string());
    let last_request_status = Some(message.to_string());
    guard.last_request_status = last_request_status.clone();
    if let Some(share) = guard.current_share.as_mut() {
        share.status = ShareStatus::Error(message.to_string());
        let info = share.status_info(local_address, last_request_status);
        events::emit_share_status(state.app.as_ref(), "download_interrupted", &info);
    }
}

async fn security_headers(request: Request<Body>, next: Next) -> Response {
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    insert_header(headers, header::CONTENT_SECURITY_POLICY, "default-src 'self'; style-src 'self' 'unsafe-inline'; script-src 'self'; connect-src 'self'; img-src 'self' data:; object-src 'none'; base-uri 'none'; frame-ancestors 'none'");
    insert_header(headers, header::X_CONTENT_TYPE_OPTIONS, "nosniff");
    insert_header(headers, header::REFERRER_POLICY, "no-referrer");
    insert_header(headers, header::X_FRAME_OPTIONS, "DENY");
    insert_header(
        headers,
        HeaderName::from_static("permissions-policy"),
        "camera=(), microphone=(), geolocation=()",
    );
    insert_header(headers, header::CACHE_CONTROL, "no-store");
    insert_header(headers, header::PRAGMA, "no-cache");
    response
}

fn insert_header(headers: &mut http::HeaderMap, name: HeaderName, value: &'static str) {
    headers.insert(name, HeaderValue::from_static(value));
}

fn mobile_download_html(snapshot: &ShareSnapshot) -> String {
    let file_name = escape_html(&snapshot.safe_file_name);
    let file_size = escape_html(&snapshot.file_size_human);
    let mime_type = escape_html(&snapshot.mime_type);
    let sender = escape_html(&snapshot.sender_name);
    let token = escape_html(&snapshot.token);
    let (refresh, action, intro, note) = match snapshot.status {
        ShareStatus::AwaitingApproval => (
            r#"<meta http-equiv="refresh" content="2">"#,
            r#"<div class="waiting">Waiting for approval on the PC...</div>"#.to_string(),
            format!(
                "{sender} received your request and must approve it before the download starts."
            ),
            "This page refreshes automatically. Approval requests time out after 60 seconds."
                .to_string(),
        ),
        ShareStatus::Approved => (
            "",
            format!(r#"<a class="button" href="/download/{token}">Download approved file</a>"#),
            format!("{sender} approved this download."),
            "The link expires automatically after the configured time.".to_string(),
        ),
        _ if !snapshot.approval_required => (
            "",
            format!(r#"<a class="button" href="/download/{token}">Download</a>"#),
            format!(
                "{sender} is sharing {} with this browser over local Wi-Fi.",
                if snapshot.is_archive {
                    format!("{} files in a ZIP archive", snapshot.file_count)
                } else {
                    "one file".to_string()
                }
            ),
            "This link expires automatically. Use FluxDrop only on trusted networks.".to_string(),
        ),
        _ => (
            r#"<meta http-equiv="refresh" content="2">"#,
            r#"<div class="waiting">Contacting the PC...</div>"#.to_string(),
            format!("{sender} is preparing this transfer."),
            "This page refreshes automatically.".to_string(),
        ),
    };
    format!(
        r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  {refresh}
  <title>FluxDrop Download</title>
  <style>
    :root {{ color-scheme: light; font-family: Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif; background: #f6f7f9; color: #111827; }}
    body {{ margin: 0; min-height: 100vh; display: grid; place-items: center; padding: 24px; box-sizing: border-box; }}
    main {{ width: min(100%, 460px); background: #fff; border: 1px solid #d7dce3; border-radius: 8px; padding: 28px; box-shadow: 0 20px 60px rgba(15, 23, 42, .12); }}
    h1 {{ margin: 0 0 8px; font-size: 1.65rem; }}
    p {{ color: #4b5563; line-height: 1.5; }}
    dl {{ display: grid; grid-template-columns: auto 1fr; gap: 10px 14px; margin: 24px 0; }}
    dt {{ color: #6b7280; }}
    dd {{ margin: 0; font-weight: 650; overflow-wrap: anywhere; }}
    a.button {{ display: block; text-align: center; padding: 15px 18px; border-radius: 7px; background: #0f172a; color: #fff; text-decoration: none; font-weight: 750; }}
    .waiting {{ padding: 15px 18px; border-radius: 7px; background: #fef3c7; color: #92400e; text-align: center; font-weight: 750; }}
    .note {{ font-size: .92rem; color: #6b7280; margin-bottom: 0; }}
  </style>
</head>
<body>
  <main>
    <h1>FluxDrop</h1>
    <p>{intro}</p>
    <dl>
      <dt>File</dt><dd>{file_name}</dd>
      <dt>Size</dt><dd>{file_size}</dd>
      <dt>Type</dt><dd>{mime_type}</dd>
    </dl>
    {action}
    <p class="note">{note}</p>
  </main>
</body>
</html>"#
    )
}

fn mobile_upload_html(snapshot: &ReceiveSnapshot) -> String {
    let token = escape_html(&snapshot.token);
    let max_size = escape_html(&format_file_size(snapshot.max_upload_bytes));
    format!(
        r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Upload to FluxDrop</title>
  <style>
    :root {{ color-scheme: light; font-family: Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif; background: #f6f7f9; color: #111827; }}
    body {{ margin: 0; min-height: 100vh; display: grid; place-items: center; padding: 24px; box-sizing: border-box; }}
    main {{ width: min(100%, 460px); background: #fff; border: 1px solid #d7dce3; border-radius: 8px; padding: 28px; box-shadow: 0 20px 60px rgba(15, 23, 42, .12); }}
    h1 {{ margin: 0 0 8px; font-size: 1.65rem; }}
    p {{ color: #4b5563; line-height: 1.5; }}
    form {{ display: grid; gap: 14px; margin-top: 22px; }}
    input {{ width: 100%; padding: 13px; box-sizing: border-box; border: 1px solid #cbd5e1; border-radius: 7px; }}
    button {{ padding: 15px 18px; border: 0; border-radius: 7px; background: #0f172a; color: #fff; font: inherit; font-weight: 750; }}
    button:disabled {{ opacity: .6; }}
    #status {{ padding: 12px; border-radius: 7px; background: #f8fafc; color: #334155; }}
  </style>
</head>
<body data-token="{token}">
  <main>
    <h1>Send a file to this PC</h1>
    <p>Select one file. FluxDrop sends its name and size to the PC for approval before the upload begins.</p>
    <p>Maximum upload size: <strong>{max_size}</strong></p>
    <form id="upload-form">
      <input id="file-input" name="file" type="file" required>
      <button id="upload-button" type="submit">Request upload</button>
    </form>
    <p id="status" aria-live="polite">Waiting for a file selection.</p>
  </main>
  <script src="/upload.js" defer></script>
</body>
</html>"#
    )
}

const ONBOARDING_HTML: &str = r##"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Connect securely to FluxDrop</title>
  <style>
    :root { color-scheme: light; font-family: ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif; }
    body { margin: 0; min-height: 100vh; display: grid; place-items: center; padding: 24px; background: #f6f7f9; color: #111827; }
    main { width: min(100%, 470px); box-sizing: border-box; background: #fff; border: 1px solid #d7dce3; border-radius: 12px; padding: 28px; box-shadow: 0 20px 60px rgba(15, 23, 42, .12); }
    .eyebrow { color: #1d4ed8; font-size: .78rem; font-weight: 800; letter-spacing: .08em; text-transform: uppercase; }
    h1 { margin: 10px 0 12px; font-size: 1.65rem; line-height: 1.2; }
    p, li { color: #4b5563; line-height: 1.55; }
    ol { padding-left: 22px; }
    .button { display: block; margin-top: 22px; padding: 13px 18px; border-radius: 8px; background: #1d4ed8; color: #fff; font-weight: 800; text-align: center; text-decoration: none; }
    .button[hidden] { display: none; }
    .note { margin-top: 18px; padding: 12px 14px; border-radius: 8px; background: #eff6ff; color: #1e3a8a; font-size: .92rem; }
    .error { color: #b91c1c; font-weight: 700; }
  </style>
  <script src="/connect.js" defer></script>
</head>
<body>
  <main>
    <span class="eyebrow">Encrypted local transfer</span>
    <h1>One browser confirmation is required</h1>
    <p>FluxDrop encrypts this transfer with a certificate generated by your PC. Because it is self-signed, your phone cannot verify it automatically.</p>
    <ol>
      <li>Tap <strong>Continue securely</strong>.</li>
      <li>On the browser warning, tap <strong>Advanced</strong>.</li>
      <li>Choose <strong>Proceed</strong> to open FluxDrop.</li>
    </ol>
    <a id="continue-link" class="button" href="#" rel="noreferrer" hidden>Continue securely</a>
    <p id="status" class="note">Preparing the encrypted connection...</p>
    <p class="note">Only continue when this QR code came from the FluxDrop PC you expect. Self-signed HTTPS blocks passive Wi-Fi snooping, but it does not prove the PC's identity.</p>
  </main>
</body>
</html>"##;

const ONBOARDING_SCRIPT: &str = r#"(() => {
  "use strict";
  const link = document.getElementById("continue-link");
  const status = document.getElementById("status");

  try {
    const encodedTarget = window.location.hash.slice(1);
    if (!encodedTarget) throw new Error("This setup link is incomplete. Scan the current FluxDrop QR code again.");
    const target = new URL(decodeURIComponent(encodedTarget));
    const validPath = /^\/(?:d|u)\/[A-Za-z0-9_-]{20,}$/.test(target.pathname);
    if (target.protocol !== "https:" || target.hostname !== window.location.hostname || !target.port || !validPath) {
      throw new Error("This setup link is invalid. Scan the current FluxDrop QR code again.");
    }
    link.href = target.toString();
    link.hidden = false;
    status.textContent = "Ready. The next page is the encrypted FluxDrop connection.";
  } catch (error) {
    status.classList.add("error");
    status.textContent = error instanceof Error ? error.message : "This setup link is invalid.";
  }
})();"#;

const UPLOAD_SCRIPT: &str = r#"(() => {
  const token = document.body.dataset.token;
  const form = document.getElementById("upload-form");
  const input = document.getElementById("file-input");
  const button = document.getElementById("upload-button");
  const status = document.getElementById("status");
  let active = false;

  const setStatus = (message) => { status.textContent = message; };
  const wait = (ms) => new Promise((resolve) => window.setTimeout(resolve, ms));

  async function pollForApproval(file) {
    for (;;) {
      await wait(1000);
      const response = await fetch(`/api/upload-status/${encodeURIComponent(token)}`, { cache: "no-store" });
      if (!response.ok) {
        const error = await response.json().catch(() => ({ error: "request_failed" }));
        throw new Error(error.error === "approval_timed_out" ? "Approval timed out." : "The receive link is no longer available.");
      }
      const current = await response.json();
      if (current.status.kind === "Approved") return upload(file);
      if (current.status.kind === "Denied") {
        throw new Error(current.approval_timed_out ? "Approval timed out." : "The PC denied this upload.");
      }
      if (current.status.kind === "Cancelled" || current.status.kind === "Expired") {
        throw new Error("The receive link is no longer available.");
      }
      setStatus("Waiting for approval on the PC...");
    }
  }

  async function upload(file) {
    setStatus("Approved. Uploading to the PC...");
    const body = new FormData();
    body.append("file", file, file.name);
    const response = await fetch(`/upload/${encodeURIComponent(token)}`, { method: "POST", body });
    if (!response.ok) {
      const error = await response.json().catch(() => ({ error: "upload_failed" }));
      throw new Error(`Upload failed: ${error.error || response.status}`);
    }
    setStatus("Upload complete. The file is safely stored on the PC.");
    button.textContent = "Uploaded";
  }

  form.addEventListener("submit", async (event) => {
    event.preventDefault();
    if (active) return;
    const file = input.files && input.files[0];
    if (!file) return;
    active = true;
    button.disabled = true;
    try {
      setStatus("Sending file details to the PC...");
      const response = await fetch(`/api/upload-request/${encodeURIComponent(token)}`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ file_name: file.name, file_size: file.size, mime_type: file.type || null })
      });
      if (!response.ok) {
        const error = await response.json().catch(() => ({ error: "request_failed" }));
        throw new Error(error.message || `Request failed: ${error.error || response.status}`);
      }
      setStatus("Waiting for approval on the PC...");
      await pollForApproval(file);
    } catch (error) {
      setStatus(error instanceof Error ? error.message : String(error));
      button.disabled = false;
      active = false;
    }
  });
})();"#;

fn error_page_response(reason: InvalidReason) -> Response {
    let (status, title, message) = match reason {
        InvalidReason::Expired | InvalidReason::Completed => (
            StatusCode::GONE,
            "Link Expired",
            "This FluxDrop link has expired or was already used.",
        ),
        InvalidReason::Cancelled => (
            StatusCode::GONE,
            "Transfer Cancelled",
            "The sender cancelled this FluxDrop transfer.",
        ),
        InvalidReason::ApprovalRequired => (
            StatusCode::FORBIDDEN,
            "Waiting for Approval",
            "The sender must approve this download on the PC before it can start.",
        ),
        InvalidReason::Denied => (
            StatusCode::FORBIDDEN,
            "Transfer Denied",
            "The sender denied this FluxDrop download.",
        ),
        InvalidReason::ApprovalTimedOut => (
            StatusCode::REQUEST_TIMEOUT,
            "Approval Timed Out",
            "The PC did not approve this request within 60 seconds. Start a new share and try again.",
        ),
        InvalidReason::RateLimited => (
            StatusCode::TOO_MANY_REQUESTS,
            "Too Many Attempts",
            "Too many invalid link attempts were received. Wait a minute and try again.",
        ),
        InvalidReason::NotFound => (
            StatusCode::NOT_FOUND,
            "Link Not Found",
            "This FluxDrop link is invalid or has expired.",
        ),
    };
    (status, Html(error_html(title, message))).into_response()
}

fn error_html(title: &str, message: &str) -> String {
    format!(
        r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>{}</title>
  <style>
    body {{ margin: 0; min-height: 100vh; display: grid; place-items: center; padding: 24px; font-family: ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif; background: #f6f7f9; color: #111827; }}
    main {{ max-width: 440px; background: #fff; border: 1px solid #d7dce3; border-radius: 8px; padding: 28px; box-shadow: 0 20px 60px rgba(15, 23, 42, .12); }}
    h1 {{ margin: 0 0 8px; font-size: 1.5rem; }}
    p {{ margin-bottom: 0; color: #4b5563; line-height: 1.5; }}
  </style>
</head>
<body><main><h1>{}</h1><p>{}</p></main></body>
</html>"#,
        escape_html(title),
        escape_html(title),
        escape_html(message)
    )
}

impl InvalidReason {
    fn label(self) -> &'static str {
        match self {
            InvalidReason::NotFound => "not found",
            InvalidReason::Expired => "expired",
            InvalidReason::Cancelled => "cancelled",
            InvalidReason::Completed => "completed",
            InvalidReason::Denied => "denied",
            InvalidReason::ApprovalTimedOut => "approval timed out",
            InvalidReason::ApprovalRequired => "approval required",
            InvalidReason::RateLimited => "rate limited",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use http_body_util::BodyExt;
    use std::io::{Cursor, Read};
    use std::net::{IpAddr, Ipv4Addr};
    use tower::ServiceExt;

    async fn request(router: Router, uri: &str) -> Response {
        request_with(router, "GET", uri, Vec::new(), Body::empty()).await
    }

    async fn request_with(
        router: Router,
        method: &str,
        uri: &str,
        headers: Vec<(&str, &str)>,
        body: Body,
    ) -> Response {
        let mut builder = Request::builder()
            .method(method)
            .uri(uri)
            .extension(ConnectInfo(SocketAddr::new(
                IpAddr::V4(Ipv4Addr::LOCALHOST),
                50000,
            )));
        for (name, value) in headers {
            builder = builder.header(name, value);
        }
        router
            .oneshot(builder.body(body).expect("request"))
            .await
            .expect("response")
    }

    #[tokio::test]
    async fn test_invalid_token_returns_404() {
        let router = build_router(AppState::new(), None);
        let response = request(router, "/d/not-a-real-token-000000").await;
        assert_ne!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_expired_share_rejects_download() {
        let temp = tempfile::NamedTempFile::new().expect("temp");
        let mut share = ShareSession::new(
            temp.path().to_path_buf(),
            "file.txt".into(),
            "file.txt".into(),
            0,
            "text/plain".into(),
        );
        share.expires_at = Utc::now() - chrono::Duration::seconds(1);
        let token = share.token.clone();
        let state = AppState::new();
        state.write().await.current_share = Some(share);
        let response = request(build_router(state, None), &format!("/download/{token}")).await;
        assert_eq!(response.status(), StatusCode::GONE);
    }

    #[tokio::test]
    async fn test_cancelled_share_rejects_download() {
        let temp = tempfile::NamedTempFile::new().expect("temp");
        let mut share = ShareSession::new(
            temp.path().to_path_buf(),
            "file.txt".into(),
            "file.txt".into(),
            0,
            "text/plain".into(),
        );
        share.cancel();
        let token = share.token.clone();
        let state = AppState::new();
        state.write().await.current_share = Some(share);
        let response = request(build_router(state, None), &format!("/download/{token}")).await;
        assert_eq!(response.status(), StatusCode::GONE);
    }

    #[tokio::test]
    async fn test_completed_single_use_share_rejects_second_download() {
        let temp = tempfile::NamedTempFile::new().expect("temp");
        let mut share = ShareSession::new(
            temp.path().to_path_buf(),
            "file.txt".into(),
            "file.txt".into(),
            0,
            "text/plain".into(),
        );
        share.mark_download_completed();
        let token = share.token.clone();
        let state = AppState::new();
        state.write().await.current_share = Some(share);
        let response = request(build_router(state, None), &format!("/download/{token}")).await;
        assert_eq!(response.status(), StatusCode::GONE);
    }

    #[tokio::test]
    async fn test_download_requires_pc_approval() {
        let temp = tempfile::NamedTempFile::new().expect("temp");
        let share = ShareSession::new(
            temp.path().to_path_buf(),
            "file.txt".into(),
            "file.txt".into(),
            0,
            "text/plain".into(),
        );
        let token = share.token.clone();
        let state = AppState::new();
        state.write().await.current_share = Some(share);
        let response = request(build_router(state, None), &format!("/download/{token}")).await;
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn test_landing_page_waits_for_approval() {
        let temp = tempfile::NamedTempFile::new().expect("temp");
        let share = ShareSession::new(
            temp.path().to_path_buf(),
            "file.txt".into(),
            "file.txt".into(),
            0,
            "text/plain".into(),
        );
        let token = share.token.clone();
        let state = AppState::new();
        state.write().await.current_share = Some(share);
        let response = request(build_router(state.clone(), None), &format!("/d/{token}")).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        assert!(String::from_utf8_lossy(&body).contains("Waiting for approval on the PC"));
        assert_eq!(
            state
                .read()
                .await
                .current_share
                .as_ref()
                .expect("share")
                .status,
            ShareStatus::AwaitingApproval
        );
    }

    #[tokio::test]
    async fn test_timeout_has_distinct_phone_message() {
        let temp = tempfile::NamedTempFile::new().expect("temp");
        let mut share = ShareSession::new(
            temp.path().to_path_buf(),
            "file.txt".into(),
            "file.txt".into(),
            0,
            "text/plain".into(),
        );
        let token = share.token.clone();
        share.mark_phone_connected(IpAddr::V4(Ipv4Addr::LOCALHOST));
        share.approval_deadline = Some(Utc::now() - chrono::Duration::seconds(1));
        let state = AppState::new();
        state.write().await.current_share = Some(share);
        assert!(mark_approval_timed_out(&state, &token).await.is_some());
        let response = request(build_router(state, None), &format!("/d/{token}")).await;
        assert_eq!(response.status(), StatusCode::REQUEST_TIMEOUT);
        let body = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        assert!(String::from_utf8_lossy(&body).contains("Approval Timed Out"));
    }

    #[tokio::test]
    async fn test_zip_archive_stream_is_valid_and_preserves_paths() {
        let directory = tempfile::tempdir().expect("tempdir");
        let first = directory.path().join("one.txt");
        let second = directory.path().join("two.txt");
        std::fs::write(&first, b"one").expect("write first");
        std::fs::write(&second, b"second").expect("write second");
        let entries = vec![
            ArchiveEntrySource {
                source_path: Some(first),
                archive_path: "bundle/one.txt".to_string(),
                size: 3,
                is_directory: false,
            },
            ArchiveEntrySource {
                source_path: Some(second),
                archive_path: "bundle/nested/two.txt".to_string(),
                size: 6,
                is_directory: false,
            },
        ];
        let share = ShareSession::new_with_payload(
            SharePayload::ZipArchive {
                entries: entries.clone(),
            },
            "bundle.zip".into(),
            "bundle.zip".into(),
            9,
            "application/zip".into(),
            2,
            true,
            10,
            true,
            false,
        );
        let token = share.token.clone();
        let app_state = AppState::new();
        app_state.write().await.current_share = Some(share);
        let http_state = HttpState {
            app_state,
            app: None,
        };
        let (writer, mut reader) = tokio::io::duplex(1024);
        let writer_state = http_state.clone();
        let writer_token = token.clone();
        let task = tokio::spawn(async move {
            write_zip_archive(&writer_state, &writer_token, entries, writer).await
        });
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await.expect("read zip");
        task.await.expect("join").expect("write zip");

        let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).expect("valid zip");
        let mut one = String::new();
        archive
            .by_name("bundle/one.txt")
            .expect("first entry")
            .read_to_string(&mut one)
            .expect("read first");
        let mut two = String::new();
        archive
            .by_name("bundle/nested/two.txt")
            .expect("second entry")
            .read_to_string(&mut two)
            .expect("read second");
        assert_eq!(one, "one");
        assert_eq!(two, "second");
    }

    #[tokio::test]
    async fn test_upload_requires_approval_before_body_is_accepted() {
        let destination = tempfile::tempdir().expect("tempdir");
        let receive = ReceiveSession::new(destination.path().to_path_buf(), 10, true, 1024);
        let token = receive.token.clone();
        let state = AppState::new();
        state.write().await.receive_session = Some(receive);
        let router = build_router(state.clone(), None);
        let metadata = serde_json::json!({
            "file_name": "photo.jpg",
            "file_size": 5,
            "mime_type": "image/jpeg"
        });
        let response = request_with(
            router,
            "POST",
            &format!("/api/upload-request/{token}"),
            vec![("content-type", "application/json")],
            Body::from(metadata.to_string()),
        )
        .await;
        assert_eq!(response.status(), StatusCode::ACCEPTED);
        assert_eq!(
            state
                .read()
                .await
                .receive_session
                .as_ref()
                .expect("receive")
                .status,
            ShareStatus::AwaitingApproval
        );

        let boundary = "fluxdrop-test";
        let body = format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"photo.jpg\"\r\nContent-Type: image/jpeg\r\n\r\nhello\r\n--{boundary}--\r\n"
        );
        let response = request_with(
            build_router(state, None),
            "POST",
            &format!("/upload/{token}"),
            vec![(
                "content-type",
                "multipart/form-data; boundary=fluxdrop-test",
            )],
            Body::from(body),
        )
        .await;
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert!(!destination.path().join("photo.jpg").exists());
    }

    #[tokio::test]
    async fn test_streamed_upload_sanitizes_name_and_leaves_no_partial_file() {
        let destination = tempfile::tempdir().expect("tempdir");
        let receive = ReceiveSession::new(destination.path().to_path_buf(), 10, false, 1024);
        let token = receive.token.clone();
        let state = AppState::new();
        state.write().await.receive_session = Some(receive);
        let metadata = serde_json::json!({
            "file_name": "../../secret.txt",
            "file_size": 5,
            "mime_type": "text/plain"
        });
        let response = request_with(
            build_router(state.clone(), None),
            "POST",
            &format!("/api/upload-request/{token}"),
            vec![("content-type", "application/json")],
            Body::from(metadata.to_string()),
        )
        .await;
        assert_eq!(response.status(), StatusCode::ACCEPTED);

        let boundary = "fluxdrop-upload";
        let body = format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"../../secret.txt\"\r\nContent-Type: text/plain\r\n\r\nhello\r\n--{boundary}--\r\n"
        );
        let response = request_with(
            build_router(state, None),
            "POST",
            &format!("/upload/{token}"),
            vec![(
                "content-type",
                "multipart/form-data; boundary=fluxdrop-upload",
            )],
            Body::from(body),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            std::fs::read(destination.path().join("secret.txt")).expect("final file"),
            b"hello"
        );
        let leftovers = std::fs::read_dir(destination.path())
            .expect("read dir")
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().contains(".part"))
            .count();
        assert_eq!(leftovers, 0);
    }

    #[tokio::test]
    async fn test_upload_metadata_over_limit_is_rejected_early() {
        let destination = tempfile::tempdir().expect("tempdir");
        let receive = ReceiveSession::new(destination.path().to_path_buf(), 10, false, 4);
        let token = receive.token.clone();
        let state = AppState::new();
        state.write().await.receive_session = Some(receive);
        let metadata = serde_json::json!({
            "file_name": "large.bin",
            "file_size": 5,
            "mime_type": "application/octet-stream"
        });
        let response = request_with(
            build_router(state, None),
            "POST",
            &format!("/api/upload-request/{token}"),
            vec![("content-type", "application/json")],
            Body::from(metadata.to_string()),
        )
        .await;
        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
        assert!(!destination.path().join("large.bin").exists());
    }

    #[tokio::test]
    async fn test_file_path_not_in_url() {
        let router = build_router(AppState::new(), None);
        let response = request(router, "/download/C:/Users/Name/file.txt").await;
        assert_ne!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_health_contains_no_share_data() {
        let response = request(build_router(AppState::new(), None), "/health").await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        assert_eq!(&body[..], br#"{"status":"ok"}"#);
    }

    #[tokio::test]
    async fn test_onboarding_page_contains_no_transfer_token() {
        let response = request(build_onboarding_router(), "/connect").await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get(header::REFERRER_POLICY)
                .and_then(|value| value.to_str().ok()),
            Some("no-referrer")
        );
        let body = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        let html = String::from_utf8_lossy(&body);
        assert!(html.contains("self-signed"));
        assert!(!html.contains("/d/"));
        assert!(!html.contains("/u/"));
    }

    #[tokio::test]
    async fn test_onboarding_script_reads_fragment_and_restricts_destination() {
        let response = request(build_onboarding_router(), "/connect.js").await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        let script = String::from_utf8_lossy(&body);
        assert!(script.contains("window.location.hash"));
        assert!(script.contains("target.protocol !== \"https:\""));
        assert!(script.contains("target.hostname !== window.location.hostname"));
    }

    #[tokio::test]
    async fn test_dual_server_serves_http_onboarding_and_https_health() {
        let directory = tempfile::tempdir().expect("tempdir");
        let state = AppState::with_settings(
            crate::settings::AppSettings::default(),
            Some(directory.path().join("settings.json")),
        );
        let server = start_server(state, None, IpAddr::V4(Ipv4Addr::LOCALHOST), 0)
            .await
            .expect("start dual server");
        let client = reqwest::Client::builder()
            .danger_accept_invalid_certs(true)
            .build()
            .expect("client");

        let onboarding = client
            .get(format!("http://{}/connect", server.onboarding_address))
            .send()
            .await
            .expect("onboarding request");
        assert_eq!(onboarding.status(), reqwest::StatusCode::OK);
        assert!(onboarding
            .text()
            .await
            .expect("onboarding body")
            .contains("self-signed"));

        let health = client
            .get(format!("https://{}/health", server.address))
            .send()
            .await
            .expect("https health request");
        assert_eq!(health.status(), reqwest::StatusCode::OK);
        assert_eq!(
            health.text().await.expect("health body"),
            r#"{"status":"ok"}"#
        );

        server.stop().await;
    }
}
