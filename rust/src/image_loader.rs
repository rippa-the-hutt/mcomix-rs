//! Image decoding, rotation and GTK texture conversion.
//! Mirrors `mcomix/image_tools.py` + `mcomix/image_handler.py` (decoding part).

use std::io::Cursor;

use image::imageops;
use image::{ImageFormat, Rgba, RgbaImage};
use log::warn;

use gtk4::gdk;

/// Decode raw image bytes into an RGBA8 image.
pub fn decode_rgba(bytes: &[u8]) -> Result<RgbaImage, String> {
    let img = image::load_from_memory(bytes)
        .map_err(|e| format!("cannot decode image: {e}"))?;
    Ok(img.to_rgba8())
}

/// Apply rotation (degrees clockwise, 0/90/180/270) and flips, mirroring
/// GdkPixbuf's rotation semantics used by MComix (`rotate_90` = clockwise).
pub fn transform(img: &RgbaImage, rotation_deg: i32, flip_h: bool, flip_v: bool) -> RgbaImage {
    let mut out = img.clone();
    let rot = rotation_deg.rem_euclid(360);
    match rot {
        // image crate's rotate90 is counter-clockwise, rotate270 is clockwise.
        90 => out = imageops::rotate270(&out),
        180 => out = imageops::rotate180(&out),
        270 => out = imageops::rotate90(&out),
        _ => {}
    }
    if flip_h {
        imageops::flip_horizontal_in_place(&mut out);
    }
    if flip_v {
        imageops::flip_vertical_in_place(&mut out);
    }
    out
}

/// Wrap an RGBA8 image in a `gdk::Texture` (memory texture).
pub fn texture_from_rgba(img: &RgbaImage) -> gdk::Texture {
    let (w, h) = img.dimensions();
    let bytes = glib::Bytes::from(img.as_raw());
    let stride = (w as usize) * 4;
    gdk::MemoryTexture::new(w as i32, h as i32, gdk::MemoryFormat::R8g8b8a8, &bytes, stride).into()
}

/// Get image dimensions without a full decode.
pub fn image_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    let reader = image::ImageReader::new(Cursor::new(bytes));
    reader.with_guessed_format().ok()?.into_dimensions().ok()
}

/// Resize an image to fit within `max_w x max_h`, preserving aspect ratio.
pub fn thumbnail(img: &RgbaImage, max_w: u32, max_h: u32) -> RgbaImage {
    let (w, h) = img.dimensions();
    let scale = ((max_w as f64 / w as f64).min(max_h as f64 / h as f64)).min(1.0);
    if scale >= 1.0 {
        return img.clone();
    }
    let nw = ((w as f64) * scale).max(1.0) as u32;
    let nh = ((h as f64) * scale).max(1.0) as u32;
    imageops::resize(img, nw, nh, imageops::FilterType::Triangle)
}

/// Encode an RGBA image as PNG bytes (used to hand thumbnails to the UI thread).
pub fn encode_png(img: &RgbaImage) -> Result<Vec<u8>, String> {
    let mut out = Cursor::new(Vec::new());
    img.write_to(&mut out, ImageFormat::Png)
        .map_err(|e| format!("cannot encode PNG: {e}"))?;
    Ok(out.into_inner())
}

/// Decode PNG bytes (from the thumbnail thread) into a texture.
pub fn texture_from_png_bytes(bytes: &[u8]) -> Option<gdk::Texture> {
    match decode_rgba(bytes) {
        Ok(img) => Some(texture_from_rgba(&img)),
        Err(e) => {
            warn!("thumbnail decode failed: {e}");
            None
        }
    }
}

/// True if the format looks animated (GIF/WebP with animation). MComix has an
/// animation mode preference; v1 always renders the first frame.
pub fn format_hint(bytes: &[u8]) -> Option<ImageFormat> {
    image::guess_format(bytes).ok()
}

/// Used by the checkerboard background preference: returns a color.
pub fn checker_color(alpha: u8) -> Rgba<u8> {
    let _ = alpha;
    Rgba([0xcc, 0xcc, 0xcc, 0xff])
}
