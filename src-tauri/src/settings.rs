use serde::{Deserialize, Serialize};
use std::fs;
use std::net::IpAddr;
use std::path::{Path, PathBuf};

pub const SETTINGS_FILE_NAME: &str = "settings.json";
pub const ALLOWED_EXPIRATION_MINUTES: [u32; 4] = [5, 10, 30, 60];
pub const MAX_UPLOAD_BYTES: u64 = 16 * 1024 * 1024 * 1024;

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ThemePreference {
    #[default]
    System,
    Light,
    Dark,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct AppSettings {
    pub expiration_minutes: u32,
    pub single_use: bool,
    pub approval_required: bool,
    pub preferred_lan_ip: Option<String>,
    pub max_upload_bytes: u64,
    pub theme: ThemePreference,
    pub shell_integration: bool,
    pub global_hotkey: bool,
    pub remember_transfer_locations: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            expiration_minutes: 10,
            single_use: true,
            approval_required: true,
            preferred_lan_ip: None,
            max_upload_bytes: 2 * 1024 * 1024 * 1024,
            theme: ThemePreference::System,
            shell_integration: false,
            global_hotkey: false,
            remember_transfer_locations: true,
        }
    }
}

impl AppSettings {
    pub fn validate(&self) -> Result<(), String> {
        if !ALLOWED_EXPIRATION_MINUTES.contains(&self.expiration_minutes) {
            return Err("Link expiration must be 5, 10, 30, or 60 minutes.".to_string());
        }
        if let Some(ip) = self.preferred_lan_ip.as_deref() {
            let parsed = ip.parse::<IpAddr>().map_err(|_| {
                "Preferred LAN adapter must contain a valid IP address.".to_string()
            })?;
            if !crate::network::is_private_lan_ip(&parsed) {
                return Err("Preferred LAN adapter must use a private IPv4 address.".to_string());
            }
        }
        validate_max_upload_bytes(self.max_upload_bytes)
    }
}

pub fn validate_max_upload_bytes(max_upload_bytes: u64) -> Result<(), String> {
    if max_upload_bytes == 0 {
        return Err("Maximum upload size must be greater than zero.".to_string());
    }
    if max_upload_bytes > MAX_UPLOAD_BYTES {
        return Err("Maximum upload size cannot exceed 16 GB.".to_string());
    }
    Ok(())
}

pub fn load(path: &Path) -> Result<AppSettings, String> {
    if !path.exists() {
        return Ok(AppSettings::default());
    }
    let contents = fs::read_to_string(path)
        .map_err(|err| format!("FluxDrop could not read settings: {err}"))?;
    let settings = serde_json::from_str::<AppSettings>(&contents)
        .map_err(|err| format!("FluxDrop settings are invalid JSON: {err}"))?;
    settings.validate()?;
    Ok(settings)
}

pub fn save(path: &Path, settings: &AppSettings) -> Result<(), String> {
    settings.validate()?;
    let parent = path
        .parent()
        .ok_or_else(|| "FluxDrop settings path has no parent directory.".to_string())?;
    fs::create_dir_all(parent)
        .map_err(|err| format!("FluxDrop could not create its settings directory: {err}"))?;
    let contents = serde_json::to_vec_pretty(settings)
        .map_err(|err| format!("FluxDrop could not serialize settings: {err}"))?;
    let temporary = temporary_path(path);
    fs::write(&temporary, contents)
        .map_err(|err| format!("FluxDrop could not write settings: {err}"))?;
    crate::file_utils::replace_file_atomic(&temporary, path)
        .map_err(|err| format!("FluxDrop could not finish saving settings: {err}"))
}

fn temporary_path(path: &Path) -> PathBuf {
    let mut temporary = path.as_os_str().to_os_string();
    temporary.push(".tmp");
    PathBuf::from(temporary)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_keep_secure_share_behavior() {
        let settings = AppSettings::default();
        assert_eq!(settings.expiration_minutes, 10);
        assert!(settings.single_use);
        assert!(settings.approval_required);
        assert_eq!(settings.max_upload_bytes, 2 * 1024 * 1024 * 1024);
        assert_eq!(settings.theme, ThemePreference::System);
        assert!(settings.remember_transfer_locations);
    }

    #[test]
    fn rejects_unsupported_expiration() {
        let settings = AppSettings {
            expiration_minutes: 15,
            ..AppSettings::default()
        };
        assert!(settings.validate().is_err());
    }

    #[test]
    fn rejects_invalid_upload_limits() {
        assert!(validate_max_upload_bytes(0).is_err());
        assert!(validate_max_upload_bytes(MAX_UPLOAD_BYTES + 1).is_err());
        assert!(validate_max_upload_bytes(MAX_UPLOAD_BYTES).is_ok());
    }

    #[test]
    fn settings_round_trip_to_json_file() {
        let directory = tempfile::tempdir().expect("tempdir");
        let path = directory.path().join(SETTINGS_FILE_NAME);
        let settings = AppSettings {
            expiration_minutes: 30,
            single_use: false,
            preferred_lan_ip: Some("192.168.1.50".to_string()),
            theme: ThemePreference::Dark,
            ..AppSettings::default()
        };
        save(&path, &settings).expect("save");
        assert_eq!(load(&path).expect("load"), settings);
    }

    #[test]
    fn legacy_settings_without_theme_follow_system() {
        let directory = tempfile::tempdir().expect("tempdir");
        let path = directory.path().join(SETTINGS_FILE_NAME);
        fs::write(
            &path,
            r#"{
                "expiration_minutes": 10,
                "single_use": true,
                "approval_required": true,
                "preferred_lan_ip": null,
                "max_upload_bytes": 2147483648
            }"#,
        )
        .expect("write legacy settings");

        assert_eq!(
            load(&path).expect("load legacy settings").theme,
            ThemePreference::System
        );
        assert!(
            load(&path)
                .expect("load legacy settings")
                .remember_transfer_locations
        );
    }
}
