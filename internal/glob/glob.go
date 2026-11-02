package glob

import (
	"fmt"
	"path"
	"path/filepath"
	"strings"
)

// Validate reports the first malformed glob. path.Match only complains when it
// reaches the bad part, so matching against an empty name checks them all
// before any listing happens.
func Validate(globs ...string) error {
	for _, g := range globs {
		if _, err := path.Match(g, ""); err != nil {
			return fmt.Errorf("%q: %w", g, err)
		}
	}

	return nil
}

// Matching returns the first glob matching a photo's name without extension, a
// directory of its path, or that whole path, so "IMG_12*" targets a photo and
// "*marie*" targets a shoot. Case does not matter.
func Matching(rel string, globs []string) string {
	rel = strings.ToLower(filepath.ToSlash(rel))
	stem := strings.TrimSuffix(rel, path.Ext(rel))
	names := append(strings.Split(stem, "/"), stem)

	for _, g := range globs {
		for _, name := range names {
			if ok, _ := path.Match(strings.ToLower(g), name); ok {
				return g
			}
		}
	}

	return ""
}
