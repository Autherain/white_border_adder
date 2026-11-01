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
	Pattern   string
	Exclude   []string
	Recursive bool
}

type Candidate struct {
	Path        string
	Size        int64
	Sidecar     string
	SidecarSize int64
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

func Plan(ctx context.Context, fs afero.Fs, dir string, o Options) ([]Candidate, error) {
	info, err := fs.Stat(dir)
	if err != nil {
		return nil, err
	}

	if !info.IsDir() {
		return nil, fmt.Errorf("%s: not a directory", dir)
	}

	// path.Match only reports a malformed pattern when it reaches the bad part, so a
	// probe against an empty name validates every pattern before any listing happens.
	for _, glob := range append([]string{o.Pattern}, o.Exclude...) {
		if _, err := path.Match(glob, ""); err != nil {
			return nil, fmt.Errorf("%q: %w", glob, err)
		}
	}

	candidates, err := scanTree(ctx, fs, dir, o)
	if err != nil {
		return nil, err
	}

	sort.Slice(candidates, func(i, j int) bool { return candidates[i].Path < candidates[j].Path })

	return candidates, nil
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
	skipped    bool
}

func scanTree(ctx context.Context, fs afero.Fs, root string, o Options) ([]Candidate, error) {
	pool := pond.NewResultPool[listed](listers)
	defer pool.StopAndWait()

	candidates := []Candidate{}
	level := []string{root}

	for len(level) > 0 {
		got, err := scanLevel(pool, fs, level, o)
		if err != nil {
			return nil, err
		}

		level = nil

		for _, l := range got {
			// Logged here rather than in the task so the lines keep the listing order.
			if l.skipped {
				log.Debugf(ctx, "skip %s: no %s/", relative(root, l.dir), convertedDir)
			}

			candidates = append(candidates, l.candidates...)
			level = append(level, l.subdirs...)
		}
	}

	return candidates, nil
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
		got.skipped = true
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
	if isDeveloped(stem, developed) || excluded(stem, o.Exclude) {
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

func excluded(stem string, globs []string) bool {
	for _, glob := range globs {
		if ok, _ := path.Match(strings.ToLower(glob), stem); ok {
			return true
		}
	}

	return false
}

func relative(root, p string) string {
	rel, err := filepath.Rel(root, p)
	if err != nil {
		return p
	}

	return rel
}
