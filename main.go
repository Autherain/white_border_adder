package main

import (
	"flag"
	"fmt"
	"image"
	"image/jpeg"
	"image/png"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"time"

	"golang.org/x/image/draw"
)

type imageJob struct {
	inputPath  string
	outputPath string
}

type processingResult struct {
	filename string
	duration time.Duration
	error    error
}

type batchResult struct {
	batchID   int
	startTime time.Time
	endTime   time.Time
	results   []processingResult
}

type processingStats struct {
	sync.Mutex
	totalImages   int
	failedImages  int
	totalDuration time.Duration
	batchResults  []batchResult
	fastest       processingResult
	slowest       processingResult
}

type Config struct {
	targetWidth          int
	targetHeight         int
	landscapeVertBorder  float64
	landscapeHorizBorder float64
	portraitVertBorder   float64
	portraitHorizBorder  float64
	batchSize            int
	maxWorkers           int
	jpegQuality          int
	outputPrefix         string
	createSeparateFolder bool
}

// Default configuration values
var defaultConfig = Config{
	targetWidth:          1080,
	targetHeight:         1080,
	landscapeVertBorder:  0.05,
	landscapeHorizBorder: 0.03,
	portraitVertBorder:   0.005,
	portraitHorizBorder:  0.18,
	batchSize:            10,
	maxWorkers:           1000,
	jpegQuality:          100,
	outputPrefix:         "bordered_",
	createSeparateFolder: true,
}

func parseFlags() (*Config, string) {
	// Create a new FlagSet to track if flags were actually set
	flagSet := flag.NewFlagSet(os.Args[0], flag.ExitOnError)

	// Create config with default values
	config := defaultConfig

	// Define flags but don't use them directly
	var (
		width          = flagSet.Int("width", defaultConfig.targetWidth, "Target width for output images")
		height         = flagSet.Int("height", defaultConfig.targetHeight, "Target height for output images")
		landscapeVert  = flagSet.Float64("landscape-vert", defaultConfig.landscapeVertBorder, "Vertical border ratio for landscape images")
		landscapeHoriz = flagSet.Float64("landscape-horiz", defaultConfig.landscapeHorizBorder, "Horizontal border ratio for landscape images")
		portraitVert   = flagSet.Float64("portrait-vert", defaultConfig.portraitVertBorder, "Vertical border ratio for portrait images")
		portraitHoriz  = flagSet.Float64("portrait-horiz", defaultConfig.portraitHorizBorder, "Horizontal border ratio for portrait images")
		batchSize      = flagSet.Int("batch-size", defaultConfig.batchSize, "Number of images to process in each batch")
		workers        = flagSet.Int("workers", defaultConfig.maxWorkers, "Maximum number of concurrent workers")
		jpegQuality    = flagSet.Int("jpeg-quality", defaultConfig.jpegQuality, "JPEG output quality (1-100)")
		outputPrefix   = flagSet.String("prefix", defaultConfig.outputPrefix, "Prefix for output filenames")
		separateFolder = flagSet.Bool("separate-folder", defaultConfig.createSeparateFolder, "Create separate folder for output")
		inputFolder    = flagSet.String("input", "", "Input folder containing images (required)")
	)

	// If only one argument is provided (the input folder), use it directly with default config
	if len(os.Args) == 2 && !strings.HasPrefix(os.Args[1], "-") {
		return &defaultConfig, os.Args[1]
	}

	// Parse flags normally if more arguments are provided
	if err := flagSet.Parse(os.Args[1:]); err != nil {
		fmt.Println("Error parsing flags:", err)
		flagSet.Usage()
		os.Exit(1)
	}

	// Check if input folder is provided
	if *inputFolder == "" && flagSet.NArg() > 0 {
		*inputFolder = flagSet.Arg(0)
	}

	if *inputFolder == "" {
		fmt.Println("Error: Input folder is required")
		flagSet.Usage()
		os.Exit(1)
	}

	// Check which flags were explicitly set and only update those values
	flagSet.Visit(func(f *flag.Flag) {
		switch f.Name {
		case "width":
			config.targetWidth = *width
		case "height":
			config.targetHeight = *height
		case "landscape-vert":
			config.landscapeVertBorder = *landscapeVert
		case "landscape-horiz":
			config.landscapeHorizBorder = *landscapeHoriz
		case "portrait-vert":
			config.portraitVertBorder = *portraitVert
		case "portrait-horiz":
			config.portraitHorizBorder = *portraitHoriz
		case "batch-size":
			config.batchSize = *batchSize
		case "workers":
			config.maxWorkers = *workers
		case "jpeg-quality":
			config.jpegQuality = *jpegQuality
		case "prefix":
			config.outputPrefix = *outputPrefix
		case "separate-folder":
			config.createSeparateFolder = *separateFolder
		}
	})

	return &config, *inputFolder
}

