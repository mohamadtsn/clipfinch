//! Smart-word search.
//!
//! * Words that name a kind (`text`, `link`, `image`, `file`, `color`) filter by kind.
//! * Every other word must appear in the clip text, source app or title.
//! * `"quoted words"` are always literal (never treated as a kind) and may contain spaces.

use crate::model::Kind;

#[derive(Debug, Default, Clone)]
pub struct Query {
    kinds: Vec<Kind>,
    words: Vec<String>,
}

fn type_word(w: &str) -> Option<Kind> {
    match w {
        "text" | "texts" => Some(Kind::Text),
        "link" | "links" | "url" | "urls" => Some(Kind::Link),
        "image" | "images" => Some(Kind::Image),
        "file" | "files" => Some(Kind::File),
        "color" | "colors" | "colour" | "colours" => Some(Kind::Color),
        _ => None,
    }
}

fn push_token(q: &mut Query, tok: &str, quoted: bool) {
    let t = tok.trim();
    if t.is_empty() {
        return;
    }
    let lower = t.to_lowercase();
    if !quoted {
        if let Some(k) = type_word(&lower) {
            if !q.kinds.contains(&k) {
                q.kinds.push(k);
            }
            return;
        }
    }
    q.words.push(lower);
}

pub fn parse(input: &str) -> Query {
    let mut q = Query::default();
    let mut cur = String::new();
    let mut in_quote = false;
    for ch in input.chars() {
        if ch == '"' {
            if in_quote {
                push_token(&mut q, &cur, true);
                in_quote = false;
            } else {
                push_token(&mut q, &cur, false);
                in_quote = true;
            }
            cur.clear();
            continue;
        }
        if ch.is_whitespace() && !in_quote {
            push_token(&mut q, &cur, false);
            cur.clear();
            continue;
        }
        cur.push(ch);
    }
    // An unterminated quote is treated as literal.
    push_token(&mut q, &cur, in_quote);
    q
}

impl Query {
    pub fn is_empty(&self) -> bool {
        self.kinds.is_empty() && self.words.is_empty()
    }

    pub fn kinds(&self) -> &[Kind] {
        &self.kinds
    }

    pub fn words(&self) -> &[String] {
        &self.words
    }

    pub fn matches(
        &self,
        kind: Kind,
        text: &str,
        app: Option<&str>,
        title: Option<&str>,
    ) -> bool {
        if !self.kinds.is_empty() && !self.kinds.contains(&kind) {
            return false;
        }
        if self.words.is_empty() {
            return true;
        }
        let t = text.to_lowercase();
        let a = app.map(|s| s.to_lowercase()).unwrap_or_default();
        let ti = title.map(|s| s.to_lowercase()).unwrap_or_default();
        self.words
            .iter()
            .all(|w| t.contains(w.as_str()) || a.contains(w.as_str()) || ti.contains(w.as_str()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_words_filter() {
        let q = parse("link");
        assert!(q.matches(Kind::Link, "https://x.y", None, None));
        assert!(!q.matches(Kind::Text, "link in text", None, None));
    }

    #[test]
    fn mixed_words() {
        let q = parse("mail link slides");
        assert!(q.matches(Kind::Link, "https://slides.example", Some("Thunderbird mail"), None));
        assert!(!q.matches(Kind::Link, "https://slides.example", Some("Firefox"), None));
        assert!(!q.matches(Kind::Text, "slides mail", None, None));
    }

    #[test]
    fn quotes_are_literal() {
        let q = parse("\"link\"");
        assert!(q.kinds().is_empty());
        assert!(q.matches(Kind::Text, "a link here", None, None));
        let q = parse("\"hello world\" file");
        assert_eq!(q.words(), &["hello world".to_string()]);
        assert_eq!(q.kinds(), &[Kind::File]);
    }

    #[test]
    fn empty_matches_everything() {
        assert!(parse("   ").is_empty());
        assert!(parse("").matches(Kind::Image, "", None, None));
    }

    #[test]
    fn title_matches() {
        let q = parse("invoice");
        assert!(q.matches(Kind::Text, "abc", None, Some("My Invoice")));
    }
}
