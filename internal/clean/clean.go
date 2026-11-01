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

	if !o.Recursive {
		return scan(ctx, fs, dir, dir, o)
	}

	return walk(ctx, fs, dir, o)
}

func walk(ctx context.Context, fs afero.Fs, dir string, o Options) ([]Candidate, error) {
	candidates := []Candidate{}

	err := afero.Walk(fs, dir, func(p string, info os.FileInfo, err error) error {
		if err != nil {
			return err
		}

		if !info.IsDir() {
			return nil
		}

		if p != dir && skipped(info.Name()) {
			return filepath.SkipDir
		}

		found, err := scan(ctx, fs, dir, p, o)
		if err != nil {
			return err
		}

		candidates = append(candidates, found...)

		return nil
	})
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

// scan lists the candidates at the root of one directory, root being the user's argument
// used only to print paths relative to it.
func scan(ctx context.Context, fs afero.Fs, root, dir string, o Options) ([]Candidate, error) {
	developed, ok := convertedNames(fs, filepath.Join(dir, convertedDir))
	if !ok {
		log.Debugf(ctx, "skip %s: no %s/", relative(root, dir), convertedDir)

		return nil, nil
	}

	entries, err := afero.ReadDir(fs, dir)
	if err != nil {
		return nil, err
	}

	candidates := []Candidate{}

	for _, entry := range entries {
		if candidate, ok := candidateFor(fs, dir, entry, developed, o); ok {
			candidates = append(candidates, candidate)
		}
	}

	return candidates, nil
}

func candidateFor(fs afero.Fs, dir string, entry os.FileInfo, developed []string, o Options) (Candidate, bool) {
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

	if sidecar, err := fs.Stat(candidate.Path + sidecarExt); err == nil && sidecar.Mode().IsRegular() {
		candidate.Sidecar = candidate.Path + sidecarExt
		candidate.SidecarSize = sidecar.Size()
	}

	return candidate, true
}

// convertedNames lists the lowercased names of the regular files in converted/;
// false means "this is not a photo directory".
func convertedNames(fs afero.Fs, converted string) ([]string, bool) {
	info, err := fs.Stat(converted)
	if err != nil || !info.IsDir() {
		return nil, false
	}

	entries, err := afero.ReadDir(fs, converted)
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
