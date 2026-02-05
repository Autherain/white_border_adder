//! White border adder — adds configurable white borders and scales images to a target size.
//! Serial version (no parallelism).

use clap::Parser;
use image::imageops::FilterType;
use image::{imageops, GenericImage, ImageBuffer, RgbaImage, Rgba};
use std::path::{Path, PathBuf};
use std::time::Instant;

const WHITE: Rgba<u8> = Rgba([255, 255, 255, 255]);

/// Add white borders to images and scale to target dimensions.
#[derive(Parser, Debug)]
#[command(name = "white_border_adder")]
#[command(about = "Add white borders to images in a folder")]
struct Args {
    /// Input folder containing images (required unless using -i)
    #[arg(index = 1)]
    input: Option<PathBuf>,

    /// Input folder (alternative to positional)
    #[arg(short = 'i', long = "input")]
    input_flag: Option<PathBuf>,

    /// Target width for output images
    #[arg(long, default_value_t = 1080)]
    width: u32,

    /// Target height for output images
    #[arg(long, default_value_t = 1080)]
    height: u32,

    /// Vertical border ratio for landscape images (0.0–1.0)
    #[arg(long, default_value_t = 0.05)]
    landscape_vert: f64,

    /// Horizontal border ratio for landscape images
    #[arg(long, default_value_t = 0.03)]
    landscape_horiz: f64,

    /// Vertical border ratio for portrait images
    #[arg(long, default_value_t = 0.005)]
    portrait_vert: f64,

    /// Horizontal border ratio for portrait images
    #[arg(long, default_value_t = 0.18)]
    portrait_horiz: f64,

    /// JPEG output quality (1–100)
    #[arg(long, default_value_t = 100)]
    jpeg_quality: u8,

    /// Prefix for output filenames
    #[arg(long, default_value = "bordered_")]
    prefix: String,

    /// Write output into a separate subfolder "bordered_images"
    #[arg(long, default_value_t = true)]
    separate_folder: bool,
}

#[derive(Clone, Copy)]
struct Config {
    target_width: u32,
    target_height: u32,
    landscape_vert_border: f64,
    landscape_horiz_border: f64,
    portrait_vert_border: f64,
    portrait_horiz_border: f64,
    jpeg_quality: u8,
    separate_folder: bool,
}

impl Config {
    fn from_args(args: &Args) -> Self {
        Self {
            target_width: args.width,
            target_height: args.height,
            landscape_vert_border: args.landscape_vert,
            landscape_horiz_border: args.landscape_horiz,
            portrait_vert_border: args.portrait_vert,
            portrait_horiz_border: args.portrait_horiz,
            jpeg_quality: args.jpeg_quality,
            separate_folder: args.separate_folder,
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let config = Config::from_args(&args);
    let input_folder = args
        .input
        .as_ref()
        .or(args.input_flag.as_ref())
        .cloned()
        .ok_or("Error: Input folder is required (pass as argument or use -i/--input)")?;
    let using_defaults = std::env::args().len() == 2
        && std::env::args().nth(1).map(|a| !a.starts_with('-')).unwrap_or(false);

    print_config(&config, using_defaults);

    let main_start = Instant::now();

    let output_folder: PathBuf = if config.separate_folder {
        input_folder.join("bordered_images")
    } else {
        input_folder.clone()
    };

    if config.separate_folder {
        std::fs::create_dir_all(&output_folder)?;
    }

    let entries = std::fs::read_dir(&input_folder)?;
    let mut total_ok = 0usize;
    let mut total_fail = 0usize;
    let mut total_duration = std::time::Duration::ZERO;
    let mut fastest: Option<(String, std::time::Duration)> = None;
    let mut slowest: Option<(String, std::time::Duration)> = None;

    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|s| s.to_lowercase())
            .unwrap_or_default();
        if ext != "jpg" && ext != "jpeg" && ext != "png" {
            continue;
        }

        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();
        let output_path = output_folder.join(format!("{}{}", args.prefix, filename));

        let start = Instant::now();
        match process_image(&path, &output_path, &config) {
            Ok(()) => {
                total_ok += 1;
                let elapsed = start.elapsed();
                total_duration += elapsed;
                println!("✅ Successfully processed {} in {:.2} seconds", filename, elapsed.as_secs_f64());
                if fastest.as_ref().map(|(_, d)| elapsed < *d).unwrap_or(true) {
                    fastest = Some((filename.clone(), elapsed));
                }
                if slowest.as_ref().map(|(_, d)| elapsed > *d).unwrap_or(true) {
                    slowest = Some((filename, elapsed));
                }
            }
            Err(e) => {
                total_fail += 1;
                eprintln!("❌ Error processing {}: {}", filename, e);
            }
        }
    }

