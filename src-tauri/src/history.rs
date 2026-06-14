use crate::receive::ReceiveSession;
use crate::share::{ShareSession, ShareStatus};
use crate::state::AppState;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub const HISTORY_FILE_NAME: &str = "history.json";
const MAX_HISTORY_ENTRIES: usize = 100;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TransferDirection {
    Send,
    Receive,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TransferOutcome {
    Completed,
    Denied,
    TimedOut,
    Cancelled,
    Expired,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RepeatTarget {
    Send { paths: Vec<PathBuf> },
    Receive { destination_folder: PathBuf },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryRecord {
    pub id: Uuid,
    pub direction: TransferDirection,
    pub file_name: String,
    pub file_size: Option<u64>,
    pub file_count: usize,
    pub is_archive: bool,
    pub mime_type: Option<String>,
    pub created_at: DateTime<Utc>,
    pub finished_at: DateTime<Utc>,
    pub client_ip: Option<String>,
    pub outcome: TransferOutcome,
    pub repeat: RepeatTarget,
}

#[derive(Debug, Clone, Serialize)]
pub struct HistoryEntry {
    pub id: Uuid,
    pub direction: TransferDirection,
    pub file_name: String,
    pub file_size: Option<u64>,
    pub file_size_human: Option<String>,
    pub file_count: usize,
    pub is_archive: bool,
    pub mime_type: Option<String>,
    pub created_at: DateTime<Utc>,
    pub finished_at: DateTime<Utc>,
    pub client_ip: Option<String>,
    pub outcome: TransferOutcome,
    pub can_repeat: bool,
}

impl HistoryRecord {
    pub fn from_share(share: &ShareSession) -> Option<Self> {
        Some(Self {
            id: share.id,
            direction: TransferDirection::Send,
            file_name: share.safe_file_name.clone(),
            file_size: Some(share.file_size),
            file_count: share.file_count,
            is_archive: share.is_archive,
            mime_type: Some(share.mime_type.clone()),
            created_at: share.created_at,
            finished_at: Utc::now(),
            client_ip: share.client_ip.map(|ip| ip.to_string()),
            outcome: terminal_outcome(&share.status, share.approval_timed_out)?,
            repeat: RepeatTarget::Send {
                paths: share.source_paths.clone(),
            },
        })
    }

    pub fn from_receive(receive: &ReceiveSession) -> Option<Self> {
        let destination_name = receive
            .destination_folder
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("selected folder");
        Some(Self {
            id: receive.id,
            direction: TransferDirection::Receive,
            file_name: receive
                .file_name
                .clone()
                .unwrap_or_else(|| format!("Receive to {destination_name}")),
            file_size: receive.declared_size,
            file_count: usize::from(receive.file_name.is_some()),
            is_archive: false,
            mime_type: receive.mime_type.clone(),
            created_at: receive.created_at,
            finished_at: Utc::now(),
            client_ip: receive.client_ip.map(|ip| ip.to_string()),
            outcome: terminal_outcome(&receive.status, receive.approval_timed_out)?,
            repeat: RepeatTarget::Receive {
                destination_folder: receive.destination_folder.clone(),
            },
        })
    }

    pub fn public_entry(&self) -> HistoryEntry {
        let can_repeat = match &self.repeat {
            RepeatTarget::Send { paths } => {
                !paths.is_empty() && paths.iter().all(|path| path.exists())
            }
            RepeatTarget::Receive { destination_folder } => destination_folder.is_dir(),
        };
        HistoryEntry {
            id: self.id,
            direction: self.direction,
            file_name: self.file_name.clone(),
            file_size: self.file_size,
            file_size_human: self.file_size.map(crate::file_utils::format_file_size),
            file_count: self.file_count,
            is_archive: self.is_archive,
            mime_type: self.mime_type.clone(),
            created_at: self.created_at,
            finished_at: self.finished_at,
            client_ip: self.client_ip.clone(),
            outcome: self.outcome,
            can_repeat,
        }
    }
}

fn terminal_outcome(status: &ShareStatus, approval_timed_out: bool) -> Option<TransferOutcome> {
    match status {
        ShareStatus::Completed => Some(TransferOutcome::Completed),
        ShareStatus::Denied if approval_timed_out => Some(TransferOutcome::TimedOut),
        ShareStatus::Denied => Some(TransferOutcome::Denied),
        ShareStatus::Cancelled => Some(TransferOutcome::Cancelled),
        ShareStatus::Expired => Some(TransferOutcome::Expired),
        ShareStatus::Error(_) => Some(TransferOutcome::Failed),
        _ => None,
    }
}

pub fn load(path: &Path) -> Result<Vec<HistoryRecord>, String> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let contents = fs::read_to_string(path)
        .map_err(|err| format!("FluxDrop could not read transfer history: {err}"))?;
    let mut records = serde_json::from_str::<Vec<HistoryRecord>>(&contents)
        .map_err(|err| format!("FluxDrop transfer history is invalid JSON: {err}"))?;
    records.truncate(MAX_HISTORY_ENTRIES);
    Ok(records)
}

pub fn save(path: &Path, records: &[HistoryRecord]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "FluxDrop history path has no parent directory.".to_string())?;
    fs::create_dir_all(parent)
        .map_err(|err| format!("FluxDrop could not create its history directory: {err}"))?;
    let contents = serde_json::to_vec_pretty(records)
        .map_err(|err| format!("FluxDrop could not serialize transfer history: {err}"))?;
    let temporary = temporary_path(path);
    fs::write(&temporary, contents)
        .map_err(|err| format!("FluxDrop could not write transfer history: {err}"))?;
    if path.exists() {
        fs::remove_file(path)
            .map_err(|err| format!("FluxDrop could not replace old transfer history: {err}"))?;
    }
    fs::rename(&temporary, path)
        .map_err(|err| format!("FluxDrop could not finish saving transfer history: {err}"))
}

pub async fn record_share(state: &AppState, share: &ShareSession) -> Result<(), String> {
    if let Some(record) = HistoryRecord::from_share(share) {
        insert_record(state, record).await?;
    }
    Ok(())
}

pub async fn record_receive(state: &AppState, receive: &ReceiveSession) -> Result<(), String> {
    if let Some(record) = HistoryRecord::from_receive(receive) {
        insert_record(state, record).await?;
    }
    Ok(())
}

async fn insert_record(state: &AppState, record: HistoryRecord) -> Result<(), String> {
    let mut guard = state.write().await;
    if guard.history.iter().any(|entry| entry.id == record.id) {
        return Ok(());
    }
    let mut next = guard.history.clone();
    next.insert(0, record);
    next.truncate(MAX_HISTORY_ENTRIES);
    if let Some(path) = guard.history_path.as_deref() {
        save(path, &next)?;
    }
    guard.history = next;
    Ok(())
}

fn temporary_path(path: &Path) -> PathBuf {
    let mut temporary = path.as_os_str().to_os_string();
    temporary.push(".tmp");
    PathBuf::from(temporary)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::share::ShareSession;

    #[test]
    fn history_round_trip_contains_no_transfer_token() {
        let directory = tempfile::tempdir().expect("tempdir");
        let path = directory.path().join(HISTORY_FILE_NAME);
        let source = directory.path().join("report.txt");
        fs::write(&source, b"report").expect("source file");
        let mut share = ShareSession::new(
            source,
            "report.txt".to_string(),
            "report.txt".to_string(),
            6,
            "text/plain".to_string(),
        );
        let token = share.token.clone();
        share.mark_download_completed();
        let records = vec![HistoryRecord::from_share(&share).expect("terminal record")];

        save(&path, &records).expect("save history");
        let serialized = fs::read_to_string(&path).expect("read history");
        assert!(!serialized.contains(&token));
        assert!(!serialized.contains("download_url"));
        assert_eq!(load(&path).expect("load history").len(), 1);
    }

    #[tokio::test]
    async fn duplicate_session_is_recorded_once() {
        let directory = tempfile::tempdir().expect("tempdir");
        let history_path = directory.path().join(HISTORY_FILE_NAME);
        let state = AppState::with_storage(
            crate::settings::AppSettings::default(),
            None,
            Vec::new(),
            Some(history_path),
        );
        let mut share = ShareSession::new(
            PathBuf::from("report.txt"),
            "report.txt".to_string(),
            "report.txt".to_string(),
            6,
            "text/plain".to_string(),
        );
        share.mark_download_completed();

        record_share(&state, &share).await.expect("first record");
        record_share(&state, &share)
            .await
            .expect("duplicate record");

        assert_eq!(state.read().await.history.len(), 1);
    }
}
