use crate::file_utils::format_file_size;
use crate::share::{generate_token, ShareStatus};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::net::IpAddr;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReceiveSession {
    pub id: Uuid,
    pub token: String,
    pub destination_folder: PathBuf,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub approval_required: bool,
    pub approved: bool,
    pub approval_deadline: Option<DateTime<Utc>>,
    pub approval_timed_out: bool,
    pub cancelled: bool,
    pub max_upload_bytes: u64,
    pub file_name: Option<String>,
    pub declared_size: Option<u64>,
    pub mime_type: Option<String>,
    pub bytes_received: u64,
    pub client_ip: Option<IpAddr>,
    pub status: ShareStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReceiveInfo {
    pub id: Uuid,
    pub upload_url: String,
    pub qr_svg: String,
    pub destination_folder_name: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub local_ip: String,
    pub port: u16,
    pub max_upload_bytes: u64,
    pub max_upload_size_human: String,
    pub status: ShareStatus,
    pub approval_required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReceiveStatusInfo {
    pub file_name: Option<String>,
    pub file_size: Option<u64>,
    pub file_size_human: Option<String>,
    pub mime_type: Option<String>,
    pub status: ShareStatus,
    pub bytes_received: u64,
    pub progress_percent: f64,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub approval_deadline: Option<DateTime<Utc>>,
    pub approval_timed_out: bool,
    pub client_ip: Option<String>,
    pub local_address: Option<String>,
    pub destination_folder_name: String,
    pub last_request_status: Option<String>,
}

impl ReceiveSession {
    pub fn new(
        destination_folder: PathBuf,
        expiration_minutes: u32,
        approval_required: bool,
        max_upload_bytes: u64,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            token: generate_token(),
            destination_folder,
            created_at: now,
            expires_at: now + Duration::minutes(i64::from(expiration_minutes)),
            approval_required,
            approved: false,
            approval_deadline: None,
            approval_timed_out: false,
            cancelled: false,
            max_upload_bytes,
            file_name: None,
            declared_size: None,
            mime_type: None,
            bytes_received: 0,
            client_ip: None,
            status: ShareStatus::Ready,
        }
    }

    pub fn is_expired(&self) -> bool {
        Utc::now() >= self.expires_at
    }

    pub fn request_approval(
        &mut self,
        client_ip: IpAddr,
        file_name: String,
        file_size: u64,
        mime_type: Option<String>,
    ) -> bool {
        self.client_ip = Some(client_ip);
        self.file_name = Some(file_name);
        self.declared_size = Some(file_size);
        self.mime_type = mime_type;
        self.bytes_received = 0;
        self.approval_timed_out = false;
        if self.approval_required {
            let now = Utc::now();
            self.approved = false;
            self.status = ShareStatus::AwaitingApproval;
            self.approval_deadline =
                Some(now + Duration::seconds(crate::share::APPROVAL_TIMEOUT_SECONDS));
            true
        } else {
            self.approved = true;
            self.status = ShareStatus::Approved;
            false
        }
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

    pub fn status_info(
        &self,
        local_address: Option<String>,
        last_request_status: Option<String>,
    ) -> ReceiveStatusInfo {
        let progress_percent = match self.declared_size {
            Some(0) => 100.0,
            Some(size) => (self.bytes_received as f64 / size as f64 * 100.0).clamp(0.0, 100.0),
            None => 0.0,
        };
        ReceiveStatusInfo {
            file_name: self.file_name.clone(),
            file_size: self.declared_size,
            file_size_human: self.declared_size.map(format_file_size),
            mime_type: self.mime_type.clone(),
            status: self.status.clone(),
            bytes_received: self.bytes_received,
            progress_percent,
            created_at: self.created_at,
            expires_at: self.expires_at,
            approval_deadline: self.approval_deadline,
            approval_timed_out: self.approval_timed_out,
            client_ip: self.client_ip.map(|ip| ip.to_string()),
            local_address,
            destination_folder_name: self
                .destination_folder
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("Selected folder")
                .to_string(),
            last_request_status,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn approval_request_captures_exact_file_metadata() {
        let mut session = ReceiveSession::new(PathBuf::from("downloads"), 10, true, 1024);
        let requested = session.request_approval(
            "192.168.1.22".parse().expect("ip"),
            "photo.jpg".to_string(),
            512,
            Some("image/jpeg".to_string()),
        );
        assert!(requested);
        assert_eq!(session.file_name.as_deref(), Some("photo.jpg"));
        assert_eq!(session.declared_size, Some(512));
        assert_eq!(session.status, ShareStatus::AwaitingApproval);
    }
}
