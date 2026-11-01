package compress

import (
	"fmt"
	"image"
	"image/jpeg"
	"image/png"
	"io"
	"io/fs"
	"path/filepath"
	"sort"
	"strings"

	"github.com/spf13/afero"
	"golang.org/x/image/draw"
)

const (
	defaultOut = "compressed"
	extPNG     = ".png"
	dirPerm    = 0o755
)

type Options struct {
	JPEGQuality int
	// MaxWidth and MaxHeight bound the output; 0 leaves that side unbounded.
	// An image already inside the bounds is re-encoded at its own size.
	MaxWidth, MaxHeight int
	Prefix              string
	// Out is the output directory: "" means "<photo dir>/compressed", a relative
	// path is joined to each photo dir, an absolute path is used as is.
	Out       string
	Recursive bool
	Force     bool
	// Verbose receives one "skip <path>: <reason>" line per skipped file; nil is quiet.
	Verbose io.Writer
}

type Job struct {
	In  string
	Out string
	// Size of the input, so the caller can report what the output saved.
	Size int64
}

// Plan lists the images under dir and where their compressed copy goes.
// Nothing is written; Job.Run does that.
func Plan(fsys afero.Fs, dir string, o Options) ([]Job, error) {
	info, err := fsys.Stat(dir)
	if err != nil {
		return nil, fmt.Errorf("stat %s: %w", dir, err)
	}

	if !info.IsDir() {
		return nil, fmt.Errorf("%s is not a directory", dir)
	}

	p := planner{fsys: fsys, dir: dir, o: o, jobs: []Job{}}

	if o.Recursive {
		err = afero.Walk(fsys, dir, p.walk)
	} else {
		err = p.readDir()
	}

	if err != nil {
		return nil, err
	}

	sort.Slice(p.jobs, func(i, j int) bool { return p.jobs[i].In < p.jobs[j].In })

	return p.jobs, nil
}

// Run writes the compressed copy and returns its size in bytes.
func (j Job) Run(fsys afero.Fs, o Options) (int64, error) {
	img, err := decode(fsys, j.In)
	if err != nil {
		return 0, err
	}

	img = shrink(img, o)

	if err := fsys.MkdirAll(filepath.Dir(j.Out), dirPerm); err != nil {
		return 0, fmt.Errorf("create output directory: %w", err)
	}

	out, err := fsys.Create(j.Out)
	if err != nil {
		return 0, fmt.Errorf("create output file: %w", err)
	}

	if err := encode(out, j.Out, img, o.JPEGQuality); err != nil {
		_ = out.Close()

		return 0, fmt.Errorf("encode output image: %w", err)
	}

	if err := out.Close(); err != nil {
		return 0, fmt.Errorf("close output file: %w", err)
	}

	info, err := fsys.Stat(j.Out)
	if err != nil {
		return 0, fmt.Errorf("stat output file: %w", err)
	}

	return info.Size(), nil
}

type planner struct {
	fsys afero.Fs
	dir  string
	o    Options
	jobs []Job
}

func (p *planner) readDir() error {
	entries, err := afero.ReadDir(p.fsys, p.dir)
	if err != nil {
		return fmt.Errorf("read %s: %w", p.dir, err)
	}

	for _, e := range entries {
		if !e.Mode().IsRegular() {
			continue
		}

		if err := p.add(filepath.Join(p.dir, e.Name()), e.Size()); err != nil {
			return err
		}
	}

	return nil
}

func (p *planner) walk(path string, info fs.FileInfo, err error) error {
	if err != nil {
		return err
	}

	if info.IsDir() {
		if path != p.dir && p.skipDir(info.Name()) {
			return filepath.SkipDir
		}

		return nil
	}

	if !info.Mode().IsRegular() {
		return nil
	}

	return p.add(path, info.Size())
}

