//! Small pure text helpers (no external dependencies).

/// True for a single token that looks like a URL (`scheme://rest` or `mailto:x@y`).
pub fn is_url(s: &str) -> bool {
    let s = s.trim();
    if s.is_empty() || s.chars().any(|c| c.is_whitespace()) {
        return false;
    }
    if let Some(rest) = s.strip_prefix("mailto:") {
        return rest.contains('@');
    }
    match s.find("://") {
        Some(i) if i > 0 => {
            let scheme = &s[..i];
            scheme
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '-' || c == '.')
                && s.len() > i + 3
        }
        _ => false,
    }
}

/// Parses `#rgb`, `#rrggbb`, `#rrggbbaa`, `rgb(r,g,b)` and `rgba(r,g,b,a)`.
pub fn parse_color(s: &str) -> Option<(u8, u8, u8, u8)> {
    let s = s.trim();
    if let Some(hex) = s.strip_prefix('#') {
        if hex.is_empty() || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
            return None;
        }
        let v = |i: usize, n: usize| u8::from_str_radix(&hex[i..i + n], 16).ok();
        return match hex.len() {
            3 => Some((v(0, 1)? * 17, v(1, 1)? * 17, v(2, 1)? * 17, 255)),
            6 => Some((v(0, 2)?, v(2, 2)?, v(4, 2)?, 255)),
            8 => Some((v(0, 2)?, v(2, 2)?, v(4, 2)?, v(6, 2)?)),
            _ => None,
        };
    }
    let lower = s.to_ascii_lowercase();
    let inner = lower
        .strip_prefix("rgba(")
        .or_else(|| lower.strip_prefix("rgb("))?
        .strip_suffix(')')?;
    let parts: Vec<&str> = inner.split(',').map(|p| p.trim()).collect();
    if parts.len() != 3 && parts.len() != 4 {
        return None;
    }
    let c = |i: usize| {
        parts[i]
            .parse::<u16>()
            .ok()
            .filter(|v| *v <= 255)
            .map(|v| v as u8)
    };
    let a = if parts.len() == 4 {
        let f: f32 = parts[3].parse().ok()?;
        if !(0.0..=1.0).contains(&f) {
            return None;
        }
        (f * 255.0).round() as u8
    } else {
        255
    };
    Some((c(0)?, c(1)?, c(2)?, a))
}

/// `#RRGGBB` (or `#RRGGBBAA` when not opaque).
pub fn color_hex(r: u8, g: u8, b: u8, a: u8) -> String {
    if a == 255 {
        format!("#{:02X}{:02X}{:02X}", r, g, b)
    } else {
        format!("#{:02X}{:02X}{:02X}{:02X}", r, g, b, a)
    }
}

pub fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    let hex = |c: u8| (c as char).to_digit(16).map(|d| d as u8);
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let (Some(h), Some(l)) = (hex(b[i + 1]), hex(b[i + 2])) {
                out.push(h * 16 + l);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Returns the local paths if every non-comment line is a `file://` URI.
pub fn uri_list_files(list: &str) -> Option<Vec<String>> {
    let mut paths = Vec::new();
    for line in list.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let rest = line.strip_prefix("file://")?;
        let path = if rest.starts_with('/') {
            rest
        } else {
            &rest[rest.find('/')?..]
        };
        paths.push(percent_decode(path));
    }
    if paths.is_empty() {
        None
    } else {
        Some(paths)
    }
}

pub fn file_name(path: &str) -> &str {
    let t = path.trim_end_matches('/');
    t.rsplit('/').next().unwrap_or(t)
}

pub fn human_size(n: u64) -> String {
    const KB: f64 = 1024.0;
    let f = n as f64;
    if f < KB {
        format!("{} B", n)
    } else if f < KB * KB {
        format!("{:.1} KB", f / KB)
    } else if f < KB * KB * KB {
        format!("{:.1} MB", f / (KB * KB))
    } else {
        format!("{:.1} GB", f / (KB * KB * KB))
    }
}

/// Compact relative time such as `now`, `5m`, `3h`, `2d`, `3w`, `4mo`, `1y`.
pub fn relative_time(now_ms: i64, then_ms: i64) -> String {
    let secs = ((now_ms - then_ms) / 1000).max(0);
    let mins = secs / 60;
    let hours = mins / 60;
    let days = hours / 24;
    if secs < 60 {
        "now".to_string()
    } else if mins < 60 {
        format!("{}m", mins)
    } else if hours < 24 {
        format!("{}h", hours)
    } else if days < 14 {
        format!("{}d", days)
    } else if days < 60 {
        format!("{}w", days / 7)
    } else if days < 365 {
        format!("{}mo", days / 30)
    } else {
        format!("{}y", days / 365)
    }
}

/// First `max` characters (no ellipsis).
pub fn take_chars(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

/// First `max` characters with an ellipsis when cut.
pub fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut t: String = s.chars().take(max).collect();
        t.push('…');
        t
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls() {
        assert!(is_url("https://example.com/a?b=1"));
        assert!(is_url("mailto:a@b.c"));
        assert!(!is_url("hello world"));
        assert!(!is_url("http://"));
        assert!(!is_url("example.com"));
    }

    #[test]
    fn colors() {
        assert_eq!(parse_color("#fff"), Some((255, 255, 255, 255)));
        assert_eq!(parse_color("#102030"), Some((16, 32, 48, 255)));
        assert_eq!(parse_color("#10203040"), Some((16, 32, 48, 64)));
        assert_eq!(parse_color("rgb(1, 2, 3)"), Some((1, 2, 3, 255)));
        assert_eq!(parse_color("rgba(1,2,3,0.5)"), Some((1, 2, 3, 128)));
        assert_eq!(parse_color("#12"), None);
        assert_eq!(parse_color("rgb(300,0,0)"), None);
        assert_eq!(parse_color("hello"), None);
        assert_eq!(color_hex(1, 2, 255, 255), "#0102FF");
    }

    #[test]
    fn uri_lists() {
        let l = "# c\r\nfile:///home/a/My%20Doc.txt\r\nfile://localhost/tmp/x\r\n";
        assert_eq!(
            uri_list_files(l),
            Some(vec!["/home/a/My Doc.txt".to_string(), "/tmp/x".to_string()])
        );
        assert_eq!(uri_list_files("https://x.y"), None);
        assert_eq!(uri_list_files(""), None);
        assert_eq!(file_name("/a/b/c.txt"), "c.txt");
        assert_eq!(file_name("/a/b/"), "b");
    }

    #[test]
    fn misc() {
        assert_eq!(human_size(512), "512 B");
        assert_eq!(human_size(2048), "2.0 KB");
        assert_eq!(relative_time(100_000, 99_000), "now");
        assert_eq!(relative_time(10 * 60_000, 0), "10m");
        assert_eq!(relative_time(3 * 3_600_000, 0), "3h");
        assert_eq!(truncate("abcdef", 3), "abc…");
        assert_eq!(truncate("abc", 3), "abc");
    }
}
