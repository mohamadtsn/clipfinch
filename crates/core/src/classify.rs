//! Turns the raw clipboard formats into a classified, hashed [`NewClip`].

use sha2::{Digest, Sha256};

use crate::imgutil;
use crate::model::{Format, Kind, NewClip};
use crate::text;

/// Searchable text is capped to keep the database and the UI light.
const MAX_TEXT_CHARS: usize = 20_000;

/// MIME types password managers set on sensitive copies.
pub fn is_secret_mime(m: &str) -> bool {
    matches!(
        m,
        "x-kde-passwordManagerHint"
            | "org.nspasteboard.ConcealedType"
            | "org.nspasteboard.TransientType"
            | "application/x-nspasteboard-concealed-type"
    )
}

/// X11 housekeeping targets that are never real content.
pub fn is_ignored_mime(m: &str) -> bool {
    matches!(
        m,
        "TARGETS" | "TIMESTAMP" | "MULTIPLE" | "SAVE_TARGETS" | "DELETE" | "INSERT_SELECTION"
            | "INSERT_PROPERTY"
    )
}

pub fn is_text_mime(m: &str) -> bool {
    m.starts_with("text/plain")
        || matches!(m, "UTF8_STRING" | "STRING" | "TEXT" | "COMPOUND_TEXT")
}

/// Best plain-text representation among `formats`.
pub fn text_of(formats: &[Format]) -> Option<String> {
    let pick = formats
        .iter()
        .find(|f| f.mime == "text/plain;charset=utf-8")
        .or_else(|| formats.iter().find(|f| f.mime == "UTF8_STRING"))
        .or_else(|| formats.iter().find(|f| f.mime == "text/plain"))
        .or_else(|| formats.iter().find(|f| is_text_mime(&f.mime)));
    pick.map(|f| String::from_utf8_lossy(&f.data).into_owned())
}

/// Preferred image representation (PNG first).
pub fn best_image(formats: &[Format]) -> Option<&Format> {
    formats
        .iter()
        .find(|f| f.mime == "image/png")
        .or_else(|| formats.iter().find(|f| f.mime.starts_with("image/")))
}

fn hash(kind: Kind, bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(kind.as_str().as_bytes());
    h.update([0u8]);
    h.update(bytes);
    format!("{:x}", h.finalize())
}

pub fn build_new_clip(formats: Vec<Format>, source_app: Option<String>) -> Option<NewClip> {
    if formats.is_empty() {
        return None;
    }
    let size: u64 = formats.iter().map(|f| f.data.len() as u64).sum();
    let plain = text_of(&formats).filter(|t| !t.trim().is_empty());
    let mut thumb: Option<Vec<u8>> = None;

    let (kind, display, hash_src): (Kind, String, Vec<u8>) = if let Some((paths, raw)) = formats
        .iter()
        .find(|f| f.mime == "text/uri-list")
        .and_then(|f| {
            let s = String::from_utf8_lossy(&f.data).into_owned();
            text::uri_list_files(&s).map(|p| (p, f.data.clone()))
        }) {
        (Kind::File, paths.join("\n"), raw)
    } else if let Some(t) = plain {
        let kind = if text::parse_color(&t).is_some() {
            Kind::Color
        } else if text::is_url(&t) {
            Kind::Link
        } else {
            Kind::Text
        };
        let bytes = t.as_bytes().to_vec();
        (kind, t.trim_end().to_string(), bytes)
    } else if let Some(img) = best_image(&formats) {
        let label = match imgutil::thumbnail(&img.data, 360, 200) {
            Some(t) => {
                let l = format!(
                    "Image {}×{} · {}",
                    t.width,
                    t.height,
                    text::human_size(img.data.len() as u64)
                );
                thumb = Some(t.png);
                l
            }
            None => format!("Image · {}", text::human_size(img.data.len() as u64)),
        };
        (Kind::Image, label, img.data.clone())
    } else if let Some(c) = formats
        .iter()
        .find(|f| f.mime == "application/x-color" && f.data.len() >= 8)
    {
        let ch = |i: usize| (u16::from_ne_bytes([c.data[i], c.data[i + 1]]) >> 8) as u8;
        let hex = text::color_hex(ch(0), ch(2), ch(4), ch(6));
        (Kind::Color, hex.clone(), hex.into_bytes())
    } else {
        return None;
    };

    Some(NewClip {
        kind,
        text: text::take_chars(&display, MAX_TEXT_CHARS),
        source_app,
        formats,
        thumb,
        hash: hash(kind, &hash_src),
        size,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fmt(m: &str, d: &[u8]) -> Format {
        Format {
            mime: m.to_string(),
            data: d.to_vec(),
        }
    }

    #[test]
    fn classifies_text_link_color() {
        let c = build_new_clip(vec![fmt("text/plain;charset=utf-8", b"hello")], None).unwrap();
        assert_eq!(c.kind, Kind::Text);
        let c = build_new_clip(vec![fmt("text/plain", b"https://a.b/c")], None).unwrap();
        assert_eq!(c.kind, Kind::Link);
        let c = build_new_clip(vec![fmt("UTF8_STRING", b"#ff8800")], None).unwrap();
        assert_eq!(c.kind, Kind::Color);
    }

    #[test]
    fn classifies_files() {
        let c = build_new_clip(
            vec![fmt("text/uri-list", b"file:///tmp/a%20b.txt\r\n")],
            None,
        )
        .unwrap();
        assert_eq!(c.kind, Kind::File);
        assert_eq!(c.text, "/tmp/a b.txt");
    }

    #[test]
    fn empty_text_is_ignored_and_hash_is_stable() {
        assert!(build_new_clip(vec![fmt("text/plain", b"   \n")], None).is_none());
        let a = build_new_clip(vec![fmt("text/plain", b"same")], None).unwrap();
        let b = build_new_clip(vec![fmt("text/plain", b"same")], Some("x".into())).unwrap();
        assert_eq!(a.hash, b.hash);
    }

    #[test]
    fn secret_mimes() {
        assert!(is_secret_mime("x-kde-passwordManagerHint"));
        assert!(!is_secret_mime("text/plain"));
    }
}
