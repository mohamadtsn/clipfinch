/// What kind of thing a clipboard entry is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    Text,
    Link,
    Image,
    File,
    Color,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Text => "text",
            Kind::Link => "link",
            Kind::Image => "image",
            Kind::File => "file",
            Kind::Color => "color",
        }
    }

    pub fn parse(s: &str) -> Option<Kind> {
        match s {
            "text" => Some(Kind::Text),
            "link" => Some(Kind::Link),
            "image" => Some(Kind::Image),
            "file" => Some(Kind::File),
            "color" => Some(Kind::Color),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Kind::Text => "Text",
            Kind::Link => "Link",
            Kind::Image => "Image",
            Kind::File => "File",
            Kind::Color => "Color",
        }
    }
}

/// One clipboard representation (a MIME type plus its bytes).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Format {
    pub mime: String,
    pub data: Vec<u8>,
}

/// A clip that is about to be stored.
#[derive(Debug, Clone)]
pub struct NewClip {
    pub kind: Kind,
    /// Searchable / preview text (capped).
    pub text: String,
    pub source_app: Option<String>,
    pub formats: Vec<Format>,
    /// PNG thumbnail, images only.
    pub thumb: Option<Vec<u8>>,
    pub hash: String,
    pub size: u64,
}

/// A stored clip (without its raw formats).
#[derive(Debug, Clone)]
pub struct Clip {
    pub id: i64,
    pub kind: Kind,
    pub text: String,
    pub title: Option<String>,
    pub source_app: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub pinboard_id: Option<i64>,
    pub size: i64,
    pub thumb: Option<Vec<u8>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pinboard {
    pub id: i64,
    pub name: String,
}
