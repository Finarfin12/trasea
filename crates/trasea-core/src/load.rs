use crate::{Error, Result};
use image::{imageops::FilterType, DynamicImage, RgbImage};
use std::path::Path;

/// Default longest side after loading (the architecture's limit).
pub const DEFAULT_MAX_SIDE: u32 = 1600;

pub fn load_path(path: &Path, max_side: u32) -> Result<RgbImage> {
    let img = image::open(path).map_err(|e| Error::Load(e.to_string()))?;
    Ok(from_dynamic(img, max_side))
}

pub fn load_bytes(bytes: &[u8], max_side: u32) -> Result<RgbImage> {
    let img = image::load_from_memory(bytes).map_err(|e| Error::Load(e.to_string()))?;
    Ok(from_dynamic(img, max_side))
}

/// Flatten transparency onto white and limit the longest side.
pub fn from_dynamic(img: DynamicImage, max_side: u32) -> RgbImage {
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    let mut rgb = RgbImage::new(w, h);
    for (x, y, p) in rgba.enumerate_pixels() {
        let a = p[3] as u32;
        let blend = |c: u8| (((c as u32) * a + 255 * (255 - a) + 127) / 255) as u8;
        rgb.put_pixel(x, y, image::Rgb([blend(p[0]), blend(p[1]), blend(p[2])]));
    }
    let long = w.max(h);
    if max_side > 0 && long > max_side {
        let f = max_side as f32 / long as f32;
        let nw = ((w as f32 * f).round() as u32).max(1);
        let nh = ((h as f32 * f).round() as u32).max(1);
        return image::imageops::resize(&rgb, nw, nh, FilterType::Lanczos3);
    }
    rgb
}
