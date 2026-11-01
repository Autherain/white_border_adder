package cli

import (
	"context"

	"github.com/evaneecloo/white_border_adder/internal/log"
)

const (
	verbDelete = "delete"
	verbWrite  = "write"
)

// An item is one planned action: the files it touches, relative to the
// argument, the bytes involved, and a warning when the file deserves a
// second look before --apply.
type item struct {
	dir   string // photo directory, relative to the argument; groups the banner
	files []string
	size  int64
	warn  string
}

// show prints the plan and returns the indexes of the items to run: all of
// them with --apply, none otherwise, since showing the plan is then the job.
func (a *app) show(ctx context.Context, verb string, items []item) []int {
	if a.apply {
		return everything(len(items))
	}

	groups := byDirectory(items)

	for _, g := range groups {
		// One directory needs no header; several get a banner each so the
		// eye finds where a folder starts in a long list.
		if len(groups) > 1 {
			log.Infof(ctx, "%s", styleBold.Render(log.Banner+" "+g.dir+": "+summary(g.items)))
		}

		for _, it := range g.items {
			showItem(ctx, verb, it)
		}
	}

	log.Infof(ctx, "%s Re-run with --apply to %s them.", styleBold.Render(summary(items)+"."), verb)

	return nil
}

func showItem(ctx context.Context, verb string, it item) {
	for i, f := range it.files {
		if i == 0 && it.warn != "" {
			log.Infof(ctx, "%s %s  %s",
				styleWould.Render("would "+verb), f, styleGone.Render(log.Banner+" "+it.warn))

			continue
		}

		log.Infof(ctx, "%s %s", styleWould.Render("would "+verb), f)
	}
}

func everything(n int) []int {
	idx := make([]int, n)
	for i := range idx {
		idx[i] = i
	}

	return idx
}

func pickFrom[T any](all []T, idx []int) []T {
	out := make([]T, 0, len(idx))
	for _, i := range idx {
		out = append(out, all[i])
	}

	return out
}

// summary is "3 files, 49 MB", or just the count when sizes are unknown.
func summary(items []item) string {
	n, total := 0, int64(0)

	for _, it := range items {
		n += len(it.files)
		total += it.size
	}

	if total == 0 {
		return files(n)
	}

	return files(n) + ", " + human(total)
}

type group struct {
	dir   string
	items []item
}

// byDirectory splits the items by directory, in order of appearance.
func byDirectory(items []item) []group {
	groups := []group{}
	index := map[string]int{}

	for _, it := range items {
		i, seen := index[it.dir]
		if !seen {
			i = len(groups)
			index[it.dir] = i
			groups = append(groups, group{dir: it.dir})
		}

		groups[i].items = append(groups[i].items, it)
	}

	return groups
}
