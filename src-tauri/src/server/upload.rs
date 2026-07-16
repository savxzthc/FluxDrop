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
    if metadata.files.is_empty()
        || metadata.file_count != metadata.files.len()
        || metadata.file_count > 1_000
        || metadata
            .files
            .iter()
            .any(|file| file.file_name.trim().is_empty())
    {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "invalid_manifest"})),
        )
            .into_response();
    }
    let computed_total = match metadata
        .files
        .iter()
        .try_fold(0_u64, |total, file| total.checked_add(file.size))
    {
        Some(total) => total,
        None => {
            return (
                StatusCode::PAYLOAD_TOO_LARGE,
                Json(serde_json::json!({"error": "too_large"})),
            )
                .into_response()
        }
    };
    if computed_total != metadata.total_size {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "invalid_manifest"})),
        )
            .into_response();
    }
    if computed_total > snapshot.max_upload_bytes {
        return (
            StatusCode::PAYLOAD_TOO_LARGE,
            Json(serde_json::json!({
                "error": "too_large",
                "message": format!("The selected batch exceeds the {} limit.", format_file_size(snapshot.max_upload_bytes))
            })),
        )
            .into_response();
    }

    let manifest = metadata
        .files
        .into_iter()
        .map(|file| ReceiveFileManifest {
            file_name: sanitize_filename(&file.file_name),
            mime_type: file
                .mime_type
                .map(|value| value.trim().chars().take(255).collect())
                .filter(|value: &String| !value.is_empty()),
            size: file.size,
        })
        .collect::<Vec<_>>();
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
        let approval_requested = receive.request_approval(addr.ip(), manifest);
        let message = if approval_requested {
            "Phone requested approval to upload a file batch."
        } else {
            "Phone upload batch accepted without approval."
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
    let overhead_allowance =
        (1024 * 1024_u64).saturating_add((snapshot.files.len() as u64).saturating_mul(4 * 1024));
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

    let mut temporary_files = Vec::with_capacity(snapshot.files.len());
    let mut received = 0_u64;
    let mut last_emit = Instant::now() - Duration::from_millis(250);
    for expected in &snapshot.files {
        let mut field = match multipart.next_field().await {
            Ok(Some(field)) => field,
            Ok(None) => {
                return upload_failure(
                    &state,
                    StatusCode::BAD_REQUEST,
                    "missing_file",
                    "The phone did not include every approved file.",
                )
                .await
            }
            Err(_) => {
                return upload_failure(
                    &state,
                    StatusCode::BAD_REQUEST,
                    "invalid_multipart",
                    "FluxDrop could not parse the upload body.",
                )
                .await
            }
        };
        let upload_name = sanitize_filename(field.file_name().unwrap_or("upload"));
        let upload_mime = field.content_type().map(ToOwned::to_owned);
        if field.name() != Some("files")
            || upload_name != expected.file_name
            || upload_mime.as_deref() != expected.mime_type.as_deref()
        {
            return upload_failure(
                &state,
                StatusCode::CONFLICT,
                "metadata_mismatch",
                "An uploaded file did not match the approved batch manifest.",
            )
            .await;
        }
        let named_temp = match tempfile::Builder::new()
            .prefix(".fluxdrop-upload-")
            .suffix(".part")
            .tempfile_in(&snapshot.destination_folder)
        {
            Ok(file) => file,
            Err(_) => {
                return upload_failure(
                    &state,
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "temp_unavailable",
                    "FluxDrop could not create a temporary upload file.",
                )
                .await
            }
        };
        let (std_file, temp_path) = named_temp.into_parts();
        let mut output = tokio::fs::File::from_std(std_file);
        let mut file_received = 0_u64;
        loop {
            let chunk = match field.chunk().await {
                Ok(Some(chunk)) => chunk,
                Ok(None) => break,
                Err(_) => {
                    return upload_failure(
                        &state,
                        StatusCode::BAD_REQUEST,
                        "interrupted",
                        "The phone upload was interrupted.",
                    )
                    .await
                }
            };
            if !receive_upload_active(&state.app_state, &token).await {
                return (
                    StatusCode::GONE,
                    Json(serde_json::json!({"error": "cancelled"})),
                )
                    .into_response();
            }
            file_received = file_received.saturating_add(chunk.len() as u64);
            received = received.saturating_add(chunk.len() as u64);
            if file_received > expected.size || received > snapshot.max_upload_bytes {
                return upload_failure(
                    &state,
                    StatusCode::PAYLOAD_TOO_LARGE,
                    "size_mismatch",
                    "An uploaded file exceeded its approved size.",
                )
                .await;
            }
            if output.write_all(&chunk).await.is_err() {
                return upload_failure(
                    &state,
                    StatusCode::INSUFFICIENT_STORAGE,
                    "write_failed",
                    "FluxDrop could not write an incoming file.",
                )
                .await;
            }
            if last_emit.elapsed() >= Duration::from_millis(150) {
                emit_upload_progress(&state, received).await;
                last_emit = Instant::now();
            }
        }
        if file_received != expected.size {
            return upload_failure(
                &state,
                StatusCode::BAD_REQUEST,
                "size_mismatch",
                "An uploaded file size did not match the approved batch manifest.",
            )
            .await;
        }
        if output.flush().await.is_err() || output.sync_all().await.is_err() {
            return upload_failure(
                &state,
                StatusCode::INSUFFICIENT_STORAGE,
                "write_failed",
                "FluxDrop could not finish writing an incoming file.",
            )
            .await;
        }
        drop(output);
        temporary_files.push(temp_path);
    }
    match multipart.next_field().await {
        Ok(None) => {}
        Ok(Some(_)) => {
            return upload_failure(
                &state,
                StatusCode::CONFLICT,
                "metadata_mismatch",
                "The upload included a file that was not in the approved batch.",
            )
            .await
        }
        Err(_) => {
            return upload_failure(
                &state,
                StatusCode::BAD_REQUEST,
                "invalid_multipart",
                "FluxDrop could not finish parsing the upload body.",
            )
            .await
        }
    }
    if snapshot.total_size != Some(received) {
        return upload_failure(
            &state,
            StatusCode::BAD_REQUEST,
            "size_mismatch",
            "The uploaded batch size did not match the approved manifest.",
        )
        .await;
    }

    let final_paths = unique_destination_paths(
        &snapshot.destination_folder,
        snapshot.files.iter().map(|file| file.file_name.as_str()),
    )
    .await;
    let mut finalized = Vec::with_capacity(final_paths.len());
    for (temp_path, final_path) in temporary_files.into_iter().zip(final_paths) {
        let persist_path = final_path.clone();
        let persisted =
            tokio::task::spawn_blocking(move || temp_path.persist_noclobber(&persist_path)).await;
        if !matches!(persisted, Ok(Ok(_))) {
            rollback_finalized_files(&finalized).await;
            return upload_failure(&state, StatusCode::CONFLICT, "store_failed", "FluxDrop could not publish the complete upload batch; newly published files were rolled back.").await;
        }
        finalized.push(final_path);
    }

    let info = mark_upload_completed(&state.app_state, &token, received).await;
    if let Some(info) = info {
        events::emit_share_status(state.app.as_ref(), "upload_completed", &info);
    }
    Json(serde_json::json!({
        "status": "ok",
        "file_count": snapshot.files.len(),
        "total_size": received,
        "file_names": snapshot.files.iter().map(|file| &file.file_name).collect::<Vec<_>>()
    }))
    .into_response()
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
        files: receive.files.clone(),
        total_size: receive.total_size,
    }
}

