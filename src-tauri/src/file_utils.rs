use percent_encoding::{utf8_percent_encode, AsciiSet, CONTROLS};
use std::path::Path;

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

    let sanitized = sanitized.trim().trim_matches('.').to_string();
    if sanitized.is_empty() {
        "download".to_string()
    } else {
        sanitized
    }
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
