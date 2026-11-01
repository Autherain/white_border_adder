package clean

import (
	"context"
	"errors"
	"fmt"
	"os"
	"path"
	"path/filepath"
	"sort"
	"strings"

	"github.com/alitto/pond/v2"
	"github.com/evaneecloo/white_border_adder/internal/log"
	"github.com/spf13/afero"
)

const (
	convertedDir = "converted"
	sidecarExt   = ".pp3"
)

type Options struct {
	Pattern string
	// Exclude globs keep what they match; Only globs, when given, keep
	// everything they do not match. Both see the stem, each directory of the
	// path below the argument, and the whole path.
	Exclude   []string
	Only      []string
	Recursive bool
}

type Candidate struct {
	Path        string
	Size        int64
	Sidecar     string
	SidecarSize int64
	// Kept says why this candidate stays ("--exclude *marie*", "not in --only
	// *suisse*"), "" when it goes. Kept candidates stay in the plan so the
	// caller can say what was spared.
	Kept string
}

// A Skipped directory holds raws but no converted/, so no sorting happened
// there and clean leaves it alone; the raw count says what is waiting in it.
type Skipped struct {
	Path string
	Raws int
	Size int64
}

// Result is the plan: what would go or is kept, and what was not even looked at.
type Result struct {
	Candidates []Candidate
	Skipped    []Skipped
}

// Files lists the paths Delete would remove: the raw, then the sidecar when present.
func (c Candidate) Files() []string {
	files := []string{c.Path}
	if c.Sidecar != "" {
		files = append(files, c.Sidecar)
	}

	return files
}

func (c Candidate) Delete(fs afero.Fs) error {
	var errs []error

	for _, file := range c.Files() {
		if err := fs.Remove(file); err != nil {
			errs = append(errs, err)
		}
	}

	return errors.Join(errs...)
}

func Plan(ctx context.Context, fs afero.Fs, dir string, o Options) (Result, error) {
	info, err := fs.Stat(dir)
	if err != nil {
		return Result{}, err
	}

	if !info.IsDir() {
		return Result{}, fmt.Errorf("%s: not a directory", dir)
	}

	// path.Match only reports a malformed pattern when it reaches the bad part, so a
	// probe against an empty name validates every pattern before any listing happens.
	for _, glob := range append(append([]string{o.Pattern}, o.Exclude...), o.Only...) {
		if _, err := path.Match(glob, ""); err != nil {
			return Result{}, fmt.Errorf("%q: %w", glob, err)
		}
	}

	result, err := scanTree(ctx, fs, dir, o)
	if err != nil {
		return Result{}, err
	}

	candidates := result.Candidates

	for i := range candidates {
		candidates[i].Kept = kept(relative(dir, candidates[i].Path), o)
	}

	sort.Slice(candidates, func(i, j int) bool { return candidates[i].Path < candidates[j].Path })
	sort.Slice(result.Skipped, func(i, j int) bool { return result.Skipped[i].Path < result.Skipped[j].Path })

	return result, nil
}

// skipped names the directories a recursive walk never enters: hidden ones and
// RawTherapee's output, which holds no raws of its own.
func skipped(name string) bool {
	return strings.HasPrefix(name, ".") || name == convertedDir
}

// A slow mount (a cloud drive over 9p) makes every listing a round trip, so
// the directories of one depth are listed at the same time. Levels stay
// sequential: a task never waits on another, so a bounded pool cannot stall.
const listers = 16

type listed struct {
	dir        string
	candidates []Candidate
	subdirs    []string
	skipped    *Skipped
}

func scanTree(ctx context.Context, fs afero.Fs, root string, o Options) (Result, error) {
	pool := pond.NewResultPool[listed](listers)
	defer pool.StopAndWait()

	result := Result{Candidates: []Candidate{}, Skipped: []Skipped{}}
	level := []string{root}

	for len(level) > 0 {
		got, err := scanLevel(pool, fs, level, o)
		if err != nil {
			return Result{}, err
		}

		level = nil

		for _, l := range got {
			result.add(ctx, root, l)
			level = append(level, l.subdirs...)
		}
	}

	return result, nil
}

// add folds one listed directory into the result. Logged here rather than in
// the task so the lines keep the listing order.
func (r *Result) add(ctx context.Context, root string, l listed) {
	r.Candidates = append(r.Candidates, l.candidates...)

	if l.skipped == nil {
		return
	}

	log.Debugf(ctx, "skip %s: no %s/", relative(root, l.dir), convertedDir)

	// A directory without raws is just a container (the year folder, a
	// JPEG-only shoot); only one holding raws is worth a recap line.
	if l.skipped.Raws > 0 {
		r.Skipped = append(r.Skipped, *l.skipped)
	}
}

