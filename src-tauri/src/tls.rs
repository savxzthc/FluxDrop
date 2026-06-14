use rcgen::{CertificateParams, DnType, KeyPair, SanType};
use std::fs;
use std::net::IpAddr;
use std::path::{Path, PathBuf};

const CERT_FILE: &str = "local-cert.pem";
const KEY_FILE: &str = "local-key.pem";
const IP_FILE: &str = "local-cert-ip.txt";

#[derive(Debug, Clone)]
pub struct CertificateFiles {
    pub cert_path: PathBuf,
    pub key_path: PathBuf,
}

pub fn ensure_certificate(config_dir: &Path, ip: IpAddr) -> Result<CertificateFiles, String> {
    let cert_path = config_dir.join(CERT_FILE);
    let key_path = config_dir.join(KEY_FILE);
    let ip_path = config_dir.join(IP_FILE);
    let expected_ip = ip.to_string();
    let matches_ip = fs::read_to_string(&ip_path)
        .ok()
        .is_some_and(|stored| stored.trim() == expected_ip);

    if matches_ip && cert_path.is_file() && key_path.is_file() {
        return Ok(CertificateFiles {
            cert_path,
            key_path,
        });
    }

    fs::create_dir_all(config_dir)
        .map_err(|err| format!("FluxDrop could not create its TLS directory: {err}"))?;
    let mut params = CertificateParams::default();
    params.subject_alt_names = vec![SanType::IpAddress(ip)];
    params
        .distinguished_name
        .push(DnType::CommonName, format!("FluxDrop local server {ip}"));
    let signing_key = KeyPair::generate()
        .map_err(|err| format!("FluxDrop could not generate a TLS key: {err}"))?;
    let certificate = params
        .self_signed(&signing_key)
        .map_err(|err| format!("FluxDrop could not generate a TLS certificate: {err}"))?;

    replace_file(&cert_path, certificate.pem().as_bytes())?;
    replace_file(&key_path, signing_key.serialize_pem().as_bytes())?;
    harden_key_permissions(&key_path)?;
    replace_file(&ip_path, expected_ip.as_bytes())?;

    Ok(CertificateFiles {
        cert_path,
        key_path,
    })
}

fn replace_file(path: &Path, contents: &[u8]) -> Result<(), String> {
    let mut temporary = path.as_os_str().to_os_string();
    temporary.push(".tmp");
    let temporary = PathBuf::from(temporary);
    fs::write(&temporary, contents)
        .map_err(|err| format!("FluxDrop could not write TLS material: {err}"))?;
    if path.exists() {
        fs::remove_file(path)
            .map_err(|err| format!("FluxDrop could not replace TLS material: {err}"))?;
    }
    fs::rename(&temporary, path)
        .map_err(|err| format!("FluxDrop could not finish writing TLS material: {err}"))
}

#[cfg(unix)]
fn harden_key_permissions(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        .map_err(|err| format!("FluxDrop could not secure the TLS key permissions: {err}"))
}

#[cfg(not(unix))]
fn harden_key_permissions(_path: &Path) -> Result<(), String> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn certificate_is_reused_for_same_ip_and_rotated_for_new_ip() {
        let directory = tempfile::tempdir().expect("tempdir");
        let first_ip: IpAddr = "192.168.1.10".parse().expect("first ip");
        let second_ip: IpAddr = "192.168.1.11".parse().expect("second ip");
        let files = ensure_certificate(directory.path(), first_ip).expect("first cert");
        let first_cert = fs::read(&files.cert_path).expect("read first");
        let reused = ensure_certificate(directory.path(), first_ip).expect("reuse cert");
        assert_eq!(first_cert, fs::read(reused.cert_path).expect("read reused"));

        let rotated = ensure_certificate(directory.path(), second_ip).expect("rotated cert");
        assert_ne!(
            first_cert,
            fs::read(rotated.cert_path).expect("read rotated")
        );
        assert_eq!(
            fs::read_to_string(directory.path().join(IP_FILE))
                .expect("ip file")
                .trim(),
            second_ip.to_string()
        );
    }
}