func printConfig(config *Config, usingDefaults bool) {
	fmt.Println("\n=== Configuration ===")
	if usingDefaults {
		fmt.Println("Using default configuration (no flags provided)")
	}
	fmt.Printf("Target dimensions: %dx%d\n", config.targetWidth, config.targetHeight)
	fmt.Printf("Landscape borders: Vertical=%.1f%%, Horizontal=%.1f%%\n",
		config.landscapeVertBorder*100, config.landscapeHorizBorder*100)
	fmt.Printf("Portrait borders: Vertical=%.1f%%, Horizontal=%.1f%%\n",
		config.portraitVertBorder*100, config.portraitHorizBorder*100)
	fmt.Printf("Batch size: %d\n", config.batchSize)
	fmt.Printf("Max workers: %d\n", config.maxWorkers)
	fmt.Printf("JPEG quality: %d\n", config.jpegQuality)
	fmt.Printf("Output prefix: %s\n", config.outputPrefix)
	fmt.Printf("Separate output folder: %v\n", config.createSeparateFolder)
	fmt.Println("==================\n")
}

func (ps *processingStats) addResult(br batchResult) {
	ps.Lock()
	defer ps.Unlock()

	ps.batchResults = append(ps.batchResults, br)

	for _, result := range br.results {
		if result.error != nil {
			ps.failedImages++
			continue
		}

		ps.totalImages++
		ps.totalDuration += result.duration

		if ps.fastest.duration == 0 || result.duration < ps.fastest.duration {
			ps.fastest = result
		}

		if result.duration > ps.slowest.duration {
			ps.slowest = result
		}
	}
}

func (ps *processingStats) printSummary() {
	ps.Lock()
	defer ps.Unlock()

	fmt.Printf("\n📊 === Processing Summary ===\n")
	fmt.Printf("✅ Total images processed: %d\n", ps.totalImages)
	fmt.Printf("❌ Failed images: %d\n", ps.failedImages)

	if ps.totalImages > 0 {
		avgDuration := ps.totalDuration / time.Duration(ps.totalImages)
		fmt.Printf("⏱️  Average processing time: %.2f seconds\n", avgDuration.Seconds())
		fmt.Printf("🚀 Fastest image: %s (%.2f seconds)\n", ps.fastest.filename, ps.fastest.duration.Seconds())
		fmt.Printf("🐢 Slowest image: %s (%.2f seconds)\n", ps.slowest.filename, ps.slowest.duration.Seconds())
	}

	fmt.Printf("\n📈 Batch Statistics:\n")
	for _, batch := range ps.batchResults {
		batchDuration := batch.endTime.Sub(batch.startTime)
		successCount := 0
		for _, result := range batch.results {
			if result.error == nil {
				successCount++
			}
		}
		fmt.Printf("📦 Batch %d: %d/%d successful, took %.2f seconds\n",
			batch.batchID, successCount, len(batch.results), batchDuration.Seconds())
	}
}

func main() {
	// Determine if we're using default configuration
	usingDefaults := len(os.Args) == 2 && !strings.HasPrefix(os.Args[1], "-")

	config, inputFolder := parseFlags()
	printConfig(config, usingDefaults)

	mainStart := time.Now()

	var outputFolder string
	if config.createSeparateFolder {
		outputFolder = filepath.Join(inputFolder, "bordered_images")
	} else {
		outputFolder = inputFolder
	}

	if config.createSeparateFolder {
		if err := os.MkdirAll(outputFolder, 0755); err != nil {
			fmt.Printf("Error creating output folder: %v\n", err)
			return
		}
	}

	files, err := os.ReadDir(inputFolder)
	if err != nil {
		fmt.Printf("Error reading directory: %v\n", err)
		return
	}

	jobs := make(chan []imageJob, config.maxWorkers)
	results := make(chan batchResult, len(files)/config.batchSize+1)
	var wg sync.WaitGroup

	stats := &processingStats{}

	for i := 0; i < config.maxWorkers; i++ {
		wg.Add(1)
		go worker(i, jobs, results, &wg, config)
	}

	var batch []imageJob
	batchCount := 0
	totalImages := 0

	for _, file := range files {
		if file.IsDir() {
			continue
		}
		filename := file.Name()
		ext := strings.ToLower(filepath.Ext(filename))
		if ext != ".jpg" && ext != ".jpeg" && ext != ".png" {
			continue
		}

		inputPath := filepath.Join(inputFolder, filename)
		outputPath := filepath.Join(outputFolder, fmt.Sprintf("%s%s", config.outputPrefix, filename))

		batch = append(batch, imageJob{inputPath, outputPath})
		totalImages++

		if len(batch) == config.batchSize || totalImages == len(files) {
			if len(batch) > 0 {
				jobs <- batch
				batchCount++
				batch = make([]imageJob, 0, config.batchSize)
			}
		}
	}

	close(jobs)

	go func() {
		wg.Wait()
		close(results)
	}()

	for result := range results {
		stats.addResult(result)
	}

	mainDuration := time.Since(mainStart)
	fmt.Printf("\nTotal execution time: %.2f seconds\n", mainDuration.Seconds())
	stats.printSummary()
}