func scanLevel(pool pond.ResultPool[listed], fs afero.Fs, dirs []string, o Options) ([]listed, error) {
	tasks := make([]pond.ResultTask[listed], 0, len(dirs))
	for _, dir := range dirs {
		tasks = append(tasks, pool.SubmitErr(func() (listed, error) { return scan(fs, dir, o) }))
	}

	got := make([]listed, 0, len(dirs))

	for _, task := range tasks {
		l, err := task.Wait()
		if err != nil {
			return nil, err
		}

		got = append(got, l)
	}

	return got, nil
}

// scan lists one directory once and takes everything from that listing: the raws,
// their sidecars, whether converted/ exists and, when recursive, where to descend.
func scan(fs afero.Fs, dir string, o Options) (listed, error) {
	entries, err := afero.ReadDir(fs, dir)
	if err != nil {
		return listed{}, err
	}

	listing := map[string]os.FileInfo{}
	for _, entry := range entries {
		listing[entry.Name()] = entry
	}

	got := listed{dir: dir}

	if developed, ok := convertedNames(fs, dir, listing[convertedDir]); ok {
		got.candidates = candidatesIn(dir, entries, listing, developed, o)
	} else {
		got.skipped = waiting(dir, entries, o)
	}

	if !o.Recursive {
		return got, nil
	}

	for _, entry := range entries {
		if entry.IsDir() && !skipped(entry.Name()) {
			got.subdirs = append(got.subdirs, filepath.Join(dir, entry.Name()))
		}
	}

	return got, nil
}

// waiting counts the raws of a directory clean will not touch, so the recap
// can say what is sitting there unsorted.
func waiting(dir string, entries []os.FileInfo, o Options) *Skipped {
	skipped := &Skipped{Path: dir}

	for _, entry := range entries {
		if !entry.Mode().IsRegular() {
			continue
		}

		if ok, _ := path.Match(o.Pattern, strings.ToLower(entry.Name())); ok {
			skipped.Raws++
			skipped.Size += entry.Size()
		}
	}

	return skipped
}

func candidatesIn(dir string, entries []os.FileInfo, listing map[string]os.FileInfo, developed []string,
	o Options,
) []Candidate {
	candidates := []Candidate{}

	for _, entry := range entries {
		if candidate, ok := candidateFor(dir, entry, listing, developed, o); ok {
			candidates = append(candidates, candidate)
		}
	}

	return candidates
}

func candidateFor(dir string, entry os.FileInfo, listing map[string]os.FileInfo, developed []string,
	o Options,
) (Candidate, bool) {
	if !entry.Mode().IsRegular() {
		return Candidate{}, false
	}

	name := strings.ToLower(entry.Name())
	if ok, _ := path.Match(o.Pattern, name); !ok {
		return Candidate{}, false
	}

	stem := strings.TrimSuffix(name, path.Ext(name))
	if isDeveloped(stem, developed) {
		return Candidate{}, false
	}

	candidate := Candidate{Path: filepath.Join(dir, entry.Name()), Size: entry.Size()}

	if sidecar, ok := listing[entry.Name()+sidecarExt]; ok && sidecar.Mode().IsRegular() {
		candidate.Sidecar = candidate.Path + sidecarExt
		candidate.SidecarSize = sidecar.Size()
	}

	return candidate, true
}

// convertedNames lists the lowercased names of the regular files in converted/,
// given its entry from the parent listing; false means "not a photo directory".
func convertedNames(fs afero.Fs, dir string, converted os.FileInfo) ([]string, bool) {
	if converted == nil || !converted.IsDir() {
		return nil, false
	}

	entries, err := afero.ReadDir(fs, filepath.Join(dir, convertedDir))
	if err != nil {
		return nil, false
	}

	names := []string{}

	for _, entry := range entries {
		if entry.Mode().IsRegular() {
			names = append(names, strings.ToLower(entry.Name()))
		}
	}

	return names, true
}

func isDeveloped(stem string, converted []string) bool {
	for _, name := range converted {
		if strings.HasPrefix(name, stem) {
			return true
		}
	}

	return false
}

// kept is the reason a candidate stays: an --exclude glob it matches, or no
// --only glob matching it when --only is given. "" means it goes.
func kept(rel string, o Options) string {
	if glob := matching(rel, o.Exclude); glob != "" {
		return "--exclude " + glob
	}

	if len(o.Only) > 0 && matching(rel, o.Only) == "" {
		return "not in --only " + strings.Join(o.Only, " ")
	}

	return ""
}

// matching returns the first glob matching the raw's name without extension,
// a directory of its path below the argument, or that whole path, so
// "IMG_12*" targets a photo and "*marie*" targets a shoot.
func matching(rel string, globs []string) string {
	rel = strings.ToLower(filepath.ToSlash(rel))
	stem := strings.TrimSuffix(rel, path.Ext(rel))
	names := append(strings.Split(stem, "/"), stem)

	for _, glob := range globs {
		for _, name := range names {
			if ok, _ := path.Match(strings.ToLower(glob), name); ok {
				return glob
			}
		}
	}

	return ""
}

func relative(root, p string) string {
	rel, err := filepath.Rel(root, p)
	if err != nil {
		return p
	}

	return rel
}
