use fast_image_resize::images::Image;
use fast_image_resize::{FilterType, PixelType, ResizeAlg, ResizeOptions, Resizer};
use image::imageops;
use image::{DynamicImage, RgbImage, Rgba, RgbaImage};

use super::{Error, Options};

/// Where the scaled photo lands on the canvas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// Pixels as the decoder left them: RGB for a JPEG, RGBA for a PNG.
pub struct Decoded {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
    pub alpha: bool,
}

/// Fits an `orig_w` x `orig_h` image inside the canvas minus its borders and
/// centres it. A square image counts as portrait, as in the original tool.
pub fn layout(orig_w: u32, orig_h: u32, o: &Options) -> Rect {
    let ratios = if orig_w > orig_h {
        o.landscape
    } else {
        o.portrait
    };
    let sides = 2.0;

    let avail_w = f64::from(o.width) * (1.0 - sides * ratios.horiz);
    let avail_h = f64::from(o.height) * (1.0 - sides * ratios.vert);
    let scale = f64::min(avail_w / f64::from(orig_w), avail_h / f64::from(orig_h));

    // Truncation is the intent: the photo never outgrows the available area.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let (width, height) = (
        (f64::from(orig_w) * scale) as u32,
        (f64::from(orig_h) * scale) as u32,
    );

    Rect {
        x: (o.width - width) / 2,
        y: (o.height - height) / 2,
        width,
        height,
    }
}

/// The photo scaled and centred on a white canvas. The layout is computed
/// from the decoded size, which for a JPEG may already be reduced: the
/// ratio is the same, so the frame is the same.
pub fn render(img: Decoded, o: &Options) -> Result<RgbaImage, Error> {
    let at = layout(img.width, img.height, o);
    let small = resize(img, at.width, at.height)?;

    let mut canvas = RgbaImage::from_pixel(o.width, o.height, Rgba([255, 255, 255, 255]));
    imageops::overlay(&mut canvas, &small, i64::from(at.x), i64::from(at.y));
    Ok(canvas)
}

/// Bilinear on SIMD lanes: the photo only ever shrinks, so a wider filter
/// buys nothing visible.
const WRONG_SIZE: &str = "resized buffer has the wrong size";

fn resize(img: Decoded, width: u32, height: u32) -> Result<RgbaImage, Error> {
    let kind = if img.alpha {
        PixelType::U8x4
    } else {
        PixelType::U8x3
    };
    let src = match Image::from_vec_u8(img.width, img.height, img.pixels, kind) {
        Ok(s) => s,
        Err(e) => return Err(Error::Pixels(e)),
    };
    let mut dst = Image::new(width, height, kind);

    let bilinear = ResizeOptions::new().resize_alg(ResizeAlg::Convolution(FilterType::Bilinear));
    if let Err(e) = Resizer::new().resize(&src, &mut dst, &bilinear) {
        return Err(Error::Resize(e));
    }

    if img.alpha {
        return match RgbaImage::from_raw(width, height, dst.into_vec()) {
            Some(out) => Ok(out),
            None => Err(Error::Format(WRONG_SIZE.to_owned())),
        };
    }
    let rgb = match RgbImage::from_raw(width, height, dst.into_vec()) {
        Some(rgb) => rgb,
        None => return Err(Error::Format(WRONG_SIZE.to_owned())),
    };
    Ok(DynamicImage::ImageRgb8(rgb).to_rgba8())
}

#[cfg(test)]
#[path = "render_test.rs"]
mod tests;
