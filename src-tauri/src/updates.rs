use crate::share::ShareStatus;
use crate::state::AppState;
use serde::Serialize;
use std::sync::Mutex;
use tauri::{AppHandle, State};
use tauri_plugin_updater::{Update, UpdaterExt};

pub const RELEASE_PAGE: &str = "https://github.com/savxzthc/FluxDrop/releases/latest";

struct DownloadedUpdate {
    update: Update,
    bytes: Vec<u8>,
}

#[derive(Default)]
pub struct PendingUpdateState(Mutex<Option<DownloadedUpdate>>);

#[derive(Debug, Clone, Serialize)]
pub struct UpdateInfo {
    pub available: bool,
    pub version: Option<String>,
    pub current_version: String,
    pub body: Option<String>,
    pub date: Option<String>,
    pub portable: bool,
    pub downloaded: bool,
    pub download_page: String,
}

pub fn build_flavor() -> &'static str {
    option_env!("FLUXDROP_BUILD_FLAVOR").unwrap_or("portable")
}

pub fn is_portable() -> bool {
    portable_flavor(Some(build_flavor()))
}

fn portable_flavor(flavor: Option<&str>) -> bool {
    !flavor.is_some_and(|value| value.eq_ignore_ascii_case("installed"))
}

#[tauri::command]
pub async fn check_for_update(
    app: AppHandle,
    app_state: State<'_, AppState>,
    pending: State<'_, PendingUpdateState>,
) -> Result<UpdateInfo, String> {
    let current_version = app.package_info().version.to_string();
    let update = app
        .updater()
        .map_err(|err| format!("Updater configuration is invalid: {err}"))?
        .check()
        .await
        .map_err(|err| format!("FluxDrop could not check for updates: {err}"))?;
    let Some(update) = update else {
        *pending
            .0
            .lock()
            .map_err(|_| "Updater state is unavailable.")? = None;
        return Ok(UpdateInfo {
            available: false,
            version: None,
            current_version,
            body: None,
            date: None,
            portable: is_portable(),
            downloaded: false,
            download_page: RELEASE_PAGE.to_string(),
        });
    };
    let mut downloaded = false;
    if !is_portable() && app_state.read().await.settings.automatic_updates {
        let bytes = update.download(|_, _| {}, || {}).await.map_err(|err| {
            format!("FluxDrop rejected or could not download the signed update: {err}")
        })?;
        *pending
            .0
            .lock()
            .map_err(|_| "Updater state is unavailable.")? = Some(DownloadedUpdate {
            update: update.clone(),
            bytes,
        });
        downloaded = true;
    }
    Ok(UpdateInfo {
        available: true,
        version: Some(update.version.clone()),
        current_version: update.current_version.clone(),
        body: update.body.clone(),
        date: update.date.map(|date| date.to_string()),
        portable: is_portable(),
        downloaded,
        download_page: RELEASE_PAGE.to_string(),
    })
}

#[tauri::command]
pub async fn install_update(
    app: AppHandle,
    app_state: State<'_, AppState>,
    pending: State<'_, PendingUpdateState>,
) -> Result<(), String> {
    if is_portable() {
        return Err("Portable FluxDrop builds do not install updates automatically. Open the release page to download the new portable build.".to_string());
    }
    if transfer_active(&app_state).await {
        return Err(
            "Finish or cancel the active transfer before installing the update.".to_string(),
        );
    }
    let downloaded = pending
        .0
        .lock()
        .map_err(|_| "Updater state is unavailable.".to_string())?
        .take();
    let downloaded = match downloaded {
        Some(downloaded) => downloaded,
        None => {
            let update = app
                .updater()
                .map_err(|err| format!("Updater configuration is invalid: {err}"))?
                .check()
                .await
                .map_err(|err| format!("FluxDrop could not refresh the update: {err}"))?
                .ok_or_else(|| "No update is currently available.".to_string())?;
            let bytes = update.download(|_, _| {}, || {}).await.map_err(|err| {
                format!("FluxDrop rejected or could not download the signed update: {err}")
            })?;
            DownloadedUpdate { update, bytes }
        }
    };
    if transfer_active(&app_state).await {
        *pending
            .0
            .lock()
            .map_err(|_| "Updater state is unavailable.".to_string())? = Some(downloaded);
        return Err("A transfer started while the update was downloading. Finish or cancel it before installing.".to_string());
    }
    downloaded
        .update
        .install(&downloaded.bytes)
        .map_err(|err| format!("FluxDrop could not install the verified update: {err}"))?;
    app.restart();
}

async fn transfer_active(state: &AppState) -> bool {
    let guard = state.read().await;
    guard
        .current_share
        .as_ref()
        .is_some_and(|share| !terminal(&share.status))
        || guard
            .receive_session
            .as_ref()
            .is_some_and(|receive| !terminal(&receive.status))
}

fn terminal(status: &ShareStatus) -> bool {
    matches!(
        status,
        ShareStatus::Completed
            | ShareStatus::Expired
            | ShareStatus::Cancelled
            | ShareStatus::Denied
            | ShareStatus::Error(_)
    )
}

#[tauri::command]
pub fn open_update_download_page() -> Result<(), String> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows::core::PCWSTR;
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::Shell::ShellExecuteW;
        use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
        let target = std::ffi::OsStr::new(RELEASE_PAGE)
            .encode_wide()
            .chain(Some(0))
            .collect::<Vec<_>>();
        let result = unsafe {
            ShellExecuteW(
                Some(HWND::default()),
                PCWSTR::null(),
                PCWSTR(target.as_ptr()),
                PCWSTR::null(),
                PCWSTR::null(),
                SW_SHOWNORMAL,
            )
        };
        if result.0 as isize <= 32 {
            return Err("Windows could not open the FluxDrop release page.".to_string());
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        Err("Open the FluxDrop releases page in your browser.".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installed_and_portable_flavors_are_distinct() {
        assert!(!portable_flavor(Some("installed")));
        assert!(portable_flavor(Some("portable")));
        assert!(portable_flavor(None));
    }

    #[tokio::test]
    async fn active_transfer_blocks_update_installation_path() {
        let state = AppState::new();
        let share = crate::share::ShareSession::new_with_options(
            std::path::PathBuf::from("file.txt"),
            "file.txt".into(),
            "file.txt".into(),
            10,
            "text/plain".into(),
            10,
            true,
            false,
        );
        state.write().await.current_share = Some(share);
        assert!(transfer_active(&state).await);
        state
            .write()
            .await
            .current_share
            .as_mut()
            .expect("share")
            .cancel();
        assert!(!transfer_active(&state).await);
    }
}
