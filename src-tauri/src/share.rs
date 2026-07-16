#[cfg(not(test))]
use crate::events;
use crate::events::EventHandle;
use crate::file_utils::format_file_size;
use crate::state::AppState;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use chrono::{DateTime, Duration, Utc};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::net::IpAddr;
use std::path::PathBuf;
use uuid::Uuid;

pub const TOKEN_BYTES: usize = 20;
pub const TOKEN_CHARS: usize = 27;
pub const TOKEN_TTL_MINUTES: i64 = 10;
pub const APPROVAL_TIMEOUT_SECONDS: i64 = 60;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", content = "message")]
pub enum ShareStatus {
    Idle,
    Preparing,
    Ready,
    Waiting,
    PhoneConnected,
    AwaitingApproval,
    Approved,
    Denied,
    Downloading,
    Uploading,
    Completed,
    Expired,
    Cancelled,
    Error(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveEntrySource {
    pub source_path: Option<PathBuf>,
    pub archive_path: String,
    pub size: u64,
    pub is_directory: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SharePayload {
    SingleFile { path: PathBuf },
    ZipArchive { entries: Vec<ArchiveEntrySource> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShareSession {
    pub id: Uuid,
    pub token: String,
    pub payload: SharePayload,
    pub source_paths: Vec<PathBuf>,
    pub safe_file_name: String,
    pub original_file_name: String,
    pub file_size: u64,
    pub mime_type: String,
    pub file_count: usize,
    pub is_archive: bool,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub single_use: bool,
    pub cancelled: bool,
    pub approval_required: bool,
    pub approved: bool,
    pub approval_requested_at: Option<DateTime<Utc>>,
    pub approval_deadline: Option<DateTime<Utc>>,
    pub approval_timed_out: bool,
    pub download_started_at: Option<DateTime<Utc>>,
    pub download_finished_at: Option<DateTime<Utc>>,
    pub bytes_sent: u64,
    #[serde(default)]
    pub served_intervals: Vec<(u64, u64)>,
    pub client_ip: Option<IpAddr>,
    pub status: ShareStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShareInfo {
    pub id: Uuid,
    pub file_name: String,
    pub file_size: u64,
    pub file_size_human: String,
    pub mime_type: String,
    pub file_count: usize,
    pub is_archive: bool,
    pub download_url: String,
    pub qr_svg: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub local_ip: String,
    pub port: u16,
    pub status: ShareStatus,
    pub approval_required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShareStatusInfo {
    pub file_name: String,
    pub file_size: u64,
    pub file_size_human: String,
    pub mime_type: String,
    pub file_count: usize,
    pub is_archive: bool,
    pub status: ShareStatus,
    pub bytes_sent: u64,
    pub progress_percent: f64,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub download_started_at: Option<DateTime<Utc>>,
    pub download_finished_at: Option<DateTime<Utc>>,
    pub client_ip: Option<String>,
    pub approval_deadline: Option<DateTime<Utc>>,
    pub approval_timed_out: bool,
    pub local_address: Option<String>,
    pub last_request_status: Option<String>,
}

impl ShareSession {
    pub fn new(
        file_path: PathBuf,
        safe_file_name: String,
        original_file_name: String,
        file_size: u64,
        mime_type: String,
    ) -> Self {
        Self::new_with_options(
            file_path,
            safe_file_name,
            original_file_name,
            file_size,
            mime_type,
            TOKEN_TTL_MINUTES as u32,
            true,
            true,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new_with_options(
        file_path: PathBuf,
        safe_file_name: String,
        original_file_name: String,
        file_size: u64,
        mime_type: String,
        expiration_minutes: u32,
        single_use: bool,
        approval_required: bool,
    ) -> Self {
        Self::new_with_payload(
            SharePayload::SingleFile {
                path: file_path.clone(),
            },
            vec![file_path],
            safe_file_name,
            original_file_name,
            file_size,
            mime_type,
            1,
            false,
            expiration_minutes,
            single_use,
            approval_required,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new_with_payload(
        payload: SharePayload,
        source_paths: Vec<PathBuf>,
        safe_file_name: String,
        original_file_name: String,
        file_size: u64,
        mime_type: String,
        file_count: usize,
        is_archive: bool,
        expiration_minutes: u32,
        single_use: bool,
        approval_required: bool,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            token: generate_token(),
            payload,
            source_paths,
            safe_file_name,
            original_file_name,
            file_size,
            mime_type,
            file_count,
            is_archive,
            created_at: now,
            expires_at: now + Duration::minutes(i64::from(expiration_minutes)),
            single_use,
            cancelled: false,
            approval_required,
            approved: false,
            approval_requested_at: None,
            approval_deadline: None,
            approval_timed_out: false,
            download_started_at: None,
            download_finished_at: None,
            bytes_sent: 0,
            served_intervals: Vec::new(),
            client_ip: None,
            status: ShareStatus::Preparing,
        }
    }

    pub fn is_valid(&self) -> bool {
        !(self.cancelled
            || self.is_expired()
            || self.single_use
                && matches!(self.status, ShareStatus::Completed | ShareStatus::Expired))
    }

    pub fn is_expired(&self) -> bool {
        Utc::now() >= self.expires_at
    }

    pub fn cancel(&mut self) {
        self.cancelled = true;
        self.status = ShareStatus::Cancelled;
    }

    pub fn expire(&mut self) {
        self.status = ShareStatus::Expired;
    }

    pub fn mark_phone_connected(&mut self, client_ip: IpAddr) -> bool {
        self.client_ip = Some(client_ip);
        if matches!(
            self.status,
            ShareStatus::Approved
                | ShareStatus::Denied
                | ShareStatus::Downloading
                | ShareStatus::Completed
                | ShareStatus::Expired
                | ShareStatus::Cancelled
                | ShareStatus::Error(_)
        ) {
            return false;
        }

        if self.approval_required && !self.approved {
            if !matches!(self.status, ShareStatus::AwaitingApproval) {
                let now = Utc::now();
                self.status = ShareStatus::AwaitingApproval;
                self.approval_requested_at = Some(now);
                self.approval_deadline = Some(now + Duration::seconds(APPROVAL_TIMEOUT_SECONDS));
                self.approval_timed_out = false;
                return true;
            }
        } else {
            self.status = ShareStatus::PhoneConnected;
        }
        false
    }

    pub fn approve(&mut self) {
        self.approved = true;
        self.approval_timed_out = false;
        self.status = ShareStatus::Approved;
    }

    pub fn deny(&mut self, timed_out: bool) {
        self.approved = false;
        self.approval_timed_out = timed_out;
        self.status = ShareStatus::Denied;
    }

    pub fn mark_download_started(&mut self, client_ip: IpAddr) {
        self.status = ShareStatus::Downloading;
        self.download_started_at.get_or_insert_with(Utc::now);
        self.client_ip = Some(client_ip);
    }

    pub fn mark_download_completed(&mut self) {
        self.status = ShareStatus::Completed;
        self.download_finished_at = Some(Utc::now());
        self.bytes_sent = self.file_size;
        if self.single_use {
            self.expires_at = Utc::now();
        }
    }

    pub fn update_progress(&mut self, bytes_sent: u64) {
        self.bytes_sent = bytes_sent.min(self.file_size);
    }

    /// Records a fully served half-open byte interval and returns true once the
    /// union of completed intervals covers the whole ordinary file.
    pub fn record_served_interval(&mut self, start: u64, end_exclusive: u64) -> bool {
        if start >= end_exclusive || start >= self.file_size {
            return self.file_size == 0;
        }
        let mut merged = (start, end_exclusive.min(self.file_size));
        let mut intervals = Vec::with_capacity(self.served_intervals.len() + 1);
        for interval in self.served_intervals.drain(..) {
            if interval.1 < merged.0 {
                intervals.push(interval);
            } else if merged.1 < interval.0 {
                intervals.push(merged);
                merged = interval;
            } else {
                merged.0 = merged.0.min(interval.0);
                merged.1 = merged.1.max(interval.1);
            }
        }
        intervals.push(merged);
        intervals.sort_unstable_by_key(|interval| interval.0);
        self.served_intervals = intervals;
        self.bytes_sent = self
            .served_intervals
            .iter()
            .map(|(start, end)| end.saturating_sub(*start))
            .sum::<u64>()
            .min(self.file_size);
        self.file_size == 0
            || matches!(self.served_intervals.as_slice(), [(0, end)] if *end >= self.file_size)
    }

    pub fn status_info(
        &self,
        local_address: Option<String>,
        last_request_status: Option<String>,
    ) -> ShareStatusInfo {
        let progress_percent = if self.file_size == 0 {
            100.0
        } else {
            (self.bytes_sent as f64 / self.file_size as f64 * 100.0).clamp(0.0, 100.0)
        };

        ShareStatusInfo {
            file_name: self.safe_file_name.clone(),
            file_size: self.file_size,
            file_size_human: format_file_size(self.file_size),
            mime_type: self.mime_type.clone(),
            file_count: self.file_count,
            is_archive: self.is_archive,
            status: self.status.clone(),
            bytes_sent: self.bytes_sent,
            progress_percent,
            created_at: self.created_at,
            expires_at: self.expires_at,
            download_started_at: self.download_started_at,
            download_finished_at: self.download_finished_at,
            client_ip: self.client_ip.map(|ip| ip.to_string()),
            approval_deadline: self.approval_deadline,
            approval_timed_out: self.approval_timed_out,
            local_address,
            last_request_status,
        }
    }
}

pub fn generate_token() -> String {
    let mut bytes = [0_u8; TOKEN_BYTES];
    rand::thread_rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

#[cfg(not(test))]
pub fn spawn_expiration_task(state: AppState, app: EventHandle) {
    tauri::async_runtime::spawn(async move {
        let mut ticker = tokio::time::interval(std::time::Duration::from_secs(30));
        loop {
            ticker.tick().await;
            let expired_info = {
                let mut guard = state.write().await;
                let local_address = guard
                    .server
                    .as_ref()
                    .map(|server| server.address.to_string());
                let last_request_status = guard.last_request_status.clone();
                let share_expired = if let Some(share) = guard.current_share.as_mut() {
                    if share.is_expired()
                        && !matches!(
                            share.status,
                            ShareStatus::Expired | ShareStatus::Completed | ShareStatus::Cancelled
                        )
                    {
                        share.expire();
                        Some((
                            share.status_info(local_address.clone(), last_request_status),
                            share.clone(),
                        ))
                    } else {
                        None
                    }
                } else {
                    None
                };
                let receive_expired = if let Some(receive) = guard.receive_session.as_mut() {
                    if receive.is_expired()
                        && !matches!(
                            receive.status,
                            ShareStatus::Expired | ShareStatus::Completed | ShareStatus::Cancelled
                        )
                    {
                        receive.status = ShareStatus::Expired;
                        Some((
                            receive.status_info(
                                local_address,
                                Some("Receive link expired.".to_string()),
                            ),
                            receive.clone(),
                        ))
                    } else {
                        None
                    }
                } else {
                    None
                };
                (share_expired, receive_expired)
            };

            if let Some((info, share)) = expired_info.0 {
                let _ = crate::history::record_share(&state, &share).await;
                events::emit_share_status(Some(&app), "share_expired", &info);
            }
            if let Some((info, receive)) = expired_info.1 {
                let _ = crate::history::record_receive(&state, &receive).await;
                events::emit_share_status(Some(&app), "receive_expired", &info);
            }
        }
    });
}

#[cfg(test)]
pub fn spawn_expiration_task(_state: AppState, _app: EventHandle) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_token_length_and_charset() {
        let token = generate_token();
        assert_eq!(token.len(), TOKEN_CHARS);
        assert!(token
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
        assert!(!token.contains('='));
    }

    #[test]
    fn test_generate_token_unique() {
        let one = generate_token();
        let two = generate_token();
        assert_ne!(one, two);
    }

    #[test]
    fn test_expired_session_invalid() {
        let mut share = ShareSession::new(
            PathBuf::from("file.txt"),
            "file.txt".into(),
            "file.txt".into(),
            10,
            "text/plain".into(),
        );
        share.expires_at = Utc::now() - Duration::seconds(1);
        assert!(!share.is_valid());
    }

    #[test]
    fn test_cancelled_session_invalid() {
        let mut share = ShareSession::new(
            PathBuf::from("file.txt"),
            "file.txt".into(),
            "file.txt".into(),
            10,
            "text/plain".into(),
        );
        share.cancel();
        assert!(!share.is_valid());
    }

    #[test]
    fn test_completed_single_use_invalid() {
        let mut share = ShareSession::new(
            PathBuf::from("file.txt"),
            "file.txt".into(),
            "file.txt".into(),
            10,
            "text/plain".into(),
        );
        share.mark_download_completed();
        assert!(!share.is_valid());
    }

    #[test]
    fn test_approval_is_required_by_default() {
        let share = ShareSession::new(
            PathBuf::from("file.txt"),
            "file.txt".into(),
            "file.txt".into(),
            10,
            "text/plain".into(),
        );
        assert!(share.approval_required);
        assert!(!share.approved);
    }

    #[test]
    fn test_phone_connection_requests_approval_once() {
        let mut share = ShareSession::new(
            PathBuf::from("file.txt"),
            "file.txt".into(),
            "file.txt".into(),
            10,
            "text/plain".into(),
        );
        let ip = "192.168.1.20".parse().expect("ip");
        assert!(share.mark_phone_connected(ip));
        assert_eq!(share.status, ShareStatus::AwaitingApproval);
        let deadline = share.approval_deadline;
        assert!(!share.mark_phone_connected(ip));
        assert_eq!(share.approval_deadline, deadline);
    }

    #[test]
    fn test_phone_refresh_does_not_overwrite_decision() {
        let mut share = ShareSession::new(
            PathBuf::from("file.txt"),
            "file.txt".into(),
            "file.txt".into(),
            10,
            "text/plain".into(),
        );
        let ip = "192.168.1.20".parse().expect("ip");
        share.mark_phone_connected(ip);
        share.approve();
        assert!(!share.mark_phone_connected(ip));
        assert_eq!(share.status, ShareStatus::Approved);

        share.deny(false);
        assert!(!share.mark_phone_connected(ip));
        assert_eq!(share.status, ShareStatus::Denied);
    }

    #[test]
    fn test_custom_share_defaults_are_applied() {
        let share = ShareSession::new_with_options(
            PathBuf::from("file.txt"),
            "file.txt".into(),
            "file.txt".into(),
            10,
            "text/plain".into(),
            30,
            false,
            false,
        );
        assert!(!share.single_use);
        assert!(!share.approval_required);
        let remaining = share.expires_at - share.created_at;
        assert_eq!(remaining.num_minutes(), 30);
    }

    #[test]
    fn served_intervals_merge_and_only_complete_full_coverage() {
        let mut share = ShareSession::new(
            PathBuf::from("file.txt"),
            "file.txt".into(),
            "file.txt".into(),
            100,
            "text/plain".into(),
        );
        assert!(!share.record_served_interval(50, 75));
        assert!(!share.record_served_interval(0, 25));
        assert!(!share.record_served_interval(20, 55));
        assert_eq!(share.served_intervals, vec![(0, 75)]);
        assert_eq!(share.bytes_sent, 75);
        assert!(share.record_served_interval(75, 100));
        assert_eq!(share.served_intervals, vec![(0, 100)]);
    }
}
