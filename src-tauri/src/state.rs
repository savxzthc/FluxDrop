use crate::network::NetworkAddress;
use crate::rate_limit::RateLimiter;
use crate::share::ShareSession;
use std::net::SocketAddr;
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
    pub server: Option<ServerHandle>,
    pub rate_limiter: RateLimiter,
    pub detected_addresses: Vec<NetworkAddress>,
    pub last_request_status: Option<String>,
}

pub struct ServerHandle {
    pub address: SocketAddr,
    pub shutdown: CancellationToken,
    pub task: ServerTaskHandle,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(RuntimeState {
                current_share: None,
                server: None,
                rate_limiter: RateLimiter::new(20, 60),
                detected_addresses: Vec::new(),
                last_request_status: None,
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
