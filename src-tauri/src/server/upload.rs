use super::*;

pub(super) async fn upload_page(
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
        Err(reason) => upload_error_page_response(reason),
    }
}

pub(super) async fn upload_script() -> Response {
    (
        [(
            header::CONTENT_TYPE,
            "application/javascript; charset=utf-8",
        )],
        UPLOAD_SCRIPT,
    )
        .into_response()
}

pub(super) async fn upload_request(
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

pub(super) async fn upload_status(
    State(state): State<HttpState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Path(token): Path<String>,
) -> Response {
    if !valid_token_shape(&token) {
        return api_error_response(
            record_invalid_reason(&state, addr.ip(), InvalidReason::NotFound).await,
        );
    }
    let mut guard = state.app_state.write().await;
    let receive = match guard.receive_session.as_ref() {
        Some(receive) if receive.token == token => receive,
        _ => {
            if !guard.rate_limiter.consume_invalid_attempt(addr.ip()) {
                return api_error_response(InvalidReason::RateLimited);
            }
            return api_error_response(InvalidReason::NotFound);
        }
    };
    let reason = if receive.cancelled {
        Some(InvalidReason::Cancelled)
    } else if receive.is_expired() {
        Some(InvalidReason::Expired)
    } else if approval_client_mismatch(
        receive.approval_required,
        receive.client_ip,
        addr.ip(),
        &receive.status,
    ) {
        Some(InvalidReason::ClientMismatch)
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

pub(super) async fn upload_file(
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
            let message = Some("Phone upload started.".to_string());
            let info = receive.status_info(local_address, message.clone());
            guard.last_request_status = message;
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
    // Keep incomplete writes in a randomized same-directory .part file, then
    // publish with no-clobber only after the approved body is fully validated.
    let named_temp = match tempfile::Builder::new()
        .prefix(".fluxdrop-upload-")
        .suffix(".part")
        .tempfile_in(&snapshot.destination_folder)
    {
        Ok(file) => file,
        Err(_) => {
            set_upload_error(&state, "FluxDrop could not create a temporary upload file.").await;
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "temp_unavailable"})),
            )
                .into_response();
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
            return (
                StatusCode::INSUFFICIENT_STORAGE,
                Json(serde_json::json!({"error": "write_failed"})),
            )
                .into_response();
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
    if output.flush().await.is_err() {
        drop(output);
        drop(temp_path);
        set_upload_error(
            &state,
            "FluxDrop could not finish writing the incoming file.",
        )
        .await;
        return (
            StatusCode::INSUFFICIENT_STORAGE,
            Json(serde_json::json!({"error": "write_failed"})),
        )
            .into_response();
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
        return (
            StatusCode::CONFLICT,
            Json(serde_json::json!({"error": "store_failed"})),
        )
            .into_response();
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
        Some(receive)
            if approval_client_mismatch(
                receive.approval_required,
                receive.client_ip,
                client_ip,
                &receive.status,
            ) =>
        {
            Some(InvalidReason::ClientMismatch)
        }
        Some(receive) if require_approval && receive.approval_required && !receive.approved => {
            Some(InvalidReason::ApprovalRequired)
        }
        Some(_) => None,
    };
    if let Some(reason) = reason {
        if matches!(reason, InvalidReason::NotFound)
            && !guard.rate_limiter.consume_invalid_attempt(client_ip)
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
    let (info, terminal_receive) = {
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
        let terminal_receive = receive.clone();
        guard.last_request_status = message;
        (info, terminal_receive)
    };
    let _ = crate::history::record_receive(app_state, &terminal_receive).await;
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
        let message = Some("Phone upload in progress.".to_string());
        let info = receive.status_info(local_address, message.clone());
        guard.last_request_status = message;
        events::emit_share_status(state.app.as_ref(), "upload_progress", &info);
    }
}

async fn mark_upload_completed(
    app_state: &AppState,
    token: &str,
    bytes_received: u64,
) -> Option<ReceiveStatusInfo> {
    let (info, terminal_receive) = {
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
        let terminal_receive = receive.clone();
        guard.last_request_status = message;
        (info, terminal_receive)
    };
    let _ = crate::history::record_receive(app_state, &terminal_receive).await;
    Some(info)
}

async fn set_upload_error(state: &HttpState, message: &str) {
    let terminal_receive = {
        let mut guard = state.app_state.write().await;
        let local_address = guard
            .server
            .as_ref()
            .map(|server| server.address.to_string());
        let terminal_receive = if let Some(receive) = guard.receive_session.as_mut() {
            receive.status = ShareStatus::Error(message.to_string());
            let info = receive.status_info(local_address, Some(message.to_string()));
            events::emit_share_status(state.app.as_ref(), "upload_interrupted", &info);
            Some(receive.clone())
        } else {
            None
        };
        guard.last_request_status = Some(message.to_string());
        terminal_receive
    };
    if let Some(receive) = terminal_receive.as_ref() {
        let _ = crate::history::record_receive(&state.app_state, receive).await;
    }
}

fn upload_error_page_response(reason: InvalidReason) -> Response {
    let (status, title, message) = match reason {
        InvalidReason::Expired | InvalidReason::Completed => (
            StatusCode::GONE,
            "Receive Link Expired",
            "This FluxDrop receive link has expired or was already used.",
        ),
        InvalidReason::Cancelled => (
            StatusCode::GONE,
            "Receive Cancelled",
            "The PC cancelled this FluxDrop receive link.",
        ),
        InvalidReason::ApprovalRequired => (
            StatusCode::FORBIDDEN,
            "Waiting for Approval",
            "The PC must approve this upload before it can start.",
        ),
        InvalidReason::ClientMismatch => (
            StatusCode::FORBIDDEN,
            "Different Device",
            "This approval belongs to the phone that requested it. Scan a fresh QR code from the device you want to use.",
        ),
        InvalidReason::Denied => (
            StatusCode::FORBIDDEN,
            "Upload Denied",
            "The PC denied this FluxDrop upload.",
        ),
        InvalidReason::ApprovalTimedOut => (
            StatusCode::REQUEST_TIMEOUT,
            "Approval Timed Out",
            "The PC did not approve this upload within 60 seconds. Start a new receive link and try again.",
        ),
        InvalidReason::RateLimited => (
            StatusCode::TOO_MANY_REQUESTS,
            "Too Many Attempts",
            "Too many invalid receive link attempts were received. Wait a minute and try again.",
        ),
        InvalidReason::NotFound => (
            StatusCode::NOT_FOUND,
            "Receive Link Not Found",
            "This FluxDrop receive link is invalid or has expired.",
        ),
    };
    (status, Html(error_html(title, message))).into_response()
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
