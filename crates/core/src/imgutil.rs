use std::io::Cursor;

pub struct Thumb {
    pub png: Vec<u8>,
    /// Dimensions of the ORIGINAL image.
    pub width: u32,
    pub height: u32,
}

/// Decodes an image and returns a small PNG thumbnail plus the original size.
pub fn thumbnail(data: &[u8], max_w: u32, max_h: u32) -> Option<Thumb> {
    let img = image::load_from_memory(data).ok()?;
    let (width, height) = (img.width(), img.height());
    let small = img.thumbnail(max_w, max_h);
    let mut out = Cursor::new(Vec::new());
    small.write_to(&mut out, image::ImageFormat::Png).ok()?;
    Some(Thumb {
        png: out.into_inner(),
        width,
        height,
    })
}
