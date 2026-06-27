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

#[cfg(windows)]
fn harden_key_permissions(path: &Path) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    use std::ptr::{null_mut, NonNull};
    use windows_sys::Win32::Foundation::{CloseHandle, LocalFree, ERROR_SUCCESS, HANDLE};
    use windows_sys::Win32::Security::Authorization::{
        SetEntriesInAclW, SetNamedSecurityInfoW, EXPLICIT_ACCESS_W, NO_MULTIPLE_TRUSTEE,
        SET_ACCESS, SE_FILE_OBJECT, TRUSTEE_IS_SID, TRUSTEE_IS_USER, TRUSTEE_W,
    };
    use windows_sys::Win32::Security::{
        GetTokenInformation, TokenUser, ACL, DACL_SECURITY_INFORMATION,
        PROTECTED_DACL_SECURITY_INFORMATION, PSID, TOKEN_QUERY, TOKEN_USER,
    };
    use windows_sys::Win32::Storage::FileSystem::FILE_ALL_ACCESS;
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    struct TokenHandle(HANDLE);
    impl Drop for TokenHandle {
        fn drop(&mut self) {
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }

    struct LocalAcl(NonNull<ACL>);
    impl Drop for LocalAcl {
        fn drop(&mut self) {
            unsafe {
                let _ = LocalFree(self.0.as_ptr().cast());
            }
        }
    }

    fn windows_error(action: &str, code: u32) -> String {
        format!("FluxDrop could not secure the TLS key permissions while trying to {action}: Windows error {code}")
    }

    let token = unsafe {
        let mut token = null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return Err(windows_error(
                "open the current process token",
                windows_sys::Win32::Foundation::GetLastError(),
            ));
        }
        TokenHandle(token)
    };

    let mut token_info_len = 0;
    unsafe {
        let _ = GetTokenInformation(token.0, TokenUser, null_mut(), 0, &mut token_info_len);
    }
    if token_info_len == 0 {
        return Err(windows_error("measure the current user token", unsafe {
            windows_sys::Win32::Foundation::GetLastError()
        }));
    }

    let mut token_info = vec![0_u8; token_info_len as usize];
    let token_user = unsafe {
        if GetTokenInformation(
            token.0,
            TokenUser,
            token_info.as_mut_ptr().cast(),
            token_info_len,
            &mut token_info_len,
        ) == 0
        {
            return Err(windows_error(
                "read the current user token",
                windows_sys::Win32::Foundation::GetLastError(),
            ));
        }
        &*(token_info.as_ptr().cast::<TOKEN_USER>())
    };
    let user_sid: PSID = token_user.User.Sid;
    let access = EXPLICIT_ACCESS_W {
        grfAccessPermissions: FILE_ALL_ACCESS,
        grfAccessMode: SET_ACCESS,
        grfInheritance: 0,
        Trustee: TRUSTEE_W {
            pMultipleTrustee: null_mut(),
            MultipleTrusteeOperation: NO_MULTIPLE_TRUSTEE,
            TrusteeForm: TRUSTEE_IS_SID,
            TrusteeType: TRUSTEE_IS_USER,
            ptstrName: user_sid.cast(),
        },
    };
    let acl = unsafe {
        let mut acl = null_mut();
        let result = SetEntriesInAclW(1, &access, null_mut(), &mut acl);
        if result != ERROR_SUCCESS {
            return Err(windows_error("build a private key ACL", result));
        }
        LocalAcl(NonNull::new(acl).ok_or_else(|| {
            "FluxDrop could not secure the TLS key permissions: Windows returned a null ACL"
                .to_string()
        })?)
    };

    let mut key_path = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    // Protect the key from broad inherited directory ACLs; only this Windows
    // user gets file access to the generated private key.
    let result = unsafe {
        SetNamedSecurityInfoW(
            key_path.as_mut_ptr(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
            null_mut(),
            null_mut(),
            acl.0.as_ptr(),
            null_mut(),
        )
    };
    if result != ERROR_SUCCESS {
        return Err(windows_error("apply the private key ACL", result));
    }

    Ok(())
}

#[cfg(not(any(unix, windows)))]
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

    #[cfg(windows)]
    #[test]
    fn windows_key_permission_hardening_keeps_key_readable() {
        let directory = tempfile::tempdir().expect("tempdir");
        let key_path = directory.path().join("local-key.pem");
        fs::write(&key_path, b"test key").expect("write key");

        harden_key_permissions(&key_path).expect("harden key");

        assert_eq!(fs::read(&key_path).expect("read key"), b"test key");
    }
}
