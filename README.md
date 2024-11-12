# white_border_adder
Lil' project to add white border onto photos ٩(^ᗜ^ )و ´-

I was frustrated to have no reliable and easy solution to add white borders to my photos to post them onto instagram.

## Features
- 🖼️ Bulk processing of images (JPG, JPEG, PNG)
- ⚡ Concurrent processing with configurable worker pool
- 🎯 Smart border sizing for both landscape and portrait orientations
- 📊 Detailed processing statistics and progress tracking
- 💪 Maintains aspect ratio while fitting to target dimensions
- 📁 Option to create a separate output folder
- ⚙️ Highly configurable through command-line flags

## Quick Start
```bash
# Basic usage with default settings
go run main.go /path/to/your/photos

# Or build and run
go build
./white_border_adder /path/to/your/photos
```

## Configuration Options
All parameters can be customized using command-line flags:

| Flag | Default | Description |
|------|---------|-------------|
| `-width` | 1080 | Target width for output images |
| `-height` | 1080 | Target height for output images |
| `-landscape-vert` | 0.05 | Vertical border ratio for landscape images (5%) |
| `-landscape-horiz` | 0.03 | Horizontal border ratio for landscape images (3%) |
| `-portrait-vert` | 0.005 | Vertical border ratio for portrait images (0.5%) |
| `-portrait-horiz` | 0.18 | Horizontal border ratio for portrait images (18%) |
| `-batch-size` | 10 | Number of images to process in each batch |
| `-workers` | 1000 | Maximum number of concurrent workers |
| `-jpeg-quality` | 100 | JPEG output quality (1-100) |
| `-prefix` | "bordered_" | Prefix for output filenames |
| `-separate-folder` | true | Create separate folder for output |

## Advanced Usage Examples
```bash
# Custom dimensions and borders
./white_border_adder -width 1200 -height 1200 -landscape-vert 0.1 -landscape-horiz 0.05 /path/to/photos

# High-performance processing
./white_border_adder -batch-size 20 -workers 2000 /path/to/photos

# Custom output settings
./white_border_adder -prefix "insta_" -separate-folder=false -jpeg-quality 95 /path/to/photos
```

## Output
- Processed images are saved with the configured prefix (default: "bordered_")
- By default, outputs are saved in a new "bordered_images" subdirectory
- Progress and statistics are displayed in real-time:
  - ✅ Successfully processed images
  - ❌ Failed images (if any)
  - ⏱️ Processing times
  - 📊 Batch statistics

## Performance Tips
1. Adjust `-batch-size` based on your system's memory
2. Tune `-workers` based on your CPU cores
3. Lower `-jpeg-quality` for faster processing if needed
4. Use the default separate folder option for better organization

## Requirements
- Go 1.21 or later
- No external dependencies beyond the Go standard library and x/image

## Known Limitations
- Only processes JPG, JPEG, and PNG files
- RAM usage scales with batch size and number of workers
- Very large images might require adjusting batch size

## License
This project is open source and available under the MIT License.
