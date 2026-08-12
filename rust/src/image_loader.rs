//! Image decoding, rotation and GTK texture conversion.
//! Mirrors `mcomix/image_tools.py` + `mcomix/image_handler.py` (decoding part).

use std::io::Cursor;

use image::imageops;
use image::{Rgba, RgbaImage};
use log::warn;

use gtk4::gdk;
use gdk_pixbuf::prelude::*;

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

/// Build a small RGBA thumbnail using gdk-pixbuf's scaled decoding.
///
/// This is the same trick the Python app uses: for JPEG (and other formats
/// with decoder-side scaling) the image is decoded at reduced resolution, so
/// thumbnails are far cheaper than a full decode + resize. Returns
/// `(width, height, tight RGBA8 pixels)`.
pub fn thumbnail_pixbuf_rgba(bytes: &[u8], max_w: u32, max_h: u32) -> Option<(u32, u32, Vec<u8>)> {
    // Load through a PixbufLoader with a size hint: JPEG is decoded at
    // reduced resolution (libjpeg DCT scaling), which is the big speedup.
    let loader = gdk_pixbuf::PixbufLoader::new();
    loader.set_size(max_w as i32, max_h as i32);
    loader.write(bytes).ok()?;
    loader.close().ok()?;
    let mut pb = loader.pixbuf()?;

    // Loaders that ignore the size hint (e.g. PNG) produce a full-size
    // pixbuf; scale it down explicitly.
    let (w, h) = (pb.width(), pb.height());
    if w > max_w as i32 || h > max_h as i32 {
        let scale = ((max_w as f64 / w as f64).min(max_h as f64 / h as f64)).min(1.0);
        let nw = ((w as f64) * scale).max(1.0) as i32;
        let nh = ((h as f64) * scale).max(1.0) as i32;
        pb = pb.scale_simple(nw, nh, gdk_pixbuf::InterpType::Bilinear)?;
    }

    let w = pb.width() as u32;
    let h = pb.height() as u32;
    let stride = pb.rowstride() as usize;
    let nch = pb.n_channels() as usize;
    let src = pb.pixel_bytes()?;
    let src = src.as_ref();
    let mut rgba = vec![0u8; (w as usize) * (h as usize) * 4];
    if nch == 4 && stride == (w as usize) * 4 {
        let n = rgba.len();
        rgba.copy_from_slice(&src[..n]);
    } else {
        for y in 0..h as usize {
            for x in 0..w as usize {
                let si = y * stride + x * nch;
                let di = (y * (w as usize) + x) * 4;
                rgba[di] = src[si];
                rgba[di + 1] = src[si + 1];
                rgba[di + 2] = src[si + 2];
                rgba[di + 3] = if nch == 4 { src[si + 3] } else { 255 };
            }
        }
    }
    Some((w, h, rgba))
}

/// Fallback thumbnail via the pure-Rust `image` crate (used when gdk-pixbuf
/// cannot decode the format, e.g. some WebP/TIFF variants).
pub fn thumbnail_rgba_fallback(bytes: &[u8], max_w: u32, max_h: u32) -> Option<(u32, u32, Vec<u8>)> {
    let img = decode_rgba(bytes).ok()?;
    let thumb = thumbnail(&img, max_w, max_h);
    let (w, h) = thumb.dimensions();
    Some((w, h, thumb.into_raw()))
}

/// True if the format looks animated (GIF/WebP with animation). MComix has an
/// animation mode preference; v1 always renders the first frame.
pub fn format_hint(bytes: &[u8]) -> Option<image::ImageFormat> {
    image::guess_format(bytes).ok()
}

/// Used by the checkerboard background preference: returns a color.
pub fn checker_color(alpha: u8) -> Rgba<u8> {
    let _ = alpha;
    Rgba([0xcc, 0xcc, 0xcc, 0xff])
}

#[cfg(test)]
mod bench {
    use super::*;
    use std::time::Instant;

    fn load_image(name: &str) -> Vec<u8> {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("test")
            .join("files")
            .join("images")
            .join(name);
        std::fs::read(path).expect("read test image")
    }

    #[test]
    #[ignore]
    fn thumbnail_speed() {
        let names = ["landscape-no-exif.jpg", "portrait-no-exif.jpg", "pattern.jpg"];
        for name in names {
            let bytes = load_image(name);
            let n = 200;
            let t0 = Instant::now();
            for _ in 0..n {
                let _ = thumbnail_pixbuf_rgba(&bytes, 160, 160);
            }
            let t1 = Instant::now();
            for _ in 0..n {
                let _ = thumbnail_rgba_fallback(&bytes, 160, 160);
            }
            let t2 = Instant::now();
            eprintln!(
                "{name}: gdk-pixbuf {:?} vs image-crate {:?} ({:.1}x)",
                t1 - t0,
                t2 - t1,
                (t2 - t1).as_secs_f64() / (t1 - t0).as_secs_f64().max(1e-9)
            );
        }
    }
}
