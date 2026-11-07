//! A white-bordered copy of every JPEG and PNG, sized for Instagram.

mod plan;
mod render;

use std::io::{Cursor, Read, Write};

use image::ImageFormat;
use vfs::{VfsError, VfsPath};

pub(crate) use plan::is_image;
pub use plan::plan;
use render::Decoded;

pub const DEFAULT_OUT: &str = "bordered";
/// Where the script that preceded wba wrote its bordered copies; those
/// trees still exist and must not be bordered again.
pub const LEGACY_OUT: &str = "bordered_images";

/// A border ratio, as a fraction of the canvas, applied to both edges of
/// its axis.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ratios {
    pub vert: f64,
    pub horiz: f64,
}

/// Where the copies go: beside each photo directory, or one directory for
/// all of them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Out {
    Beside(String),
    At(VfsPath),
}

impl Default for Out {
    fn default() -> Self {
        Self::Beside(DEFAULT_OUT.to_owned())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Options {
    pub width: u32,
    pub height: u32,
    pub landscape: Ratios,
    pub portrait: Ratios,
    pub jpeg_quality: u8,
    pub prefix: String,
    pub out: Out,
    pub recursive: bool,
    pub force: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Job {
    pub input: VfsPath,
    pub output: VfsPath,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("read input file: {0}")]
    Read(VfsError),
    #[error("unsupported image format {0:?}")]
    Format(String),
    #[error("decode image: {0}")]
    Decode(image::ImageError),
    #[error("decode JPEG: {0}")]
    DecodeJpeg(jpeg_decoder::Error),
    #[error("resize image: {0}")]
    Resize(fast_image_resize::ResizeError),
    #[error("decoded pixels do not match their size: {0}")]
    Pixels(fast_image_resize::ImageBufferError),
    #[error("encode output image: {0}")]
    Encode(image::ImageError),
    #[error("create output directory: {0}")]
    Mkdir(VfsError),
    #[error("write output file: {0}")]
    Write(VfsError),
}

#[derive(Debug, thiserror::Error)]
#[error("{}: {source}", input.as_str())]
pub struct Failed {
    pub input: VfsPath,
    #[source]
    pub source: Error,
}

impl Job {
    /// Decodes the input, renders it on the canvas and writes the output.
    /// The job is consumed: one copy per job. The output path comes back
    /// for the caller to print.
    pub fn run(self, o: &Options) -> Result<VfsPath, Failed> {
        let input = self.input;
        match write(&input, &self.output, o) {
            Ok(()) => Ok(self.output),
            Err(source) => Err(Failed { input, source }),
        }
    }
}

fn format(path: &VfsPath) -> Result<ImageFormat, Error> {
    match crate::path::ext(&path.filename()).as_str() {
        ".jpg" | ".jpeg" => Ok(ImageFormat::Jpeg),
        ".png" => Ok(ImageFormat::Png),
        other => Err(Error::Format(other.to_owned())),
    }
}

fn write(input: &VfsPath, output: &VfsPath, o: &Options) -> Result<(), Error> {
    // The whole file in one read and one write: a network mount pays a
    // round trip per call, and the codecs work in small chunks.
    let mut data = Vec::new();
    let mut file = match input.open_file() {
        Ok(f) => f,
        Err(e) => return Err(Error::Read(e)),
    };
    if let Err(e) = file.read_to_end(&mut data) {
        return Err(Error::Read(e.into()));
    }

    let decoded = match format(input)? {
        ImageFormat::Jpeg => decode_jpeg(&data, o)?,
        _ => decode_png(&data)?,
    };
    let canvas = render::render(decoded, o)?;

    let mut buf = Cursor::new(Vec::new());
    let encoded = if format(output)? == ImageFormat::Png {
        canvas.write_to(&mut buf, ImageFormat::Png)
    } else {
        let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, o.jpeg_quality);
        image::DynamicImage::ImageRgba8(canvas)
            .to_rgb8()
            .write_with_encoder(encoder)
    };
    if let Err(e) = encoded {
        return Err(Error::Encode(e));
    }

    if let Err(e) = output.parent().create_dir_all() {
        return Err(Error::Mkdir(e));
    }
    let mut file = match output.create_file() {
        Ok(f) => f,
        Err(e) => return Err(Error::Write(e)),
    };
    if let Err(e) = file.write_all(buf.get_ref()) {
        return Err(Error::Write(e.into()));
    }
    Ok(())
}

/// A JPEG is decoded no larger than it needs to be: the decoder can skip
/// part of the inverse DCT and hand back a picture 1/2, 1/4 or 1/8 the
/// size, never smaller than asked. An 18 Mpx photo bound for a 1080 canvas
/// then never exists in memory.
fn decode_jpeg(data: &[u8], o: &Options) -> Result<Decoded, Error> {
    let mut decoder = jpeg_decoder::Decoder::new(data);
    if let Err(e) = decoder.read_info() {
        return Err(Error::DecodeJpeg(e));
    }
    let info = match decoder.info() {
        Some(i) => i,
        None => return Err(Error::Format("JPEG without a header".to_owned())),
    };

    let at = render::layout(u32::from(info.width), u32::from(info.height), o);
    // The decoder takes u16 sizes; a canvas past that asks for its largest.
    let wanted_w = match u16::try_from(at.width) {
        Ok(w) => w,
        Err(_) => u16::MAX,
    };
    let wanted_h = match u16::try_from(at.height) {
        Ok(h) => h,
        Err(_) => u16::MAX,
    };
    let (width, height) = match decoder.scale(wanted_w, wanted_h) {
        Ok(size) => size,
        Err(e) => return Err(Error::DecodeJpeg(e)),
    };
    let pixels = match decoder.decode() {
        Ok(p) => p,
        Err(e) => return Err(Error::DecodeJpeg(e)),
    };

    let rgb = match info.pixel_format {
        jpeg_decoder::PixelFormat::RGB24 => pixels,
        jpeg_decoder::PixelFormat::L8 => grey_to_rgb(&pixels),
        other => return Err(Error::Format(format!("JPEG {other:?}"))),
    };
    Ok(Decoded {
        width: u32::from(width),
        height: u32::from(height),
        pixels: rgb,
        alpha: false,
    })
}

fn grey_to_rgb(grey: &[u8]) -> Vec<u8> {
    let mut rgb = Vec::with_capacity(grey.len() * 3);
    for &g in grey {
        rgb.extend_from_slice(&[g, g, g]);
    }
    rgb
}

fn decode_png(data: &[u8]) -> Result<Decoded, Error> {
    let img = match image::load_from_memory_with_format(data, ImageFormat::Png) {
        Ok(i) => i,
        Err(e) => return Err(Error::Decode(e)),
    };
    let rgba = img.to_rgba8();
    Ok(Decoded {
        width: rgba.width(),
        height: rgba.height(),
        pixels: rgba.into_raw(),
        alpha: true,
    })
}

#[cfg(test)]
#[path = "mod_test.rs"]
mod tests;
