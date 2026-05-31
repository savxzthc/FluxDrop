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
pub const TOKEN_TTL_MINUTES: i64 = 10;

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
    Completed,
    Expired,
    Cancelled,
    Error(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShareSession {
    pub id: Uuid,
    pub token: String,
    pub file_path: PathBuf,
    pub safe_file_name: String,
    pub original_file_name: String,
    pub file_size: u64,
    pub mime_type: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub single_use: bool,
    pub cancelled: bool,
    pub approval_required: bool,
    pub approved: bool,
    pub download_started_at: Option<DateTime<Utc>>,
    pub download_finished_at: Option<DateTime<Utc>>,
    pub bytes_sent: u64,
    pub client_ip: Option<IpAddr>,
    pub status: ShareStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShareInfo {
    pub id: Uuid,
    pub token: String,
    pub file_name: String,
    pub file_size: u64,
    pub file_size_human: String,
    pub mime_type: String,
    pub download_url: String,
    pub qr_svg: String,
    pub expires_at: DateTime<Utc>,
    pub local_ip: String,
    pub port: u16,
    pub status: ShareStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShareStatusInfo {
    pub file_name: String,
    pub file_size: u64,
    pub file_size_human: String,
    pub mime_type: String,
    pub status: ShareStatus,
    pub bytes_sent: u64,
    pub progress_percent: f64,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub download_started_at: Option<DateTime<Utc>>,
    pub download_finished_at: Option<DateTime<Utc>>,
    pub client_ip: Option<String>,
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
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            token: generate_token(),
            file_path,
            safe_file_name,
            original_file_name,
            file_size,
            mime_type,
            created_at: now,
            expires_at: now + Duration::minutes(TOKEN_TTL_MINUTES),
            single_use: true,
            cancelled: false,
            approval_required: false,
            approved: false,
            download_started_at: None,
            download_finished_at: None,
            bytes_sent: 0,
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

    pub fn mark_phone_connected(&mut self, client_ip: IpAddr) {
        self.client_ip = Some(client_ip);
        if !matches!(
            self.status,
            ShareStatus::Downloading | ShareStatus::Completed
        ) {
            self.status = if self.approval_required {
                ShareStatus::AwaitingApproval
            } else {
                ShareStatus::PhoneConnected
            };
        }
    }

    pub fn mark_download_started(&mut self, client_ip: IpAddr) {
        self.status = ShareStatus::Downloading;
        self.download_started_at = Some(Utc::now());
        self.client_ip = Some(client_ip);
        self.bytes_sent = 0;
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
            status: self.status.clone(),
            bytes_sent: self.bytes_sent,
            progress_percent,
            created_at: self.created_at,
            expires_at: self.expires_at,
            download_started_at: self.download_started_at,
            download_finished_at: self.download_finished_at,
            client_ip: self.client_ip.map(|ip| ip.to_string()),
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
                if let Some(share) = guard.current_share.as_mut() {
                    if share.is_expired()
                        && !matches!(
                            share.status,
                            ShareStatus::Expired | ShareStatus::Completed | ShareStatus::Cancelled
                        )
                    {
                        share.expire();
                        Some(share.status_info(local_address, last_request_status))
                    } else {
                        None
                    }
                } else {
                    None
                }
            };

            if let Some(info) = expired_info {
                events::emit_share_status(Some(&app), "share_expired", &info);
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
        assert!(token.len() >= 27);
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
}