    let main_elapsed = main_start.elapsed();
    println!("\nTotal execution time: {:.2} seconds", main_elapsed.as_secs_f64());
    println!("\n📊 === Processing Summary ===");
    println!("✅ Total images processed: {}", total_ok);
    println!("❌ Failed images: {}", total_fail);
    if total_ok > 0 {
        let avg = total_duration.as_secs_f64() / total_ok as f64;
        println!("⏱️  Average processing time: {:.2} seconds", avg);
        if let Some((name, d)) = &fastest {
            println!("🚀 Fastest image: {} ({:.2} seconds)", name, d.as_secs_f64());
        }
        if let Some((name, d)) = &slowest {
            println!("🐢 Slowest image: {} ({:.2} seconds)", name, d.as_secs_f64());
        }
    }
    println!();

    Ok(())
}

fn print_config(config: &Config, using_defaults: bool) {
    println!("\n=== Configuration ===");
    if using_defaults {
        println!("Using default configuration (no flags provided)");
    }
    println!(
        "Target dimensions: {}x{}",
        config.target_width, config.target_height
    );
    println!(
        "Landscape borders: Vertical={:.1}%, Horizontal={:.1}%",
        config.landscape_vert_border * 100.0,
        config.landscape_horiz_border * 100.0
    );
    println!(
        "Portrait borders: Vertical={:.1}%, Horizontal={:.1}%",
        config.portrait_vert_border * 100.0,
        config.portrait_horiz_border * 100.0
    );
    println!("JPEG quality: {}", config.jpeg_quality);
    println!("Separate output folder: {}", config.separate_folder);
    println!("==================\n");
}

fn process_image(
    input_path: &Path,
    output_path: &Path,
    config: &Config,
) -> Result<(), Box<dyn std::error::Error>> {
    let img = image::open(input_path)?.to_rgba8();
    let (orig_width, orig_height) = img.dimensions();
    let is_landscape = orig_width > orig_height;

    let (vert_ratio, horiz_ratio) = if is_landscape {
        (config.landscape_vert_border, config.landscape_horiz_border)
    } else {
        (config.portrait_vert_border, config.portrait_horiz_border)
    };

    let available_width = config.target_width as f64 * (1.0 - 2.0 * horiz_ratio);
    let available_height = config.target_height as f64 * (1.0 - 2.0 * vert_ratio);

    let scale = (available_width / orig_width as f64).min(available_height / orig_height as f64);

    let scaled_width = (orig_width as f64 * scale).round() as u32;
    let scaled_height = (orig_height as f64 * scale).round() as u32;

    // White canvas
    let mut canvas: RgbaImage =
        ImageBuffer::from_pixel(config.target_width, config.target_height, WHITE);

    // Resize source image (bilinear-like filter)
    let resized = imageops::resize(
        &img,
        scaled_width,
        scaled_height,
        FilterType::Triangle,
    );

    let offset_x = (config.target_width - scaled_width) / 2;
    let offset_y = (config.target_height - scaled_height) / 2;

    canvas.copy_from(&resized, offset_x, offset_y)?;

    let out_ext = output_path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_lowercase())
        .unwrap_or_default();

    if out_ext == "png" {
        canvas.save(output_path)?;
    } else {
        let mut out_file = std::fs::File::create(output_path)?;
        let mut encoder =
            image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out_file, config.jpeg_quality);
        encoder.encode_image(&canvas)?;
    }

    Ok(())
}
