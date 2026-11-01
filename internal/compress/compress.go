package compress

import (
	"context"
	"errors"
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
)

const (
	defaultOut = "compressed"
	extPNG     = ".png"
	dirPerm    = 0o755
)

type Options struct {
	JPEGQuality int
	Prefix      string
	// Out is the output directory: "" means "<photo dir>/compressed", a relative
	// path is joined to each photo dir, an absolute path is used as is.
	Out       string
	Recursive bool
	Force     bool
}

type Job struct {
	In  string
	Out string
	// Size of the input, so the caller can report what the output saved.
	Size int64
}

// Plan lists the images under dir and where their compressed copy goes.
// Nothing is written; Job.Run does that. Skipped files are reported on the
// context's logger at debug level.
func Plan(ctx context.Context, fsys afero.Fs, dir string, o Options) ([]Job, error) {
	info, err := fsys.Stat(dir)
	if err != nil {
		return nil, fmt.Errorf("stat %s: %w", dir, err)
	}

	if !info.IsDir() {
		return nil, fmt.Errorf("%s is not a directory", dir)
	}

	p := planner{ctx: ctx, fsys: fsys, dir: dir, o: o, jobs: []Job{}}

	if err := p.scan(dir); err != nil {
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
	ctx  context.Context
	fsys afero.Fs
	dir  string
	o    Options
	jobs []Job
}

// scan lists dir once: its images become jobs and, under Recursive, its
// subdirectories are scanned in turn. One listing per directory and one per
// output directory is what keeps a network mount bearable.
func (p *planner) scan(dir string) error {
	entries, err := afero.ReadDir(p.fsys, dir)
	if err != nil {
		return fmt.Errorf("read %s: %w", dir, err)
	}

	existing, err := p.existing(dir)
	if err != nil {
		return err
	}

	for _, e := range entries {
		if err := p.visit(dir, e, existing); err != nil {
			return err
		}
	}

	return nil
}

func (p *planner) visit(dir string, e fs.FileInfo, existing map[string]bool) error {
	switch {
	case e.IsDir():
		if !p.o.Recursive || p.skipDir(e.Name()) {
			return nil
		}

		return p.scan(filepath.Join(dir, e.Name()))
	case e.Mode().IsRegular():
		p.add(dir, e, existing)
	default:
	}

	return nil
}

// existing names what the output directory already holds, so that checking
// each image costs a map lookup instead of a stat. A missing output directory
// holds nothing; Force makes the question moot.
func (p *planner) existing(photoDir string) (map[string]bool, error) {
	names := map[string]bool{}
	if p.o.Force {
		return names, nil
	}

	out := p.outDir(photoDir)

	entries, err := afero.ReadDir(p.fsys, out)
	if err != nil {
		if errors.Is(err, fs.ErrNotExist) {
			return names, nil
		}

		return nil, fmt.Errorf("read %s: %w", out, err)
	}

	for _, e := range entries {
		names[e.Name()] = true
	}

	return names, nil
}

// legacyOut is where the script that preceded wba wrote its bordered copies;
// those trees still exist and must not be bordered or compressed again.
const legacyOut = "bordered_images"

// skipDir keeps the walk out of hidden directories and of wba's own outputs,
// so a second run never compresses a bordered or compressed copy. RawTherapee's
// converted/ is entered on purpose: that is where the exports to shrink live.
func (p *planner) skipDir(name string) bool {
	return strings.HasPrefix(name, ".") || name == "bordered" || name == legacyOut || name == p.outBase()
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

func (p *planner) add(dir string, e fs.FileInfo, existing map[string]bool) {
	if !isImage(e.Name()) {
		return
	}

	name := p.o.Prefix + e.Name()
	path := filepath.Join(dir, e.Name())

	if existing[name] {
		p.report(path, "output exists")

		return
	}

	p.jobs = append(p.jobs, Job{In: path, Out: filepath.Join(p.outDir(dir), name), Size: e.Size()})
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
		enc := png.Encoder{CompressionLevel: png.BestCompression}

		return enc.Encode(w, img)
	}

	return jpeg.Encode(w, img, &jpeg.Options{Quality: quality})
}