async fn upload_failure(
    state: &HttpState,
    status: StatusCode,
    code: &str,
    message: &str,
) -> Response {
    set_upload_error(state, message).await;
    (status, Json(serde_json::json!({"error": code}))).into_response()
}

async fn receive_upload_active(app_state: &AppState, token: &str) -> bool {
    let guard = app_state.read().await;
    guard.receive_session.as_ref().is_some_and(|receive| {
        receive.token == token
            && !receive.cancelled
            && !receive.is_expired()
            && matches!(receive.status, ShareStatus::Uploading)
    })
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

async fn unique_destination_paths<'a>(
    folder: &std::path::Path,
    file_names: impl Iterator<Item = &'a str>,
) -> Vec<std::path::PathBuf> {
    let mut reserved = std::collections::HashSet::new();
    let mut result = Vec::new();
    for file_name in file_names {
        let path = std::path::Path::new(file_name);
        let stem = path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("upload");
        let extension = path.extension().and_then(|value| value.to_str());
        let mut index = 1_u64;
        loop {
            let candidate_name = if index == 1 {
                file_name.to_string()
            } else {
                match extension {
                    Some(extension) => format!("{stem} ({index}).{extension}"),
                    None => format!("{stem} ({index})"),
                }
            };
            let candidate = folder.join(candidate_name);
            if !reserved.contains(&candidate)
                && !tokio::fs::try_exists(&candidate).await.unwrap_or(true)
            {
                reserved.insert(candidate.clone());
                result.push(candidate);
                break;
            }
            index = index.saturating_add(1);
        }
    }
    result
}

pub(super) async fn rollback_finalized_files(paths: &[std::path::PathBuf]) {
    for path in paths.iter().rev() {
        let _ = tokio::fs::remove_file(path).await;
    }
}
