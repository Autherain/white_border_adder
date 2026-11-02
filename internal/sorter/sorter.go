package sorter

import (
	"context"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"regexp"
	"sort"
	"strings"
	"time"

	"github.com/alitto/pond/v2"
	"github.com/evaneecloo/white_border_adder/internal/glob"
	"github.com/evaneecloo/white_border_adder/internal/log"
	"github.com/spf13/afero"
)

type Options struct {
	Recursive bool
	// Exclude globs leave a photo, and any directory they match, untouched and
	// unread. They see the stem, each directory of the path below the root
	// and the whole path, as in clean.
	Exclude []string
}

// Move sends one photo to <root>/<year>/<day folder>/ with what belongs to it:
// its RawTherapee sidecar and its developed copies from converted/.
type Move struct {
	From, To string
	With     []Pair
	// Kept says why the photo stays where it is; kept moves stay in the plan so
	// the recap can show what was left alone.
	Kept string
}

type Pair struct {
	From, To string
}

// Pairs lists every file that moves, the photo first.
func (m Move) Pairs() []Pair {
	return append([]Pair{{m.From, m.To}}, m.With...)
}

const (
	sidecarExt   = ".pp3"
	convertedDir = "converted"
	// IO bound, not CPU bound: each task is one small read from a possibly
	// remote drive, so far more than NumCPU run at once.
	readers = 32
	dirPerm = 0o755
)

var (
	photoExts = map[string]bool{
		".rw2": true, ".jpg": true, ".jpeg": true, ".png": true,
		".dng": true, ".arw": true, ".cr2": true, ".cr3": true, ".nef": true, ".raf": true,
	}
	yearName = regexp.MustCompile(`^\d{4}$`)
)

// converted/ and bordered/ hold what was derived from the originals, not the
// originals: their dates say nothing and they follow their shoot, so they are
// never read and never moved. bordered_images is the old script's output.
func skippedDir(name string) bool {
	return strings.HasPrefix(name, ".") || name == "converted" || name == "bordered" || name == "bordered_images"
}

type photo struct {
	path    string
	sidecar bool
	// developed are the files of converted/ made from this photo.
	developed []string
}

type dated struct {
	photo
	when time.Time
}

// Plan files every photo found under sources (the root itself when none) under
// <root>/<year>/<day folder> by the date the camera recorded, falling back to
// the file's modification time. The day folder is, in order: the dated folder
// the photo already sits in when its date is right; the one existing under the
// year for that day, so the place name survives; else the bare date.
func Plan(ctx context.Context, fs afero.Fs, root string, sources []string, o Options) ([]Move, error) {
	for _, d := range append([]string{root}, sources...) {
		info, err := fs.Stat(d)
		if err != nil {
			return nil, err
		}

		if !info.IsDir() {
			return nil, fmt.Errorf("%s: not a directory", d)
		}
	}

	if len(sources) == 0 {
		sources = []string{root}

		// Run from inside 2026/ the year folders live one level up; without
		// this a 2025 photo would land in 2026/2025/.
		if abs, err := filepath.Abs(root); err == nil && yearName.MatchString(filepath.Base(abs)) {
			root = filepath.Join(root, "..")
		}
	}

	if err := glob.Validate(o.Exclude...); err != nil {
		return nil, err
	}

	photos, err := find(fs, sources, o)
	if err != nil {
		return nil, err
	}

	photos = leftOut(ctx, photos, root, o)

	p := planner{fs: fs, root: root, listed: map[string][]string{}, taken: map[string]string{}}

	moves := []Move{}
	for _, d := range readDates(fs, photos) {
		if m, ok := p.decide(d); ok {
			moves = append(moves, m)
		}
	}

	sort.Slice(moves, func(i, j int) bool { return moves[i].From < moves[j].From })

	return moves, nil
}

// leftOut drops the excluded photos before their dates are read: skipping the
// read is most of the point when the excluded tree is a large archive.
func leftOut(ctx context.Context, photos []photo, root string, o Options) []photo {
	if len(o.Exclude) == 0 {
		return photos
	}

	kept := photos[:0]

	for _, ph := range photos {
		rel, err := filepath.Rel(root, ph.path)
		if err != nil || strings.HasPrefix(rel, "..") {
			rel = ph.path
		}

		if g := glob.Matching(rel, o.Exclude); g != "" {
			log.Debugf(ctx, "skip %s: --exclude %s", rel, g)

			continue
		}

		kept = append(kept, ph)
	}

	return kept
}

