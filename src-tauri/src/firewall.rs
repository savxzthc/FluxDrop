use serde::{Deserialize, Serialize};

pub const FIREWALL_RULE_NAME: &str = "FluxDrop (Private LAN)";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FirewallState {
    Healthy,
    MissingRule,
    StaleExecutablePath,
    WrongProfile,
    PublicNetwork,
    UnavailableService,
    Unsupported,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FirewallDiagnostic {
    pub state: FirewallState,
    pub message: String,
    pub rule_name: String,
    pub executable_path: Option<String>,
    pub configured_path: Option<String>,
    pub current_profiles: Vec<String>,
    pub repair_available: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RuleFacts {
    configured_path: String,
    private_only: bool,
    enabled: bool,
    inbound_tcp_allow: bool,
}

fn classify(current_path: &str, public_network: bool, rule: Option<&RuleFacts>) -> FirewallState {
    if public_network {
        return FirewallState::PublicNetwork;
    }
    let Some(rule) = rule else {
        return FirewallState::MissingRule;
    };
    if !same_windows_path(current_path, &rule.configured_path) {
        return FirewallState::StaleExecutablePath;
    }
    if !rule.private_only || !rule.enabled || !rule.inbound_tcp_allow {
        return FirewallState::WrongProfile;
    }
    FirewallState::Healthy
}

fn same_windows_path(left: &str, right: &str) -> bool {
    left.trim_matches('"')
        .replace('/', "\\")
        .eq_ignore_ascii_case(&right.trim_matches('"').replace('/', "\\"))
}

fn diagnostic_message(state: FirewallState) -> &'static str {
    match state {
        FirewallState::Healthy => "The private-profile inbound TCP firewall rule matches this FluxDrop executable.",
        FirewallState::MissingRule => "Windows Firewall has no FluxDrop private-network rule.",
        FirewallState::StaleExecutablePath => "The FluxDrop firewall rule points to a different executable path.",
        FirewallState::WrongProfile => "The FluxDrop firewall rule is disabled, malformed, or not restricted to the private profile.",
        FirewallState::PublicNetwork => "Windows currently reports an active public network. FluxDrop will not enable public-profile access.",
        FirewallState::UnavailableService => "Windows Firewall policy is unavailable. Check that the Windows Defender Firewall service is running.",
        FirewallState::Unsupported => "Firewall diagnosis is available only on Windows.",
    }
}

#[cfg(windows)]
pub async fn diagnose() -> FirewallDiagnostic {
    tokio::task::spawn_blocking(diagnose_windows)
        .await
        .unwrap_or_else(|_| unavailable_diagnostic())
}

#[cfg(not(windows))]
pub async fn diagnose() -> FirewallDiagnostic {
    FirewallDiagnostic {
        state: FirewallState::Unsupported,
        message: diagnostic_message(FirewallState::Unsupported).to_string(),
        rule_name: FIREWALL_RULE_NAME.to_string(),
        executable_path: std::env::current_exe()
            .ok()
            .map(|path| path.display().to_string()),
        configured_path: None,
        current_profiles: Vec::new(),
        repair_available: false,
    }
}

#[cfg(windows)]
fn unavailable_diagnostic() -> FirewallDiagnostic {
    FirewallDiagnostic {
        state: FirewallState::UnavailableService,
        message: diagnostic_message(FirewallState::UnavailableService).to_string(),
        rule_name: FIREWALL_RULE_NAME.to_string(),
        executable_path: std::env::current_exe()
            .ok()
            .map(|path| path.display().to_string()),
        configured_path: None,
        current_profiles: Vec::new(),
        repair_available: false,
    }
}

#[cfg(windows)]
fn diagnose_windows() -> FirewallDiagnostic {
    use windows::core::BSTR;
    use windows::Win32::NetworkManagement::WindowsFirewall::{
        INetFwPolicy2, NetFwPolicy2, NET_FW_ACTION_ALLOW, NET_FW_IP_PROTOCOL_TCP,
        NET_FW_PROFILE2_PRIVATE, NET_FW_PROFILE2_PUBLIC, NET_FW_RULE_DIR_IN,
    };
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED,
    };

    let executable_path = match std::env::current_exe() {
        Ok(path) => path.display().to_string(),
        Err(_) => return unavailable_diagnostic(),
    };
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let policy: INetFwPolicy2 =
            match CoCreateInstance(&NetFwPolicy2, None, CLSCTX_INPROC_SERVER) {
                Ok(policy) => policy,
                Err(_) => return unavailable_diagnostic(),
            };
        let profiles = match policy.CurrentProfileTypes() {
            Ok(profiles) => profiles,
            Err(_) => return unavailable_diagnostic(),
        };
        let profile_names = profile_names(profiles);
        let public_network = profiles & NET_FW_PROFILE2_PUBLIC.0 != 0;
        let rules = match policy.Rules() {
            Ok(rules) => rules,
            Err(_) => return unavailable_diagnostic(),
        };
        let rule = rules.Item(&BSTR::from(FIREWALL_RULE_NAME)).ok();
        let facts = rule.as_ref().map(|rule| RuleFacts {
            configured_path: rule
                .ApplicationName()
                .map(|value| value.to_string())
                .unwrap_or_default(),
            private_only: rule
                .Profiles()
                .is_ok_and(|value| value == NET_FW_PROFILE2_PRIVATE.0),
            enabled: rule.Enabled().is_ok_and(|value| value.as_bool()),
            inbound_tcp_allow: rule
                .Protocol()
                .is_ok_and(|value| value == NET_FW_IP_PROTOCOL_TCP.0)
                && rule
                    .Direction()
                    .is_ok_and(|value| value == NET_FW_RULE_DIR_IN)
                && rule
                    .Action()
                    .is_ok_and(|value| value == NET_FW_ACTION_ALLOW),
        });
        let state = classify(&executable_path, public_network, facts.as_ref());
        FirewallDiagnostic {
            state,
            message: diagnostic_message(state).to_string(),
            rule_name: FIREWALL_RULE_NAME.to_string(),
            executable_path: Some(executable_path),
            configured_path: facts.map(|facts| facts.configured_path),
            current_profiles: profile_names,
            repair_available: matches!(
                state,
                FirewallState::MissingRule
                    | FirewallState::StaleExecutablePath
                    | FirewallState::WrongProfile
            ),
        }
    }
}

