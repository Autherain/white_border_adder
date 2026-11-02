package cli

import (
	"context"
	"fmt"
	"slices"
	"strings"

	"github.com/evaneecloo/white_border_adder/internal/log"
)

const (
	verbDelete = "delete"
	verbWrite  = "write"
	verbMove   = "move"
)

// An item is one planned action: the files it touches, relative to the
// argument, the bytes involved, a warning when the file deserves a second
// look before --apply, and the reason it is kept out of the run, if any.
type item struct {
	dir   string // photo directory, relative to the argument; groups the banner
	files []string
	size  int64
	warn  string
	kept  string // "" runs; otherwise the --exclude glob that keeps it
}

// A skipped directory is one the verb refused to look into; the recap names
// it so the reader knows the list is not the whole tree.
type skipped struct {
	dir    string
	detail string
}

// show prints the plan and returns the indexes of the items to run: the ones
// not kept with --apply, none otherwise, since showing the plan is then the job.
func (a *app) show(ctx context.Context, verb string, items []item, skips []skipped) []int {
	if a.apply {
		return toRun(items)
	}

	groups := byDirectory(items)

	for _, g := range groups {
		run := running(g.items)
		if len(run) == 0 {
			continue
		}

		// One directory needs no header; several get a banner each so the
		// eye finds where a folder starts in a long list.
		if len(groups) > 1 {
			log.Infof(ctx, "%s", styleBold.Render(log.Banner+" "+g.dir+": "+summary(run)))
		}

		for _, it := range run {
			showItem(ctx, verb, it)
		}
	}

	for _, line := range recap(verb, items, skips, planned) {
		log.Infof(ctx, "%s", line)
	}

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

// A stage says whether the recap describes a plan or what just ran, which
// only changes the tense of its verbs.
type stage int

const (
	planned stage = iota
	finished
)

// recap is the block a reader checks before and after --apply: per directory
// what runs, what is kept and what was skipped, how many files deserve a
// look, then the totals.
func recap(verb string, items []item, skips []skipped, at stage) []string {
	groups := byDirectory(items)
	run, kept := running(items), keptOf(items)

	lines := []string{}
	if len(groups) > 1 || len(kept) > 0 || len(skips) > 0 {
		lines = append(lines, styleBold.Render(log.Banner+" recap"))
		lines = append(lines, recapGroups(verb, groups, skips, at)...)
	}

	if flagged := filter(run, func(it item) bool { return it.warn != "" }); len(flagged) > 0 {
		lines = append(lines, styleGone.Render(fmt.Sprintf("%s %s flagged: %s",
			log.Banner, files(len(flagged)), flagged[0].warn)))
	}

	total := summary(run)
	if at == finished {
		total = ran(verb, run)
	}

	if len(kept) > 0 {
		total += ", " + summary(kept) + " kept"
	}

	// Nothing to run means no hint either: --apply would do nothing.
	if len(run) == 0 {
		return append(lines, styleBold.Render("nothing to do"+strings.TrimPrefix(total, summary(run))+"."))
	}

	if at == finished {
		return append(lines, styleBold.Render(total+"."))
	}

	return append(lines, fmt.Sprintf("%s Re-run with --apply to %s them.", styleBold.Render(total+"."), verb))
}

// ran words the total once the verb ran: "3 files deleted, 49 MB freed".
func ran(verb string, run []item) string {
	n, total := 0, int64(0)

	for _, it := range run {
		n += len(it.files)
		total += it.size
	}

	switch verb {
	case verbDelete:
		return files(n) + " deleted, " + human(total) + " freed"
	case verbMove:
		return files(n) + " moved"
	default:
		return files(n) + " written"
	}
}

func recapGroups(verb string, groups []group, skips []skipped, at stage) []string {
	width := 0
	for _, g := range groups {
		width = max(width, len(g.dir))
	}

	for _, sk := range skips {
		width = max(width, len(sk.dir))
	}

	action := verb
	if at == finished {
		action = past(verb)
	}

	lines := []string{}

	for _, g := range groups {
		if run := running(g.items); len(run) > 0 {
			lines = append(lines, fmt.Sprintf("%-7s %-*s  %s", action, width, g.dir, summary(run)))
		}

		if kept := keptOf(g.items); len(kept) > 0 {
			lines = append(lines, styleFaint.Render(fmt.Sprintf("%-7s %-*s  %s  %s",
				"keep", width, g.dir, summary(kept), strings.Join(reasons(kept), ", "))))
		}
	}

	for _, sk := range skips {
		lines = append(lines, styleFaint.Render(fmt.Sprintf("%-7s %-*s  %s", "skip", width, sk.dir, sk.detail)))
	}

	return lines
}

func past(verb string) string {
	if verb == verbWrite {
		return "wrote"
	}

	return verb + "d"
}

func toRun(items []item) []int {
	idx := []int{}

	for i, it := range items {
		if it.kept == "" {
			idx = append(idx, i)
		}
	}

	return idx
}

func running(items []item) []item {
	return filter(items, func(it item) bool { return it.kept == "" })
}

func keptOf(items []item) []item {
	return filter(items, func(it item) bool { return it.kept != "" })
}

func filter(items []item, keep func(item) bool) []item {
	out := []item{}

	for _, it := range items {
		if keep(it) {
			out = append(out, it)
		}
	}

	return out
}

// reasons lists the distinct globs that kept these items, in order of appearance.
func reasons(kept []item) []string {
	out := []string{}

	for _, it := range kept {
		if !slices.Contains(out, it.kept) {
			out = append(out, it.kept)
		}
	}

	return out
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