func worker(id int, jobs <-chan []imageJob, results chan<- batchResult, wg *sync.WaitGroup, config *Config) {
	defer wg.Done()

	for batch := range jobs {
		batchStart := time.Now()
		br := batchResult{
			batchID:   id,
			startTime: batchStart,
		}

		for _, job := range batch {
			start := time.Now()
			err := processImage(job.inputPath, job.outputPath, config)
			duration := time.Since(start)

			result := processingResult{
				filename: filepath.Base(job.inputPath),
				duration: duration,
				error:    err,
			}

			br.results = append(br.results, result)

			if err != nil {
				fmt.Printf("❌ Error processing %s: %v\n", filepath.Base(job.inputPath), err)
			} else {
				fmt.Printf("✅ Successfully processed %s in %.2f seconds\n",
					filepath.Base(job.inputPath), duration.Seconds())
			}
		}

		br.endTime = time.Now()
		results <- br
	}
}

func processImage(inputPath, outputPath string, config *Config) error {
	input, err := os.Open(inputPath)
	if err != nil {
		return fmt.Errorf("error opening input file: %v", err)
	}
	defer input.Close()

	var img image.Image
	switch strings.ToLower(filepath.Ext(inputPath)) {
	case ".jpg", ".jpeg":
		img, err = jpeg.Decode(input)
	case ".png":
		img, err = png.Decode(input)
	default:
		return fmt.Errorf("unsupported image format")
	}
	if err != nil {
		return fmt.Errorf("error decoding image: %v", err)
	}

	bounds := img.Bounds()
	origWidth := bounds.Dx()
	origHeight := bounds.Dy()
	isLandscape := origWidth > origHeight

	verticalBorderRatio := config.landscapeVertBorder
	horizontalBorderRatio := config.landscapeHorizBorder
	if !isLandscape {
		verticalBorderRatio = config.portraitVertBorder
		horizontalBorderRatio = config.portraitHorizBorder
	}

	availableWidth := float64(config.targetWidth) * (1 - 2*horizontalBorderRatio)
	availableHeight := float64(config.targetHeight) * (1 - 2*verticalBorderRatio)

	scale := min(
		availableWidth/float64(origWidth),
		availableHeight/float64(origHeight),
	)

	scaledWidth := int(float64(origWidth) * scale)
	scaledHeight := int(float64(origHeight) * scale)

	// Create the white background image
	newImg := image.NewRGBA(image.Rect(0, 0, config.targetWidth, config.targetHeight))
	draw.Draw(newImg, newImg.Bounds(), image.White, image.Point{}, draw.Src)

	// Calculate the position to place the scaled image
	offsetX := (config.targetWidth - scaledWidth) / 2
	offsetY := (config.targetHeight - scaledHeight) / 2

	// Create a rectangle for the destination area
	destRect := image.Rect(offsetX, offsetY, offsetX+scaledWidth, offsetY+scaledHeight)

	// Scale and draw the image in one step using draw.ApproxBiLinear
	draw.ApproxBiLinear.Scale(newImg, destRect, img, img.Bounds(), draw.Over, nil)

	output, err := os.Create(outputPath)
	if err != nil {
		return fmt.Errorf("error creating output file: %v", err)
	}
	defer output.Close()

	if strings.ToLower(filepath.Ext(outputPath)) == ".png" {
		err = png.Encode(output, newImg)
	} else {
		err = jpeg.Encode(output, newImg, &jpeg.Options{Quality: config.jpegQuality})
	}
	if err != nil {
		return fmt.Errorf("error encoding output image: %v", err)
	}

	return nil
}

func drawImage(dst *image.RGBA, src *image.RGBA, offset image.Point) {
	bounds := src.Bounds()
	for y := bounds.Min.Y; y < bounds.Max.Y; y++ {
		for x := bounds.Min.X; x < bounds.Max.X; x++ {
			dst.Set(x+offset.X, y+offset.Y, src.At(x, y))
		}
	}
}

func min(a, b float64) float64 {
	if a < b {
		return a
	}
	return b
}
