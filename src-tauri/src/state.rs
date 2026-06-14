use crate::history::HistoryRecord;
use crate::network::NetworkAddress;
use crate::rate_limit::RateLimiter;
use crate::receive::ReceiveSession;
use crate::settings::AppSettings;
use crate::share::ShareSession;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

#[cfg(not(test))]
pub type ServerTaskHandle = tauri::async_runtime::JoinHandle<()>;

#[cfg(test)]
pub type ServerTaskHandle = tokio::task::JoinHandle<()>;

#[derive(Clone)]
pub struct AppState {
    inner: Arc<RwLock<RuntimeState>>,
}

pub struct RuntimeState {
    pub current_share: Option<ShareSession>,
    pub receive_session: Option<ReceiveSession>,
    pub server: Option<ServerHandle>,
    pub rate_limiter: RateLimiter,
    pub detected_addresses: Vec<NetworkAddress>,
    pub last_request_status: Option<String>,
    pub settings: AppSettings,
    pub settings_path: Option<PathBuf>,
    pub history: Vec<HistoryRecord>,
    pub history_path: Option<PathBuf>,
}

pub struct ServerHandle {
    pub address: SocketAddr,
    pub onboarding_address: SocketAddr,
    pub shutdown: CancellationToken,
    pub task: Option<ServerTaskHandle>,
    pub onboarding_task: Option<ServerTaskHandle>,
}

impl AppState {
    pub fn new() -> Self {
        Self::with_settings(AppSettings::default(), None)
    }

    pub fn with_settings(settings: AppSettings, settings_path: Option<PathBuf>) -> Self {
        Self::with_storage(settings, settings_path, Vec::new(), None)
    }

    pub fn with_storage(
        settings: AppSettings,
        settings_path: Option<PathBuf>,
        history: Vec<HistoryRecord>,
        history_path: Option<PathBuf>,
    ) -> Self {
        Self {
            inner: Arc::new(RwLock::new(RuntimeState {
                current_share: None,
                receive_session: None,
                server: None,
                rate_limiter: RateLimiter::new(20, 60),
                detected_addresses: Vec::new(),
                last_request_status: None,
                settings,
                settings_path,
                history,
                history_path,
            })),
        }
    }

    pub async fn read(&self) -> tokio::sync::RwLockReadGuard<'_, RuntimeState> {
        self.inner.read().await
    }

    pub async fn write(&self) -> tokio::sync::RwLockWriteGuard<'_, RuntimeState> {
        self.inner.write().await
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for ServerHandle {
    fn drop(&mut self) {
        self.shutdown.cancel();
    }
}

impl ServerHandle {
    pub async fn stop(mut self) {
        self.shutdown.cancel();
        if let Some(task) = self.task.take() {
            let _ = task.await;
        }
        if let Some(task) = self.onboarding_task.take() {
            let _ = task.await;
        }
    }
}
