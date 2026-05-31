use crate::events;
use crate::events::EventHandle;
use crate::file_utils::{content_disposition_filename, escape_html, format_file_size};
use crate::share::{ShareSession, ShareStatus, ShareStatusInfo};
use crate::state::{AppState, ServerHandle};
use async_stream::stream;
use axum::body::Body;
use axum::extract::{ConnectInfo, Path, State};
use axum::http::{header, HeaderName, HeaderValue, Request, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use bytes::Bytes;
use chrono::{DateTime, Utc};
use serde::Serialize;
use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tokio::io::AsyncReadExt;
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
pub struct HttpState {
    pub app_state: AppState,
    pub app: Option<EventHandle>,
}

#[derive(Debug, Clone)]
struct ShareSnapshot {
    token: String,
    file_path: PathBuf,
    safe_file_name: String,
    file_size: u64,
    file_size_human: String,
    mime_type: String,
    expires_at: DateTime<Utc>,
    single_use: bool,
    sender_name: String,
    status: ShareStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InvalidReason {
    NotFound,
    Expired,
    Cancelled,
    Completed,
    Denied,
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
}

#[derive(Serialize)]
struct ErrorBody {
    error: &'static str,
    message: &'static str,
}

pub async fn start_server(
    app_state: AppState,
    app: Option<EventHandle>,
    ip: IpAddr,
    port: u16,
) -> std::io::Result<ServerHandle> {
    let listener = TcpListener::bind(SocketAddr::new(ip, port)).await?;
    let address = listener.local_addr()?;
    let shutdown = CancellationToken::new();
    let shutdown_for_task = shutdown.clone();
    let router = build_router(app_state, app);

    #[cfg(not(test))]
    let task = tauri::async_runtime::spawn(async move {
        let service = router.into_make_service_with_connect_info::<SocketAddr>();
        let result = axum::serve(listener, service)
            .with_graceful_shutdown(async move {
                shutdown_for_task.cancelled().await;
            })
            .await;
        if let Err(err) = result {
            eprintln!("FluxDrop local server stopped unexpectedly: {err}");
        }
    });

    #[cfg(test)]
    let task = tokio::spawn(async move {
        let service = router.into_make_service_with_connect_info::<SocketAddr>();
        let result = axum::serve(listener, service)
            .with_graceful_shutdown(async move {
                shutdown_for_task.cancelled().await;
            })
            .await;
        if let Err(err) = result {
            eprintln!("FluxDrop local server stopped unexpectedly: {err}");
        }
    });

    Ok(ServerHandle {
        address,
        shutdown,
        task,
    })
}

pub fn build_router(app_state: AppState, app: Option<EventHandle>) -> Router {
    Router::new()
        .route("/", get(root))
        .route("/d/:token", get(download_page))
        .route("/api/share/:token", get(share_api))
        .route("/download/:token", get(download_file).head(download_head))
        .route("/health", get(health))
        .with_state(HttpState { app_state, app })
        .layer(middleware::from_fn(security_headers))
}

async fn root() -> Html<&'static str> {
    Html("<!doctype html><html><head><meta charset=\"utf-8\"><title>FluxDrop</title></head><body><h1>FluxDrop is running</h1><p>The local transfer server is ready.</p></body></html>")
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
        Ok(snapshot) => {
            {
                let mut guard = state.app_state.write().await;
                let local_address = guard
                    .server
                    .as_ref()
                    .map(|server| server.address.to_string());
                if let Some(share) = guard.current_share.as_mut() {
                    share.mark_phone_connected(addr.ip());
                    let info = share.status_info(
                        local_address,
                        Some("Phone opened the download page.".to_string()),
                    );
                    guard.last_request_status = Some("Phone opened the download page.".to_string());
                    events::emit_share_status(state.app.as_ref(), "phone_connected", &info);
                }
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

    let file = match tokio::fs::File::open(&snapshot.file_path).await {
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

    let app_state = state.app_state.clone();
    let app = state.app.clone();
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
                    events::emit_share_status(app.as_ref(), "progress_updated", &info);
                }
                last_emit = Instant::now();
            }
            yield Ok::<Bytes, std::io::Error>(Bytes::copy_from_slice(&buffer[..read]));
        }

        let completed_info = mark_completed(&app_state, &token).await;
        if let Some(info) = completed_info {
            events::emit_share_status(app.as_ref(), "download_completed", &info);
        }
    };

    response_builder_for_download(&snapshot)
        .body(Body::from_stream(stream))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

fn response_builder_for_download(snapshot: &ShareSnapshot) -> http::response::Builder {
    http::Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, snapshot.mime_type.as_str())
        .header(header::CONTENT_LENGTH, snapshot.file_size.to_string())
        .header(
            header::CONTENT_DISPOSITION,
            format!(
                "attachment; filename*=UTF-8''{}",
                content_disposition_filename(&snapshot.safe_file_name)
            ),
        )
        .header(header::CACHE_CONTROL, "no-store")
        .header(header::PRAGMA, "no-cache")
        .header(header::X_CONTENT_TYPE_OPTIONS, "nosniff")
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
        Some(share) if require_approval && share.approval_required && !share.approved => {
            Some(InvalidReason::ApprovalRequired)
        }
        Some(share) if require_approval && matches!(share.status, ShareStatus::Denied) => {
            Some(InvalidReason::Denied)
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

async fn record_invalid(
    state: &HttpState,
    client_ip: IpAddr,
    reason: InvalidReason,
) -> Result<ShareSnapshot, InvalidReason> {
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
        file_path: share.file_path.clone(),
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
    }
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
    insert_header(headers, header::CONTENT_SECURITY_POLICY, "default-src 'self'; style-src 'self' 'unsafe-inline'; script-src 'none'; img-src 'self' data:; object-src 'none'; base-uri 'none'; frame-ancestors 'none'");
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
    format!(
        r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
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
    .note {{ font-size: .92rem; color: #6b7280; margin-bottom: 0; }}
  </style>
</head>
<body>
  <main>
    <h1>FluxDrop</h1>
    <p>{sender} is sharing one file with this browser over local Wi-Fi.</p>
    <dl>
      <dt>File</dt><dd>{file_name}</dd>
      <dt>Size</dt><dd>{file_size}</dd>
      <dt>Type</dt><dd>{mime_type}</dd>
    </dl>
    <a class="button" href="/download/{token}">Download</a>
    <p class="note">This link is single-use and expires automatically. FluxDrop v0.1 uses local HTTP, so use it only on trusted networks.</p>
  </main>
</body>
</html>"#
    )
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
        InvalidReason::Denied => (
            StatusCode::FORBIDDEN,
            "Transfer Denied",
            "The sender denied this FluxDrop download.",
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
    use std::net::{IpAddr, Ipv4Addr};
    use tower::ServiceExt;

    async fn request(router: Router, uri: &str) -> Response {
        router
            .oneshot(
                Request::builder()
                    .uri(uri)
                    .extension(ConnectInfo(SocketAddr::new(
                        IpAddr::V4(Ipv4Addr::LOCALHOST),
                        50000,
                    )))
                    .body(Body::empty())
                    .expect("request"),
            )
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
}
