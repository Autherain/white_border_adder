package border

import (
	"context"
	"fmt"
	"image"
	"image/jpeg"
	"image/png"
	"io"
	"io/fs"
	"path/filepath"
	"sort"
	"strings"

	"github.com/evaneecloo/white_border_adder/internal/log"
	"github.com/spf13/afero"
	"golang.org/x/image/draw"
)

const (
	defaultOut = "bordered"
	extPNG     = ".png"
	dirPerm    = 0o755
	// A border ratio applies to both edges of an axis.
	sides = 2
)

type Options struct {
	Width, Height                 int
	LandscapeVert, LandscapeHoriz float64
	PortraitVert, PortraitHoriz   float64
	JPEGQuality                   int
	Prefix                        string
	// Out is the output directory: "" means "<photo dir>/bordered", a relative
	// path is joined to each photo dir, an absolute path is used as is.
	Out       string
	Recursive bool
	Force     bool
}

type Job struct {
	In  string
	Out string
}

// Plan lists the images under dir and where their bordered copy goes.
// Nothing is written; Job.Run does that. Skipped files are reported on the
// context logger at debug level.
func Plan(ctx context.Context, fsys afero.Fs, dir string, o Options) ([]Job, error) {
	info, err := fsys.Stat(dir)
	if err != nil {
		return nil, fmt.Errorf("stat %s: %w", dir, err)
	}

	if !info.IsDir() {
		return nil, fmt.Errorf("%s is not a directory", dir)
	}

	p := planner{ctx: ctx, fsys: fsys, dir: dir, o: o, jobs: []Job{}}

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

func (j Job) Run(fsys afero.Fs, o Options) error {
	img, err := decode(fsys, j.In)
	if err != nil {
		return err
	}

	canvas := render(img, o)

	if err := fsys.MkdirAll(filepath.Dir(j.Out), dirPerm); err != nil {
		return fmt.Errorf("create output directory: %w", err)
	}

	out, err := fsys.Create(j.Out)
	if err != nil {
		return fmt.Errorf("create output file: %w", err)
	}

	if err := encode(out, j.Out, canvas, o.JPEGQuality); err != nil {
		_ = out.Close()

		return fmt.Errorf("encode output image: %w", err)
	}

	return out.Close()
}

type planner struct {
	ctx  context.Context
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

		if err := p.add(filepath.Join(p.dir, e.Name())); err != nil {
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

	return p.add(path)
}

// skipDir keeps the walk out of hidden directories, RawTherapee's converted/
// and our own output directory, so bordered files are never bordered again.
func (p *planner) skipDir(name string) bool {
	return strings.HasPrefix(name, ".") || name == "converted" || name == p.outBase()
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

func (p *planner) add(path string) error {
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

	p.jobs = append(p.jobs, Job{In: path, Out: out})

	return nil
}

func (p *planner) report(path, reason string) {
	rel, err := filepath.Rel(p.dir, path)
	if err != nil {
		rel = path
	}

	log.Debugf(p.ctx, "skip %s: %s", rel, reason)
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
		return png.Encode(w, img)
	}

	return jpeg.Encode(w, img, &jpeg.Options{Quality: quality})
}

func render(img image.Image, o Options) *image.RGBA {
	bounds := img.Bounds()
	canvas := image.NewRGBA(image.Rect(0, 0, o.Width, o.Height))
	draw.Draw(canvas, canvas.Bounds(), image.White, image.Point{}, draw.Src)
	draw.ApproxBiLinear.Scale(canvas, layout(bounds.Dx(), bounds.Dy(), o), img, bounds, draw.Over, nil)

	return canvas
}

// layout fits an origW x origH image inside the canvas minus its borders and
// centers it, returning where the scaled image lands on the canvas. A square
// image counts as portrait, as in the original tool.
func layout(origW, origH int, o Options) image.Rectangle {
	vert, horiz := o.PortraitVert, o.PortraitHoriz
	if origW > origH {
		vert, horiz = o.LandscapeVert, o.LandscapeHoriz
	}

	availW := float64(o.Width) * (1 - sides*horiz)
	availH := float64(o.Height) * (1 - sides*vert)
	scale := min(availW/float64(origW), availH/float64(origH))

	scaledW := int(float64(origW) * scale)
	scaledH := int(float64(origH) * scale)
	offX := (o.Width - scaledW) / sides
	offY := (o.Height - scaledH) / sides

	return image.Rect(offX, offY, offX+scaledW, offY+scaledH)
}
