use std::collections::HashMap;
use std::path::Path;

const BLOCKED_HEADERS: &[&str] = &[
    "host",
    "content-length",
    "transfer-encoding",
    "connection",
    "keep-alive",
    "upgrade",
    "proxy-authenticate",
    "proxy-authorization",
    "te",
    "trailer",
    "authorization",
    "cookie",
    "set-cookie",
];

pub fn sanitize_filename(name: &str) -> String {
    let file_name = Path::new(name)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("download.bin");
    let sanitized: String = file_name
        .chars()
        .map(|ch| match ch {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            _ => ch,
        })
        .collect();
    if sanitized.trim().is_empty() {
        "download.bin".to_string()
    } else {
        sanitized
    }
}

pub fn filename_from_url(url: &str) -> String {
    let trimmed = url
        .split('?')
        .next()
        .unwrap_or(url)
        .split('#')
        .next()
        .unwrap_or(url)
        .trim_end_matches('/');
    let path_part = match trimmed.split_once("://") {
        Some((_, remainder)) => match remainder.split_once('/') {
            Some((_, path)) => path,
            None => return "download.bin".to_string(),
        },
        None => trimmed,
    };
    if path_part.is_empty() {
        return "download.bin".to_string();
    }
    let candidate = path_part.rsplit('/').next().unwrap_or("download.bin");
    if candidate.is_empty() {
        return "download.bin".to_string();
    }
    sanitize_filename(candidate)
}

pub fn browser_headers(
    page_url: Option<&str>,
    custom_headers: HashMap<String, String>,
) -> Vec<(String, String)> {
    let mut headers: Vec<(String, String)> = custom_headers
        .into_iter()
        .filter(|(name, _)| {
            let lower = name.to_ascii_lowercase();
            !BLOCKED_HEADERS.contains(&lower.as_str())
        })
        .collect();

    if let Some(page_url) = page_url {
        if !headers
            .iter()
            .any(|(name, _)| name.eq_ignore_ascii_case("Referer"))
        {
            headers.push(("Referer".to_string(), page_url.to_string()));
        }
    }

    headers
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocked_headers_are_stripped() {
        let mut raw = HashMap::new();
        raw.insert("Host".to_string(), "evil.com".to_string());
        raw.insert("Content-Length".to_string(), "9999".to_string());
        raw.insert("Connection".to_string(), "keep-alive".to_string());
        raw.insert("Authorization".to_string(), "Bearer tok".to_string());
        raw.insert("Cookie".to_string(), "SID=secret".to_string());
        raw.insert("Set-Cookie".to_string(), "SID=secret".to_string());
        raw.insert("Transfer-Encoding".to_string(), "chunked".to_string());
        raw.insert("X-Custom".to_string(), "ok".to_string());

        let result = browser_headers(None, raw);
        let names: Vec<String> = result
            .iter()
            .map(|(key, _)| key.to_ascii_lowercase())
            .collect();
        for blocked in [
            "host",
            "content-length",
            "connection",
            "authorization",
            "cookie",
            "set-cookie",
            "transfer-encoding",
        ] {
            assert!(!names.contains(&blocked.to_string()));
        }
        assert!(names.contains(&"x-custom".to_string()));
    }

    #[test]
    fn referer_is_injected_only_when_absent() {
        let result = browser_headers(Some("https://example.com/page"), HashMap::new());
        assert_eq!(result[0].1, "https://example.com/page");

        let mut raw = HashMap::new();
        raw.insert("Referer".to_string(), "https://custom.com/".to_string());
        let result = browser_headers(Some("https://page.com/"), raw);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].1, "https://custom.com/");
    }

    #[test]
    fn filenames_are_sanitized() {
        for (input, expected) in [
            ("../etc/passwd", "passwd"),
            ("/etc/passwd", "passwd"),
            ("file:name?.bin", "file_name_.bin"),
            ("", "download.bin"),
            ("   ", "download.bin"),
        ] {
            assert_eq!(sanitize_filename(input), expected);
        }
    }

    #[test]
    fn filenames_are_derived_without_query_or_fragment() {
        for (url, expected) in [
            ("https://example.com/file.zip?token=abc", "file.zip"),
            ("https://example.com/file.zip#section", "file.zip"),
            ("https://example.com/file.zip?token=abc#anchor", "file.zip"),
            ("https://example.com/", "download.bin"),
        ] {
            assert_eq!(filename_from_url(url), expected);
        }
    }
}
