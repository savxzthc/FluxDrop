use super::*;

pub(super) async fn download_page(
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

pub(super) async fn share_api(
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

pub(super) async fn download_head(
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

pub(super) async fn download_file(
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

fn single_file_body(state: HttpState, snapshot: &ShareSnapshot, file: tokio::fs::File) -> Body {
    let app_state = state.app_state.clone();
    let token = snapshot.token.clone();
    let file_size = snapshot.file_size;
    let stream = stream! {
        let mut file = file;
        let mut buffer = vec![0_u8; 512 * 1024];
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
    let (writer, reader) = tokio::io::duplex(1024 * 1024);
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

pub(super) async fn write_zip_archive(
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
            && !guard.rate_limiter.consume_invalid_attempt(client_ip)
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

pub(super) async fn mark_approval_timed_out(
    app_state: &AppState,
    token: &str,
) -> Option<ShareStatusInfo> {
    let (info, terminal_share) = {
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
        let terminal_share = share.clone();
        guard.last_request_status = last_request_status;
        (info, terminal_share)
    };
    let _ = crate::history::record_share(app_state, &terminal_share).await;
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

pub(super) async fn mark_completed(app_state: &AppState, token: &str) -> Option<ShareStatusInfo> {
    let (info, terminal_share) = {
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
        (
            share.status_info(local_address, last_request_status),
            share.clone(),
        )
    };
    let _ = crate::history::record_share(app_state, &terminal_share).await;
    Some(info)
}

async fn set_error_status(state: &HttpState, message: &str) {
    let terminal_share = {
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
            Some(share.clone())
        } else {
            None
        }
    };
    if let Some(share) = terminal_share.as_ref() {
        let _ = crate::history::record_share(&state.app_state, share).await;
    }
}
