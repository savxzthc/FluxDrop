use tauri_plugin_global_shortcut::{Code, Modifiers, Shortcut};

pub const SEND_FLAG: &str = "--send";

pub fn global_shortcut() -> Shortcut {
    Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::KeyD)
}

pub fn extract_send_paths(args: &[String]) -> Vec<String> {
    let mut paths = Vec::new();
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        if arg == SEND_FLAG {
            if let Some(path) = iter.next() {
                push_path(&mut paths, path);
            }
        } else if let Some(rest) = arg.strip_prefix("--send=") {
            push_path(&mut paths, rest);
        }
    }
    paths
}

fn push_path(paths: &mut Vec<String>, candidate: &str) {
    let trimmed = candidate.trim().trim_matches('"');
    if !trimmed.is_empty() {
        paths.push(trimmed.to_string());
    }
}

pub fn apply_registration(enabled: bool) -> Result<(), String> {
    if enabled {
        let exe = std::env::current_exe()
            .map_err(|err| format!("FluxDrop could not locate its own executable: {err}"))?;
        platform::register(&exe)
    } else {
        platform::unregister()
    }
}

pub fn is_registered() -> bool {
    platform::is_registered()
}

#[cfg(windows)]
mod platform {
    use std::path::Path;
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;

    const FILE_MENU_KEY: &str = r"Software\Classes\*\shell\FluxDrop";
    const FILE_COMMAND_KEY: &str = r"Software\Classes\*\shell\FluxDrop\command";
    const DIR_MENU_KEY: &str = r"Software\Classes\Directory\shell\FluxDrop";
    const DIR_COMMAND_KEY: &str = r"Software\Classes\Directory\shell\FluxDrop\command";
    const MENU_LABEL: &str = "Send with FluxDrop";

    pub fn register(exe: &Path) -> Result<(), String> {
        let exe_display = exe.to_string_lossy().into_owned();
        let command = format!("\"{exe_display}\" --send \"%1\"");
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        for (menu_path, command_path) in [
            (FILE_MENU_KEY, FILE_COMMAND_KEY),
            (DIR_MENU_KEY, DIR_COMMAND_KEY),
        ] {
            let (menu_key, _) = hkcu
                .create_subkey(menu_path)
                .map_err(|err| format!("FluxDrop could not open its context-menu key: {err}"))?;
            menu_key
                .set_value("", MENU_LABEL)
                .map_err(|err| format!("FluxDrop could not write the context-menu label: {err}"))?;
            menu_key
                .set_value("Icon", &exe_display)
                .map_err(|err| format!("FluxDrop could not write the context-menu icon: {err}"))?;
            let (command_key, _) = hkcu
                .create_subkey(command_path)
                .map_err(|err| format!("FluxDrop could not open its command key: {err}"))?;
            command_key
                .set_value("", &command)
                .map_err(|err| format!("FluxDrop could not write the launch command: {err}"))?;
        }
        Ok(())
    }

    pub fn unregister() -> Result<(), String> {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        for key in [FILE_MENU_KEY, DIR_MENU_KEY] {
            match hkcu.delete_subkey_all(key) {
                Ok(()) => {}
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                Err(err) => {
                    return Err(format!(
                        "FluxDrop could not remove its context-menu entry: {err}"
                    ))
                }
            }
        }
        Ok(())
    }

    pub fn is_registered() -> bool {
        RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey(FILE_COMMAND_KEY)
            .is_ok()
    }
}

#[cfg(not(windows))]
mod platform {
    use std::path::Path;

    pub fn register(_exe: &Path) -> Result<(), String> {
        Ok(())
    }

    pub fn unregister() -> Result<(), String> {
        Ok(())
    }

    pub fn is_registered() -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_single_send_path() {
        let args = vec![
            "fluxdrop.exe".to_string(),
            SEND_FLAG.to_string(),
            r"C:\Users\Me\report.pdf".to_string(),
        ];
        assert_eq!(extract_send_paths(&args), vec![r"C:\Users\Me\report.pdf"]);
    }

    #[test]
    fn extracts_multiple_send_paths() {
        let args = vec![
            "fluxdrop.exe".to_string(),
            SEND_FLAG.to_string(),
            r"C:\one.txt".to_string(),
            SEND_FLAG.to_string(),
            r"C:\two.txt".to_string(),
        ];
        assert_eq!(
            extract_send_paths(&args),
            vec![r"C:\one.txt", r"C:\two.txt"]
        );
    }

    #[test]
    fn supports_equals_form_and_trims_quotes() {
        let args = vec![
            "fluxdrop.exe".to_string(),
            "--send=\"C:\\spaced path\\file.zip\"".to_string(),
        ];
        assert_eq!(extract_send_paths(&args), vec![r"C:\spaced path\file.zip"]);
    }

    #[test]
    fn ignores_unrelated_args() {
        let args = vec!["fluxdrop.exe".to_string(), "--debug".to_string()];
        assert!(extract_send_paths(&args).is_empty());
    }
}
