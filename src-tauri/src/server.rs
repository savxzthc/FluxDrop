use crate::events;
use crate::events::EventHandle;
use crate::file_utils::{content_disposition_filename, format_file_size, sanitize_filename};
use crate::receive::{ReceiveSession, ReceiveStatusInfo};
use crate::share::{
    ArchiveEntrySource, SharePayload, ShareSession, ShareStatus, ShareStatusInfo, TOKEN_CHARS,
};
use crate::state::{AppState, ServerHandle};
use crate::tls;
use async_stream::stream;
use async_zip::{Compression, ZipEntryBuilder};
use axum::body::Body;
use axum::extract::{ConnectInfo, Multipart, Path, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use axum::Json;
use bytes::Bytes;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::io;
use std::net::{IpAddr, SocketAddr, TcpListener as StdTcpListener};
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_util::io::ReaderStream;
use tokio_util::sync::CancellationToken;

mod download;
mod routes;
mod security;
mod templates;
mod upload;

pub use routes::{build_onboarding_router, build_router};
use templates::{
    error_html, mobile_download_html, mobile_upload_html, ONBOARDING_HTML, ONBOARDING_SCRIPT,
    UPLOAD_SCRIPT,
};

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
    ClientMismatch,
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

async fn root() -> Html<&'static str> {
    Html(templates::ROOT_HTML)
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

fn valid_token_shape(token: &str) -> bool {
    token.len() == TOKEN_CHARS
        && token
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
}

fn approval_client_mismatch(
    approval_required: bool,
    approved_client_ip: Option<IpAddr>,
    request_ip: IpAddr,
    status: &ShareStatus,
) -> bool {
    approval_required
        && matches!(
            status,
            ShareStatus::PhoneConnected
                | ShareStatus::AwaitingApproval
                | ShareStatus::Approved
                | ShareStatus::Downloading
                | ShareStatus::Uploading
        )
        && approved_client_ip.is_some_and(|client_ip| client_ip != request_ip)
}

fn api_error_response(reason: InvalidReason) -> Response {
    let (status, error) = match reason {
        InvalidReason::NotFound => (StatusCode::NOT_FOUND, "not_found"),
        InvalidReason::Expired | InvalidReason::Completed => (StatusCode::GONE, "expired"),
        InvalidReason::Cancelled => (StatusCode::GONE, "cancelled"),
        InvalidReason::Denied => (StatusCode::FORBIDDEN, "denied"),
        InvalidReason::ApprovalTimedOut => (StatusCode::REQUEST_TIMEOUT, "approval_timed_out"),
        InvalidReason::ApprovalRequired => (StatusCode::FORBIDDEN, "approval_required"),
        InvalidReason::ClientMismatch => (StatusCode::FORBIDDEN, "client_mismatch"),
        InvalidReason::RateLimited => (StatusCode::TOO_MANY_REQUESTS, "rate_limited"),
    };
    (status, Json(serde_json::json!({"error": error}))).into_response()
}

async fn record_invalid<T>(
    state: &HttpState,
    client_ip: IpAddr,
    reason: InvalidReason,
) -> Result<T, InvalidReason> {
    Err(record_invalid_reason(state, client_ip, reason).await)
}

async fn record_invalid_reason(
    state: &HttpState,
    client_ip: IpAddr,
    reason: InvalidReason,
) -> InvalidReason {
    let mut guard = state.app_state.write().await;
    if !guard.rate_limiter.consume_invalid_attempt(client_ip) {
        guard.last_request_status =
            Some("Too many invalid link attempts from one address.".to_string());
        return InvalidReason::RateLimited;
    }
    guard.last_request_status = Some(format!("Rejected request: {}", reason.label()));
    reason
}

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
        InvalidReason::ClientMismatch => (
            StatusCode::FORBIDDEN,
            "Different Device",
            "This approval belongs to the phone that requested it. Scan a fresh QR code from the device you want to use.",
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
            InvalidReason::ClientMismatch => "different client",
            InvalidReason::RateLimited => "rate limited",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use axum::Router;
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
        request_from_with(
            router,
            IpAddr::V4(Ipv4Addr::LOCALHOST),
            method,
            uri,
            headers,
            body,
        )
        .await
    }

    async fn request_from(router: Router, client_ip: IpAddr, uri: &str) -> Response {
        request_from_with(router, client_ip, "GET", uri, Vec::new(), Body::empty()).await
    }

    async fn request_from_with(
        router: Router,
        client_ip: IpAddr,
        method: &str,
        uri: &str,
        headers: Vec<(&str, &str)>,
        body: Body,
    ) -> Response {
        let mut builder = Request::builder()
            .method(method)
            .uri(uri)
            .extension(ConnectInfo(SocketAddr::new(client_ip, 50000)));
        for (name, value) in headers {
            builder = builder.header(name, value);
        }
        router
            .oneshot(builder.body(body).expect("request"))
            .await
            .expect("response")
    }

    async fn json_body(response: Response) -> serde_json::Value {
        let body = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        serde_json::from_slice(&body).expect("json body")
    }

    #[tokio::test]
    async fn test_invalid_token_returns_404() {
        let router = build_router(AppState::new(), None);
        let response = request(router, "/d/not-a-real-token-000000").await;
        assert_ne!(response.status(), StatusCode::OK);
    }

    #[test]
    fn token_shape_matches_generated_token_length() {
        let token = crate::share::generate_token();
        assert!(valid_token_shape(&token));
        assert!(!valid_token_shape(&token[..TOKEN_CHARS - 1]));
        assert!(!valid_token_shape(&format!("{token}a")));
        assert!(!valid_token_shape(&"a".repeat(TOKEN_CHARS - 1)));
        assert!(!valid_token_shape(&format!(
            "{}!",
            &token[..TOKEN_CHARS - 1]
        )));
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
    async fn expired_share_api_returns_precise_error() {
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
        let response = request(build_router(state, None), &format!("/api/share/{token}")).await;
        assert_eq!(response.status(), StatusCode::GONE);
        assert_eq!(json_body(response).await["error"], "expired");
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
    async fn approved_download_is_bound_to_requesting_client() {
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
        let first_phone = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 20));
        let second_phone = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 21));

        let response = request_from(
            build_router(state.clone(), None),
            first_phone,
            &format!("/d/{token}"),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        state
            .write()
            .await
            .current_share
            .as_mut()
            .expect("share")
            .approve();

        let response = request_from(
            build_router(state.clone(), None),
            second_phone,
            &format!("/api/share/{token}"),
        )
        .await;
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert_eq!(json_body(response).await["error"], "client_mismatch");

        let response = request_from(
            build_router(state, None),
            first_phone,
            &format!("/api/share/{token}"),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn denied_share_api_returns_precise_error() {
        let temp = tempfile::NamedTempFile::new().expect("temp");
        let mut share = ShareSession::new(
            temp.path().to_path_buf(),
            "file.txt".into(),
            "file.txt".into(),
            0,
            "text/plain".into(),
        );
        share.deny(false);
        let token = share.token.clone();
        let state = AppState::new();
        state.write().await.current_share = Some(share);
        let response = request(build_router(state, None), &format!("/api/share/{token}")).await;
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert_eq!(json_body(response).await["error"], "denied");
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
        assert!(download::mark_approval_timed_out(&state, &token)
            .await
            .is_some());
        assert_eq!(
            state.read().await.history[0].outcome,
            crate::history::TransferOutcome::TimedOut
        );
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
    async fn completed_download_is_added_to_history() {
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

        assert!(download::mark_completed(&state, &token).await.is_some());

        let guard = state.read().await;
        assert_eq!(guard.history.len(), 1);
        assert_eq!(
            guard.history[0].outcome,
            crate::history::TransferOutcome::Completed
        );
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
                source_path: Some(first.clone()),
                archive_path: "bundle/one.txt".to_string(),
                size: 3,
                is_directory: false,
            },
            ArchiveEntrySource {
                source_path: Some(second.clone()),
                archive_path: "bundle/nested/two.txt".to_string(),
                size: 6,
                is_directory: false,
            },
        ];
        let share = ShareSession::new_with_payload(
            SharePayload::ZipArchive {
                entries: entries.clone(),
            },
            vec![first, second],
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
        let (writer, mut reader) = tokio::io::duplex(1024 * 1024);
        let writer_state = http_state.clone();
        let writer_token = token.clone();
        let task = tokio::spawn(async move {
            download::write_zip_archive(&writer_state, &writer_token, entries, writer).await
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
    async fn approved_upload_is_bound_to_requesting_client() {
        let destination = tempfile::tempdir().expect("tempdir");
        let receive = ReceiveSession::new(destination.path().to_path_buf(), 10, true, 1024);
        let token = receive.token.clone();
        let state = AppState::new();
        state.write().await.receive_session = Some(receive);
        let first_phone = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 20));
        let second_phone = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 21));
        let metadata = serde_json::json!({
            "file_name": "photo.jpg",
            "file_size": 5,
            "mime_type": "image/jpeg"
        });
        let response = request_from_with(
            build_router(state.clone(), None),
            first_phone,
            "POST",
            &format!("/api/upload-request/{token}"),
            vec![("content-type", "application/json")],
            Body::from(metadata.to_string()),
        )
        .await;
        assert_eq!(response.status(), StatusCode::ACCEPTED);
        state
            .write()
            .await
            .receive_session
            .as_mut()
            .expect("receive")
            .approve();

        let response = request_from(
            build_router(state.clone(), None),
            second_phone,
            &format!("/api/upload-status/{token}"),
        )
        .await;
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert_eq!(json_body(response).await["error"], "client_mismatch");

        let response = request_from(
            build_router(state, None),
            first_phone,
            &format!("/api/upload-status/{token}"),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn denied_receive_page_uses_upload_copy() {
        let destination = tempfile::tempdir().expect("tempdir");
        let mut receive = ReceiveSession::new(destination.path().to_path_buf(), 10, true, 1024);
        receive.deny(false);
        let token = receive.token.clone();
        let state = AppState::new();
        state.write().await.receive_session = Some(receive);
        let response = request(build_router(state, None), &format!("/u/{token}")).await;
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        let body = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        let html = String::from_utf8_lossy(&body);
        assert!(html.contains("Upload Denied"));
        assert!(html.contains("denied this FluxDrop upload"));
        assert!(!html.contains("download"));
    }

    #[tokio::test]
    async fn upload_page_exposes_client_side_size_limit() {
        let destination = tempfile::tempdir().expect("tempdir");
        let receive = ReceiveSession::new(destination.path().to_path_buf(), 10, true, 4096);
        let token = receive.token.clone();
        let state = AppState::new();
        state.write().await.receive_session = Some(receive);
        let response = request(build_router(state, None), &format!("/u/{token}")).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        let html = String::from_utf8_lossy(&body);
        assert!(html.contains(r#"data-max-upload-bytes="4096""#));
        assert!(html.contains("Maximum upload size"));
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
    async fn upload_metadata_endpoint_keeps_default_body_limit() {
        let destination = tempfile::tempdir().expect("tempdir");
        let receive =
            ReceiveSession::new(destination.path().to_path_buf(), 10, false, 4 * 1024 * 1024);
        let token = receive.token.clone();
        let state = AppState::new();
        state.write().await.receive_session = Some(receive);
        let body = serde_json::json!({
            "file_name": "a".repeat(2 * 1024 * 1024 + 1),
            "file_size": 1,
            "mime_type": "text/plain"
        })
        .to_string();
        let response = request_with(
            build_router(state.clone(), None),
            "POST",
            &format!("/api/upload-request/{token}"),
            vec![("content-type", "application/json")],
            Body::from(body),
        )
        .await;
        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
        assert!(matches!(
            state
                .read()
                .await
                .receive_session
                .as_ref()
                .expect("receive")
                .status,
            ShareStatus::Ready
        ));
    }

    #[tokio::test]
    async fn upload_status_rate_limits_invalid_token_shapes() {
        let state = AppState::new();
        for _ in 0..20 {
            let response =
                request(build_router(state.clone(), None), "/api/upload-status/bad").await;
            assert_eq!(response.status(), StatusCode::NOT_FOUND);
        }
        let response = request(build_router(state, None), "/api/upload-status/bad").await;
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(json_body(response).await["error"], "rate_limited");
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
    async fn test_transfer_server_sends_strict_security_headers() {
        let response = request(build_router(AppState::new(), None), "/").await;
        assert_eq!(response.status(), StatusCode::OK);
        let csp = response
            .headers()
            .get(header::CONTENT_SECURITY_POLICY)
            .and_then(|value| value.to_str().ok())
            .expect("content security policy");
        assert!(csp.contains("form-action 'self'"));
        assert!(csp.contains("frame-ancestors 'none'"));
        assert!(csp.contains("script-src-attr 'none'"));
        assert_eq!(
            response
                .headers()
                .get(header::REFERRER_POLICY)
                .and_then(|value| value.to_str().ok()),
            Some("no-referrer")
        );
        assert_eq!(
            response
                .headers()
                .get(header::X_CONTENT_TYPE_OPTIONS)
                .and_then(|value| value.to_str().ok()),
            Some("nosniff")
        );
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
        assert!(script.contains(r#"{27}$"#));
        assert!(script.contains("target.protocol !== \"https:\""));
        assert!(script.contains("target.hostname !== window.location.hostname"));
    }

    #[tokio::test]
    async fn test_upload_script_has_actionable_error_messages() {
        let response = request(build_router(AppState::new(), None), "/upload.js").await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        let script = String::from_utf8_lossy(&body);
        assert!(script.contains("readApiError"));
        assert!(script.contains("selectedFileTooLarge"));
        assert!(script.contains("Ready to request approval"));
        assert!(script.contains("The file size changed during upload"));
        assert!(script.contains("could not safely store this upload"));
        assert!(script.contains("Too many invalid attempts"));
        assert!(!script.contains("Upload failed:"));
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