func find(fs afero.Fs, sources []string, o Options) ([]photo, error) {
	pool := pond.NewResultPool[scanned](readers)
	defer pool.StopAndWait()

	seen := map[string]bool{}
	level := sources
	photos := []photo{}

	for len(level) > 0 {
		tasks := make([]pond.ResultTask[scanned], 0, len(level))
		for _, dir := range level {
			tasks = append(tasks, pool.SubmitErr(func() (scanned, error) { return scan(fs, dir, o) }))
		}

		level = nil

		for _, t := range tasks {
			s, err := t.Wait()
			if err != nil {
				return nil, err
			}

			for _, ph := range s.photos {
				if !seen[ph.path] {
					seen[ph.path] = true
					photos = append(photos, ph)
				}
			}

			level = append(level, s.subdirs...)
		}
	}

	return photos, nil
}

type scanned struct {
	photos  []photo
	subdirs []string
}

func scan(fs afero.Fs, dir string, o Options) (scanned, error) {
	entries, err := afero.ReadDir(fs, dir)
	if err != nil {
		return scanned{}, err
	}

	names := map[string]bool{}
	for _, e := range entries {
		names[e.Name()] = true
	}

	var converted []string
	if names[convertedDir] {
		converted = developedNames(fs, dir)
	}

	var got scanned

	for _, e := range entries {
		switch {
		case e.IsDir():
			// A directory excluded by its own name is not even listed.
			if o.Recursive && !skippedDir(e.Name()) && glob.Matching(e.Name(), o.Exclude) == "" {
				got.subdirs = append(got.subdirs, filepath.Join(dir, e.Name()))
			}
		case e.Mode().IsRegular() && photoExts[strings.ToLower(filepath.Ext(e.Name()))]:
			ph := photo{path: filepath.Join(dir, e.Name()), sidecar: names[e.Name()+sidecarExt]}
			ph.developed = developedFrom(e.Name(), converted)

			got.photos = append(got.photos, ph)
		}
	}

	return got, nil
}

func developedNames(fs afero.Fs, dir string) []string {
	entries, err := afero.ReadDir(fs, filepath.Join(dir, convertedDir))
	if err != nil {
		return nil
	}

	names := []string{}

	for _, e := range entries {
		if e.Mode().IsRegular() {
			names = append(names, e.Name())
		}
	}

	return names
}

// developedFrom picks the converted files named after the photo: IMG_1264.RW2
// has IMG_1264.jpg and IMG_1264-2.png but not IMG_12645.jpg, since the name
// that follows the stem must not continue it with a digit. Moving a wrong file
// is worse than missing one, hence stricter than clean's plain prefix.
func developedFrom(name string, converted []string) []string {
	stem := strings.ToLower(strings.TrimSuffix(name, filepath.Ext(name)))

	var out []string

	for _, c := range converted {
		low := strings.ToLower(c)
		if !strings.HasPrefix(low, stem) {
			continue
		}

		if rest := low[len(stem):]; rest != "" && rest[0] >= '0' && rest[0] <= '9' {
			continue
		}

		out = append(out, c)
	}

	return out
}

func readDates(fs afero.Fs, photos []photo) []dated {
	pool := pond.NewResultPool[dated](readers)
	defer pool.StopAndWait()

	tasks := make([]pond.ResultTask[dated], 0, len(photos))
	for _, ph := range photos {
		tasks = append(tasks, pool.Submit(func() dated { return dated{photo: ph, when: whenShot(fs, ph.path)} }))
	}

	out := make([]dated, 0, len(photos))

	for _, t := range tasks {
		d, _ := t.Wait()
		out = append(out, d)
	}

	return out
}

func whenShot(fs afero.Fs, path string) time.Time {
	if t, ok := shotDate(fs, path); ok {
		return t
	}

	info, err := fs.Stat(path)
	if err != nil {
		return time.Time{}
	}

	return info.ModTime()
}

