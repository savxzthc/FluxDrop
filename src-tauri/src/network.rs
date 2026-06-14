use serde::{Deserialize, Serialize};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener};

pub const PREFERRED_PORT: u16 = 47995;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NetworkAddress {
    pub interface_name: String,
    pub ip: String,
    pub preferred: bool,
    pub reason: String,
}

pub fn is_private_lan_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let octets = v4.octets();
            v4.is_private() && !v4.is_loopback() && !v4.is_link_local() && octets != [0, 0, 0, 0]
        }
        IpAddr::V6(_) => false,
    }
}

pub fn list_network_addresses() -> Vec<NetworkAddress> {
    let mut addresses = local_ip_address::list_afinet_netifas()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(interface_name, ip)| {
            if !is_private_lan_ip(&ip) {
                return None;
            }
            let lowered = interface_name.to_ascii_lowercase();
            let virtualish = [
                "virtual",
                "vmware",
                "vbox",
                "docker",
                "hyper-v",
                "loopback",
                "tailscale",
                "vpn",
            ]
            .iter()
            .any(|needle| lowered.contains(needle));
            Some(NetworkAddress {
                interface_name,
                ip: ip.to_string(),
                preferred: !virtualish,
                reason: if virtualish {
                    "Private address on an adapter that looks virtual or VPN-like.".to_string()
                } else {
                    "Private LAN address suitable for phone access.".to_string()
                },
            })
        })
        .collect::<Vec<_>>();

    addresses.sort_by(|a, b| {
        b.preferred
            .cmp(&a.preferred)
            .then_with(|| a.interface_name.cmp(&b.interface_name))
    });
    addresses
}

pub fn preferred_ip_address(addresses: &[NetworkAddress]) -> IpAddr {
    addresses
        .iter()
        .find(|address| address.preferred)
        .or_else(|| addresses.first())
        .and_then(|address| address.ip.parse::<IpAddr>().ok())
        .unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST))
}

pub fn configured_ip_address(addresses: &[NetworkAddress], configured_ip: Option<&str>) -> IpAddr {
    configured_ip
        .and_then(|configured| {
            addresses
                .iter()
                .find(|address| address.ip == configured)
                .and_then(|address| address.ip.parse::<IpAddr>().ok())
        })
        .unwrap_or_else(|| preferred_ip_address(addresses))
}

pub fn select_available_port(ip: IpAddr, preferred_port: u16) -> std::io::Result<u16> {
    for port in preferred_port..preferred_port.saturating_add(50) {
        if TcpListener::bind(SocketAddr::new(ip, port)).is_ok() {
            return Ok(port);
        }
    }
    let listener = TcpListener::bind(SocketAddr::new(ip, 0))?;
    Ok(listener.local_addr()?.port())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_private_ip_detection() {
        assert!(is_private_lan_ip(&IpAddr::V4(Ipv4Addr::new(
            192, 168, 1, 1
        ))));
        assert!(is_private_lan_ip(&IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1))));
        assert!(is_private_lan_ip(&IpAddr::V4(Ipv4Addr::new(172, 16, 0, 1))));
    }

    #[test]
    fn test_loopback_excluded() {
        assert!(!is_private_lan_ip(&IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))));
    }

    #[test]
    fn test_port_selection_fallback() {
        let ip = IpAddr::V4(Ipv4Addr::LOCALHOST);
        let listener = TcpListener::bind(SocketAddr::new(ip, 0)).expect("bind occupied test port");
        let occupied = listener.local_addr().expect("local addr").port();
        let selected = select_available_port(ip, occupied).expect("select fallback port");
        assert_ne!(selected, occupied);
    }

    #[test]
    fn test_configured_private_address_overrides_heuristic() {
        let addresses = vec![
            NetworkAddress {
                interface_name: "Ethernet".into(),
                ip: "192.168.1.5".into(),
                preferred: true,
                reason: "preferred".into(),
            },
            NetworkAddress {
                interface_name: "VPN".into(),
                ip: "10.0.0.5".into(),
                preferred: false,
                reason: "virtual".into(),
            },
        ];
        assert_eq!(
            configured_ip_address(&addresses, Some("10.0.0.5")),
            "10.0.0.5".parse::<IpAddr>().expect("ip")
        );
    }
}
