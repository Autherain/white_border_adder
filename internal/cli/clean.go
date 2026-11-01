package cli

import (
	"context"
	"path/filepath"

	"github.com/evaneecloo/white_border_adder/internal/clean"
	"github.com/evaneecloo/white_border_adder/internal/log"
	"github.com/spf13/cobra"
)

func (a *app) cleanCmd() *cobra.Command {
	o := clean.Options{}

	cmd := &cobra.Command{
		Use:   "clean <dir>",
		Short: "Delete raws that RawTherapee never developed",
		Long: `Delete the raws of <dir> that RawTherapee never developed.

What gets deleted, step by step:

  1. <dir> must contain a converted/ subdirectory, the folder RawTherapee
     exports into. Without it nothing happens there: no converted/ means no
     sorting was done. With --recursive every subdirectory is checked the
     same way; those without converted/ are skipped.
  2. A raw is a file at the root of that directory matching --pattern
     (*.rw2 by default, any case). Raws in subdirectories or inside
     converted/ are never touched.
  3. A raw is developed when any file in converted/ starts with the raw's
     name without extension: IMG_1264.RW2 is developed by IMG_1264.jpg,
     IMG_1264_bw.tif or IMG_1264-2.png. The extension does not matter.
  4. Every raw that is not developed is deleted, with its RawTherapee
     sidecar (IMG_1264.RW2.pp3) when there is one. The sidecar of a raw
     that stays, stays.

Nothing is deleted without --apply: the default run prints one line per
file, a banner per directory with its total, and the grand total.

A line ending in "####### has a .pp3 but nothing in converted/" is a raw
you opened in RawTherapee (the .pp3 holds your edits) but never exported.
Look at it before --apply, or keep it with --exclude.

--exclude takes a glob and keeps every raw whose name without extension
matches it (--exclude IMG_1264, --exclude 'IMG_12*'), or whose path below
<dir> has a directory matching it (--exclude '*marie*' keeps a whole
shoot). Repeat the flag for several patterns; case does not matter.`,
		Args: cobra.ExactArgs(1),
		RunE: func(cmd *cobra.Command, args []string) error {
			o.Recursive = a.recursive

			return a.runClean(cmd.Context(), args[0], o)
		},
	}

	f := cmd.Flags()
	f.StringVar(&o.Pattern, "pattern", "*.rw2", "glob on the file name that marks a raw")
	f.StringArrayVar(&o.Exclude, "exclude", nil,
		"glob on a raw's name without extension or on a directory of its path, to keep; repeatable")

	return cmd
}

func (a *app) runClean(ctx context.Context, dir string, o clean.Options) error {
	candidates, err := clean.Plan(ctx, a.fs, dir, o)
	if err != nil {
		return err
	}

	if len(candidates) == 0 {
		log.Infof(ctx, "%s", styleFaint.Render("nothing to do"))

		return nil
	}

	items := make([]item, 0, len(candidates))

	for _, c := range candidates {
		it := item{
			dir:   rel(dir, filepath.Dir(c.Path)),
			files: relAll(dir, c.Files()),
			size:  c.Size + c.SidecarSize,
			kept:  c.KeptBy,
		}

		// A sidecar without an export means time was spent on this raw in
		// RawTherapee; worth a look before it goes.
		if c.Sidecar != "" {
			it.warn = "has a .pp3 but nothing in converted/"
		}

		items = append(items, it)
	}

	chosen := a.show(ctx, verbDelete, items)
	if len(chosen) == 0 {
		return nil
	}

	failed := a.deleteAll(ctx, dir, pickFrom(candidates, chosen))

	for _, line := range recap(verbDelete, items, finished) {
		log.Infof(ctx, "%s", line)
	}

	return failures(failed)
}

// deleteAll removes the candidates one by one; a failure is reported and
// counted, the rest goes on.
func (a *app) deleteAll(ctx context.Context, dir string, candidates []clean.Candidate) int {
	failed := 0

	for _, c := range candidates {
		if err := c.Delete(a.fs); err != nil {
			failed++

			log.Errorf(ctx, "%s: %v", rel(dir, c.Path), err)

			continue
		}

		for _, p := range c.Files() {
			log.Infof(ctx, "%s %s", styleGone.Render("deleted"), rel(dir, p))
		}
	}

	return failed
}

func relAll(dir string, paths []string) []string {
	out := make([]string, 0, len(paths))
	for _, p := range paths {
		out = append(out, rel(dir, p))
	}

	return out
}