type planner struct {
	fs   afero.Fs
	root string
	// listed caches the dated folders of each year directory; taken maps a
	// planned destination to its source so two cards with an IMG_0001.JPG do
	// not collide.
	listed map[string][]string
	taken  map[string]string
}

func (p *planner) decide(d dated) (Move, bool) {
	if d.when.IsZero() {
		return Move{From: d.path, Kept: "no date"}, true
	}

	day, iso, year := d.when.Format("20060102"), d.when.Format("2006-01-02"), d.when.Format("2006")
	dated := func(name string) bool { return strings.HasPrefix(name, day) || strings.HasPrefix(name, iso) }

	parts := p.below(filepath.Dir(d.path))

	if len(parts) >= 2 && parts[0] == year && dated(parts[1]) {
		return Move{}, false
	}

	name, sub := "", []string(nil)

	for i := 0; i < min(2, len(parts)); i++ {
		if dated(parts[i]) && (i == 0 || yearName.MatchString(parts[0])) {
			name, sub = parts[i], parts[i+1:]

			break
		}
	}

	if name == "" {
		var reason string

		if name, reason = p.dayFolder(year, day, dated); reason != "" {
			return Move{From: d.path, Kept: reason}, true
		}
	}

	dir := filepath.Join(append([]string{p.root, year, name}, sub...)...)
	if dir == filepath.Dir(d.path) {
		return Move{}, false
	}

	return p.target(d, dir), true
}

// below splits the directory into its parts under the root; a source outside
// the root has none, so its photos are treated as loose.
func (p *planner) below(dir string) []string {
	rel, err := filepath.Rel(p.root, dir)
	if err != nil || rel == "." || rel == ".." || strings.HasPrefix(rel, ".."+string(filepath.Separator)) {
		return nil
	}

	return strings.Split(rel, string(filepath.Separator))
}

func (p *planner) dayFolder(year, day string, dated func(string) bool) (string, string) {
	dir := filepath.Join(p.root, year)

	names, ok := p.listed[dir]
	if !ok {
		entries, _ := afero.ReadDir(p.fs, dir)
		for _, e := range entries {
			if e.IsDir() {
				names = append(names, e.Name())
			}
		}

		p.listed[dir] = names
	}

	var found []string

	for _, n := range names {
		if dated(n) {
			found = append(found, n)
		}
	}

	switch len(found) {
	case 0:
		return day, ""
	case 1:
		return found[0], ""
	default:
		return "", fmt.Sprintf("%d folders for %s in %s/: %s", len(found), day, year, strings.Join(found, ", "))
	}
}

func (p *planner) target(d dated, dir string) Move {
	m := Move{From: d.path, To: filepath.Join(dir, filepath.Base(d.path))}

	if d.sidecar {
		m.With = append(m.With, Pair{d.path + sidecarExt, m.To + sidecarExt})
	}

	for _, name := range d.developed {
		m.With = append(m.With, Pair{
			filepath.Join(filepath.Dir(d.path), convertedDir, name),
			filepath.Join(dir, convertedDir, name),
		})
	}

	for _, pair := range m.Pairs() {
		if other, clash := p.taken[pair.To]; clash {
			m.Kept = "same name as " + other

			return m
		}

		if exists(p.fs, pair.To) {
			m.Kept = "already exists in " + filepath.Base(dir)

			return m
		}
	}

	for _, pair := range m.Pairs() {
		p.taken[pair.To] = pair.From
	}

	return m
}

func exists(fs afero.Fs, path string) bool {
	_, err := fs.Stat(path)

	return !errors.Is(err, os.ErrNotExist)
}

// Run checks every target again right before moving: a rename onto an empty
// directory or a stale file must never replace something. The photo goes
// first, so a failure never leaves developed copies without their original.
func (m Move) Run(fs afero.Fs) error {
	for _, pair := range m.Pairs() {
		if exists(fs, pair.To) {
			return fmt.Errorf("%s: already exists", pair.To)
		}
	}

	var errs []error

	for i, pair := range m.Pairs() {
		err := fs.MkdirAll(filepath.Dir(pair.To), dirPerm)
		if err == nil {
			err = fs.Rename(pair.From, pair.To)
		}

		if err != nil {
			if i == 0 {
				return err
			}

			errs = append(errs, err)
		}
	}

	return errors.Join(errs...)
}
