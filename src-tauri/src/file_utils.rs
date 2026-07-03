use percent_encoding::{utf8_percent_encode, AsciiSet, CONTROLS};
use std::io;
use std::path::Path;

#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;

#[cfg(windows)]
use windows_sys::Win32::Storage::FileSystem::{
    MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
};

const RFC5987_ATTR_CHAR: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'%')
    .add(b'\'')
    .add(b'(')
    .add(b')')
    .add(b'*')
    .add(b',')
    .add(b'/')
    .add(b':')
    .add(b';')
    .add(b'<')
    .add(b'>')
    .add(b'?')
    .add(b'@')
    .add(b'[')
    .add(b'\\')
    .add(b']')
    .add(b'{')
    .add(b'}');
const MAX_FILENAME_CHARS: usize = 180;

pub fn sanitize_filename(input: &str) -> String {
    let name = Path::new(input)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or(input);

    let name = name
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(name)
        .trim()
        .trim_matches('.');

    let mut sanitized = String::with_capacity(name.len());
    for ch in name.chars() {
        if ch.is_control()
            || matches!(
                ch,
                '"' | '\'' | '<' | '>' | ':' | '|' | '?' | '*' | '\r' | '\n'
            )
        {
            sanitized.push('_');
        } else {
            sanitized.push(ch);
        }
    }

    let sanitized = truncate_filename_component(sanitized.trim().trim_matches('.'));
    if sanitized.is_empty() {
        "download".to_string()
    } else if is_windows_reserved_name(&sanitized) {
        format!("_{sanitized}")
    } else {
        sanitized
    }
}

fn truncate_filename_component(name: &str) -> String {
    if name.chars().count() <= MAX_FILENAME_CHARS {
        return name.to_string();
    }

    let path = Path::new(name);
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty() && value.chars().count() <= 24);
    let extension_chars = extension.map_or(0, |value| value.chars().count() + 1);
    let stem_limit = MAX_FILENAME_CHARS.saturating_sub(extension_chars).max(1);
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or(name);
    let mut shortened = stem.chars().take(stem_limit).collect::<String>();
    if let Some(extension) = extension {
        shortened.push('.');
        shortened.push_str(extension);
    }
    shortened.trim().trim_matches('.').to_string()
}

fn is_windows_reserved_name(name: &str) -> bool {
    let stem = name
        .split('.')
        .next()
        .unwrap_or(name)
        .trim()
        .to_ascii_uppercase();
    matches!(
        stem.as_str(),
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "CLOCK$"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
    )
}

pub fn escape_html(input: &str) -> String {
    let mut escaped = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#x27;"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

pub fn header_safe_filename(input: &str) -> String {
    input
        .chars()
        .filter(|ch| !matches!(ch, '\r' | '\n') && !ch.is_control())
        .collect()
}

pub fn content_disposition_filename(input: &str) -> String {
    let safe = header_safe_filename(input);
    utf8_percent_encode(&safe, RFC5987_ATTR_CHAR).to_string()
}

pub fn replace_file_atomic(from: &Path, to: &Path) -> io::Result<()> {
    replace_file_atomic_impl(from, to)
}

#[cfg(windows)]
fn replace_file_atomic_impl(from: &Path, to: &Path) -> io::Result<()> {
    let from_wide = path_to_wide(from);
    let to_wide = path_to_wide(to);
    let moved = unsafe {
        MoveFileExW(
            from_wide.as_ptr(),
            to_wide.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if moved == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(not(windows))]
fn replace_file_atomic_impl(from: &Path, to: &Path) -> io::Result<()> {
    std::fs::rename(from, to)
}

#[cfg(windows)]
fn path_to_wide(path: &Path) -> Vec<u16> {
    path.as_os_str().encode_wide().chain(Some(0)).collect()
}

pub fn format_file_size(bytes: u64) -> String {
    if bytes == 0 {
        return "0 B".to_string();
    }

    let units = ["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit_index = 0;
    while size >= 1024.0 && unit_index < units.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }

    if unit_index == 0 {
        format!("{bytes} B")
    } else {
        format!("{size:.1} {}", units[unit_index])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_normal_filename() {
        assert_eq!(sanitize_filename("example.pdf"), "example.pdf");
    }

    #[test]
    fn test_sanitize_path_traversal() {
        assert_eq!(sanitize_filename("../../secret.txt"), "secret.txt");
    }

    #[test]
    fn test_sanitize_hidden() {
        assert!(!sanitize_filename(".hidden").starts_with('.'));
    }

    #[test]
    fn test_sanitize_windows_path() {
        assert_eq!(sanitize_filename("C:\\Users\\Name\\file.txt"), "file.txt");
    }

    #[test]
    fn test_sanitize_windows_reserved_device_name() {
        assert_eq!(sanitize_filename("CON.txt"), "_CON.txt");
        assert_eq!(sanitize_filename("lpt1"), "_lpt1");
    }

    #[test]
    fn test_sanitize_truncates_long_names_and_preserves_extension() {
        let name = format!("{}.txt", "a".repeat(260));
        let sanitized = sanitize_filename(&name);
        assert_eq!(sanitized.chars().count(), MAX_FILENAME_CHARS);
        assert!(sanitized.ends_with(".txt"));
    }

    #[test]
    fn test_escape_html_script_tag() {
        assert_eq!(
            escape_html("<script>alert(1)</script>"),
            "&lt;script&gt;alert(1)&lt;/script&gt;"
        );
    }

    #[test]
    fn test_escape_html_double_quote() {
        assert_eq!(escape_html("hello \"world\""), "hello &quot;world&quot;");
    }

    #[test]
    fn test_escape_html_ampersand() {
        assert_eq!(escape_html("a & b"), "a &amp; b");
    }

    #[test]
    fn test_escape_html_single_quote() {
        assert_eq!(escape_html("'"), "&#x27;");
    }

    #[test]
    fn test_header_safe_prevents_crlf() {
        let safe = header_safe_filename("file\r\nContent-Length: 0.txt");
        assert!(!safe.contains('\r'));
        assert!(!safe.contains('\n'));
    }

    #[test]
    fn replace_file_atomic_overwrites_existing_file() {
        let directory = tempfile::tempdir().expect("tempdir");
        let target = directory.path().join("settings.json");
        let temporary = directory.path().join("settings.json.tmp");
        std::fs::write(&target, b"old").expect("write old");
        std::fs::write(&temporary, b"new").expect("write new");

        replace_file_atomic(&temporary, &target).expect("replace file");

        assert_eq!(std::fs::read(&target).expect("read target"), b"new");
        assert!(!temporary.exists());
    }

    #[test]
    fn test_format_file_size_zero() {
        assert_eq!(format_file_size(0), "0 B");
    }

    #[test]
    fn test_format_file_size_kb() {
        assert_eq!(format_file_size(1536), "1.5 KB");
    }

    #[test]
    fn test_format_file_size_mb() {
        assert_eq!(format_file_size(2_415_919), "2.3 MB");
    }

    #[test]
    fn test_format_file_size_gb() {
        assert_eq!(format_file_size(13_958_643_712), "13.0 GB");
    }
}