// skipDir keeps the walk out of hidden directories, RawTherapee's converted/,
// the border verb's output and our own, so a copy is never compressed again.
func (p *planner) skipDir(name string) bool {
	return strings.HasPrefix(name, ".") || name == "converted" || name == "bordered" || name == p.outBase()
}

func (p *planner) outBase() string {
	if p.o.Out == "" {
		return defaultOut
	}

	return filepath.Base(p.o.Out)
}

func (p *planner) outDir(photoDir string) string {
	switch {
	case p.o.Out == "":
		return filepath.Join(photoDir, defaultOut)
	case filepath.IsAbs(p.o.Out):
		return p.o.Out
	default:
		return filepath.Join(photoDir, p.o.Out)
	}
}

func (p *planner) add(path string, size int64) error {
	if !isImage(path) {
		return nil
	}

	out := filepath.Join(p.outDir(filepath.Dir(path)), p.o.Prefix+filepath.Base(path))

	if !p.o.Force {
		exists, err := afero.Exists(p.fsys, out)
		if err != nil {
			return fmt.Errorf("stat %s: %w", out, err)
		}

		if exists {
			p.report(path, "output exists")

			return nil
		}
	}

	p.jobs = append(p.jobs, Job{In: path, Out: out, Size: size})

	return nil
}

func (p *planner) report(path, reason string) {
	if p.o.Verbose == nil {
		return
	}

	rel, err := filepath.Rel(p.dir, path)
	if err != nil {
		rel = path
	}

	_, _ = fmt.Fprintf(p.o.Verbose, "skip %s: %s\n", rel, reason)
}

func isImage(path string) bool {
	switch strings.ToLower(filepath.Ext(path)) {
	case ".jpg", ".jpeg", extPNG:
		return true
	default:
		return false
	}
}

func decode(fsys afero.Fs, path string) (image.Image, error) {
	in, err := fsys.Open(path)
	if err != nil {
		return nil, fmt.Errorf("open input file: %w", err)
	}

	defer func() { _ = in.Close() }()

	var img image.Image

	switch strings.ToLower(filepath.Ext(path)) {
	case ".jpg", ".jpeg":
		img, err = jpeg.Decode(in)
	case extPNG:
		img, err = png.Decode(in)
	default:
		return nil, fmt.Errorf("unsupported image format %q", filepath.Ext(path))
	}

	if err != nil {
		return nil, fmt.Errorf("decode image: %w", err)
	}

	return img, nil
}

func encode(w io.Writer, path string, img image.Image, quality int) error {
	if strings.ToLower(filepath.Ext(path)) == extPNG {
		enc := png.Encoder{CompressionLevel: png.BestCompression}

		return enc.Encode(w, img)
	}

	return jpeg.Encode(w, img, &jpeg.Options{Quality: quality})
}

// shrink returns img itself when it already fits, so an unchanged image is
// not resampled and loses nothing but encoder precision.
func shrink(img image.Image, o Options) image.Image {
	bounds := img.Bounds()

	w, h := fit(bounds.Dx(), bounds.Dy(), o.MaxWidth, o.MaxHeight)
	if w == bounds.Dx() && h == bounds.Dy() {
		return img
	}

	dst := image.NewRGBA(image.Rect(0, 0, w, h))
	draw.ApproxBiLinear.Scale(dst, dst.Bounds(), img, bounds, draw.Src, nil)

	return dst
}

// fit scales w x h down to sit inside maxW x maxH, keeping the ratio and
// never growing it; 0 on a side means that side is unbounded. Integer
// arithmetic keeps a bound exact where a float scale would round it away.
func fit(w, h, maxW, maxH int) (fitW, fitH int) {
	fitW, fitH = w, h

	if maxW > 0 && fitW > maxW {
		fitH = fitH * maxW / fitW
		fitW = maxW
	}

	if maxH > 0 && fitH > maxH {
		fitW = fitW * maxH / fitH
		fitH = maxH
	}

	return fitW, fitH
}
