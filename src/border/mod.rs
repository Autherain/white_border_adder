//! `wba border` writes a white-bordered copy of every JPEG and PNG, sized
//! for Instagram.
//!
//! Instagram crops what does not fit its frame. The copy is a white canvas
//! of the frame's size with the whole photo scaled down and centred on it,
//! so nothing is cropped. The original is never touched.

mod plan;
mod render;

use std::io::{Cursor, Read, Write};

use image::{ImageFormat, RgbaImage};
use vfs::{VfsError, VfsPath};

pub(crate) use plan::is_image;
pub use plan::plan;
use render::Decoded;

pub const DEFAULT_OUT: &str = "bordered";
/// Where the script that preceded wba wrote its bordered copies; those
/// trees still exist and must not be bordered again.
pub const LEGACY_OUT: &str = "bordered_images";

/// How thick the white border is, as a fraction of the canvas: 0.05 leaves
/// 5% of white on each of the two edges of that axis.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ratios {
    /// Top and bottom.
    pub vert: f64,
    /// Left and right.
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
    /// The canvas, in pixels.
    pub width: u32,
    pub height: u32,
    /// The borders of a photo wider than tall, and of the others.
    pub landscape: Ratios,
    pub portrait: Ratios,
    pub jpeg_quality: u8,
    /// Put before the photo's name to name its copy: bordered_IMG_1264.jpg.
    pub prefix: String,
    pub out: Out,
    pub recursive: bool,
    /// Writes a copy again even when it already exists.
    pub force: bool,
}

/// One copy to make: the photo to read and the bordered file to write.
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
    pub fn run(self, options: &Options) -> Result<VfsPath, Failed> {
        let input = self.input;
        match write(&input, &self.output, options) {
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

fn write(input: &VfsPath, output: &VfsPath, options: &Options) -> Result<(), Error> {
    let data = read_whole(input)?;
    let decoded = match format(input)? {
        ImageFormat::Jpeg => decode_jpeg(&data, options)?,
        _ => decode_png(&data)?,
    };
    let canvas = render::render(decoded, options)?;
    let encoded = encode(canvas, format(output)?, options.jpeg_quality)?;
    write_whole(output, &encoded)
}

/// The whole file in one read, and `write_whole` in one write: a network
/// mount pays a round trip per call, and the codecs work in small chunks.
fn read_whole(input: &VfsPath) -> Result<Vec<u8>, Error> {
    let mut data = Vec::new();
    let mut file = match input.open_file() {
        Ok(f) => f,
        Err(e) => return Err(Error::Read(e)),
    };
    if let Err(e) = file.read_to_end(&mut data) {
        return Err(Error::Read(e.into()));
    }
    Ok(data)
}

fn write_whole(output: &VfsPath, data: &[u8]) -> Result<(), Error> {
    if let Err(e) = output.parent().create_dir_all() {
        return Err(Error::Mkdir(e));
    }
    let mut file = match output.create_file() {
        Ok(f) => f,
        Err(e) => return Err(Error::Write(e)),
    };
    if let Err(e) = file.write_all(data) {
        return Err(Error::Write(e.into()));
    }
    Ok(())
}

fn encode(canvas: RgbaImage, format: ImageFormat, jpeg_quality: u8) -> Result<Vec<u8>, Error> {
    let mut buf = Cursor::new(Vec::new());

    if format == ImageFormat::Png {
        if let Err(e) = canvas.write_to(&mut buf, ImageFormat::Png) {
            return Err(Error::Encode(e));
        }
        return Ok(buf.into_inner());
    }

    // JPEG has no alpha channel; the canvas is opaque, so dropping it
    // loses nothing.
    let rgb = image::DynamicImage::ImageRgba8(canvas).to_rgb8();
    let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, jpeg_quality);
    if let Err(e) = rgb.write_with_encoder(encoder) {
        return Err(Error::Encode(e));
    }
    Ok(buf.into_inner())
}

/// A JPEG is decoded no larger than it needs to be: the decoder can skip
/// part of the inverse DCT and hand back a picture 1/2, 1/4 or 1/8 the
/// size, never smaller than asked. An 18 Mpx photo bound for a 1080 canvas
/// then never exists in memory.
fn decode_jpeg(data: &[u8], options: &Options) -> Result<Decoded, Error> {
    let mut decoder = jpeg_decoder::Decoder::new(data);
    if let Err(e) = decoder.read_info() {
        return Err(Error::DecodeJpeg(e));
    }
    let info = match decoder.info() {
        Some(i) => i,
        None => return Err(Error::Format("JPEG without a header".to_owned())),
    };

    let photo = render::layout(u32::from(info.width), u32::from(info.height), options);
    let (width, height) = match decoder.scale(fit_u16(photo.width), fit_u16(photo.height)) {
        Ok(size) => size,
        Err(e) => return Err(Error::DecodeJpeg(e)),
    };
    let pixels = match decoder.decode() {
        Ok(p) => p,
        Err(e) => return Err(Error::DecodeJpeg(e)),
    };

    Ok(Decoded {
        width: u32::from(width),
        height: u32::from(height),
        pixels: to_rgb(info.pixel_format, pixels)?,
        alpha: false,
    })
}

/// The decoder takes u16 sizes; a canvas past that asks for its largest.
fn fit_u16(size: u32) -> u16 {
    match u16::try_from(size) {
        Ok(s) => s,
        Err(_) => u16::MAX,
    }
}

/// The renderer wants three bytes a pixel; CMYK and 16-bit JPEG are refused.
fn to_rgb(format: jpeg_decoder::PixelFormat, pixels: Vec<u8>) -> Result<Vec<u8>, Error> {
    match format {
        jpeg_decoder::PixelFormat::RGB24 => Ok(pixels),
        jpeg_decoder::PixelFormat::L8 => Ok(grey_to_rgb(&pixels)),
        other => Err(Error::Format(format!("JPEG {other:?}"))),
    }
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