#[cfg(windows)]
fn profile_names(profiles: i32) -> Vec<String> {
    use windows::Win32::NetworkManagement::WindowsFirewall::{
        NET_FW_PROFILE2_DOMAIN, NET_FW_PROFILE2_PRIVATE, NET_FW_PROFILE2_PUBLIC,
    };
    let mut names = Vec::new();
    if profiles & NET_FW_PROFILE2_DOMAIN.0 != 0 {
        names.push("domain".to_string());
    }
    if profiles & NET_FW_PROFILE2_PRIVATE.0 != 0 {
        names.push("private".to_string());
    }
    if profiles & NET_FW_PROFILE2_PUBLIC.0 != 0 {
        names.push("public".to_string());
    }
    names
}

#[cfg(windows)]
pub fn launch_elevated_repair() -> Result<(), String> {
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    let executable = std::env::current_exe()
        .map_err(|err| format!("FluxDrop could not locate its executable: {err}"))?;
    let verb = wide("runas");
    let file = wide(&executable.display().to_string());
    let args = wide(repair_helper_argument());
    let result = unsafe {
        ShellExecuteW(
            Some(HWND::default()),
            PCWSTR(verb.as_ptr()),
            PCWSTR(file.as_ptr()),
            PCWSTR(args.as_ptr()),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };
    if result.0 as isize <= 32 {
        return Err(
            "Windows declined or could not launch the elevated firewall repair.".to_string(),
        );
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn launch_elevated_repair() -> Result<(), String> {
    Err("Firewall repair is available only on Windows.".to_string())
}

#[cfg(windows)]
fn wide(value: &str) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    std::ffi::OsStr::new(value)
        .encode_wide()
        .chain(Some(0))
        .collect()
}

fn repair_helper_argument() -> &'static str {
    "--repair-firewall"
}

#[cfg(windows)]
pub fn repair_current_executable() -> Result<(), String> {
    use windows::core::BSTR;
    use windows::Win32::NetworkManagement::WindowsFirewall::{
        INetFwPolicy2, INetFwRule, NetFwPolicy2, NetFwRule, NET_FW_ACTION_ALLOW,
        NET_FW_IP_PROTOCOL_TCP, NET_FW_PROFILE2_PRIVATE, NET_FW_RULE_DIR_IN,
    };
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED,
    };
    let executable = std::env::current_exe()
        .map_err(|err| format!("FluxDrop could not locate its executable: {err}"))?;
    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED)
            .ok()
            .map_err(|err| err.to_string())?;
        let policy: INetFwPolicy2 = CoCreateInstance(&NetFwPolicy2, None, CLSCTX_INPROC_SERVER)
            .map_err(|err| format!("Windows Firewall policy is unavailable: {err}"))?;
        let rules = policy
            .Rules()
            .map_err(|err| format!("Windows Firewall rules are unavailable: {err}"))?;
        let _ = rules.Remove(&BSTR::from(FIREWALL_RULE_NAME));
        let rule: INetFwRule = CoCreateInstance(&NetFwRule, None, CLSCTX_INPROC_SERVER)
            .map_err(|err| format!("FluxDrop could not create a firewall rule: {err}"))?;
        rule.SetName(&BSTR::from(FIREWALL_RULE_NAME))
            .map_err(|err| err.to_string())?;
        rule.SetDescription(&BSTR::from(
            "Allows FluxDrop transfers from the local subnet on private Windows networks only.",
        ))
        .map_err(|err| err.to_string())?;
        rule.SetApplicationName(&BSTR::from(executable.display().to_string()))
            .map_err(|err| err.to_string())?;
        rule.SetProtocol(NET_FW_IP_PROTOCOL_TCP.0)
            .map_err(|err| err.to_string())?;
        rule.SetDirection(NET_FW_RULE_DIR_IN)
            .map_err(|err| err.to_string())?;
        rule.SetProfiles(NET_FW_PROFILE2_PRIVATE.0)
            .map_err(|err| err.to_string())?;
        rule.SetRemoteAddresses(&BSTR::from("LocalSubnet"))
            .map_err(|err| err.to_string())?;
        rule.SetEdgeTraversal(false.into())
            .map_err(|err| err.to_string())?;
        rule.SetAction(NET_FW_ACTION_ALLOW)
            .map_err(|err| err.to_string())?;
        rule.SetEnabled(true.into())
            .map_err(|err| err.to_string())?;
        rules
            .Add(&rule)
            .map_err(|err| format!("FluxDrop could not install the firewall rule: {err}"))?;
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn repair_current_executable() -> Result<(), String> {
    Err("Firewall repair is available only on Windows.".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_missing_stale_wrong_and_healthy_rules() {
        assert_eq!(
            classify("C:\\FluxDrop.exe", false, None),
            FirewallState::MissingRule
        );
        let mut facts = RuleFacts {
            configured_path: "D:\\FluxDrop.exe".into(),
            private_only: true,
            enabled: true,
            inbound_tcp_allow: true,
        };
        assert_eq!(
            classify("C:\\FluxDrop.exe", false, Some(&facts)),
            FirewallState::StaleExecutablePath
        );
        facts.configured_path = "c:/fluxdrop.exe".into();
        facts.private_only = false;
        assert_eq!(
            classify("C:\\FluxDrop.exe", false, Some(&facts)),
            FirewallState::WrongProfile
        );
        facts.private_only = true;
        assert_eq!(
            classify("C:\\FluxDrop.exe", false, Some(&facts)),
            FirewallState::Healthy
        );
        assert_eq!(
            classify("C:\\FluxDrop.exe", true, Some(&facts)),
            FirewallState::PublicNetwork
        );
    }

    #[test]
    fn elevated_helper_uses_fixed_argument() {
        assert_eq!(repair_helper_argument(), "--repair-firewall");
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn windows_diagnosis_always_returns_structured_state() {
        let diagnostic = diagnose().await;
        assert_eq!(diagnostic.rule_name, FIREWALL_RULE_NAME);
        assert!(diagnostic.executable_path.is_some());
        assert!(!diagnostic.message.is_empty());
    }
}
