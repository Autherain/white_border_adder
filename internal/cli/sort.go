package cli

import (
	"context"
	"path/filepath"

	"github.com/evaneecloo/white_border_adder/internal/log"
	"github.com/evaneecloo/white_border_adder/internal/sorter"
	"github.com/spf13/cobra"
)

func (a *app) sortCmd() *cobra.Command {
	o := sorter.Options{}

	cmd := &cobra.Command{
		Use:   "sort <dir> [source...]",
		Short: "File photos under <year>/<day folder> by the date they were shot",
		Long: `File every photo under <dir>/<year>/<day folder>, by the date it was shot.

Run it on the directory that holds your 2025/ and 2026/ folders, or from inside
one of them: a <dir> named like a year is searched alone and the photos that
belong to another year leave it for that year's folder. Without
[source] it re-files everything below <dir>, which fixes photos moved into
the wrong day folder; give a [source] directory (a dump of unsorted photos,
inside or outside <dir>) to file only what is in it. Use --recursive to go
into subdirectories.

The date is the one the camera wrote inside the file (EXIF), read from the
first 256 kB only. A photo without one, a PNG for instance, falls back to its
modification time.

The day folder is chosen in this order:
  1. the dated folder the photo already sits in, when its date is right:
     the photo stays, nothing is listed;
  2. the folder of that day already under the year (20260620_toulouse), so
     the place name you typed is kept;
  3. a new folder named after the day (20260620).
When several folders exist for a day the photo is kept and listed: only you
know which is right.

Raws, JPEG and PNG move. A raw's .pp3 sidecar and its developed copies in
converted/ (IMG_1264.jpg, IMG_1264-2.png for IMG_1264.RW2, not IMG_12645.jpg)
move with it, into <day folder>/converted/. converted/ and bordered/ are
never scanned as photos, and bordered/ is never moved: it is not original. Nothing is
overwritten, and emptied folders are left in place.

--exclude leaves alone every photo whose name without extension matches the
glob, or that has a directory in its path matching it, and does not even read
it: --exclude 'DSC_*' for the old Nikon numbering, --exclude '*nikon*' for a
whole folder. Repeat it for several; case does not matter. Add --verbose to
see what was left out.

Nothing moves without --apply.`,
		Args: cobra.MinimumNArgs(1),
		RunE: func(cmd *cobra.Command, args []string) error {
			o.Recursive = a.recursive

			return a.runSort(cmd.Context(), args[0], args[1:], o)
		},
	}

	cmd.Flags().StringArrayVar(&o.Exclude, "exclude", nil,
		"glob on a photo's name without extension or on a directory of its path, to leave alone; repeatable")

	return cmd
}

func (a *app) runSort(ctx context.Context, dir string, sources []string, o sorter.Options) error {
	moves, err := sorter.Plan(ctx, a.fs, dir, sources, o)
	if err != nil {
		return err
	}

	if len(moves) == 0 {
		log.Infof(ctx, "%s", styleFaint.Render("nothing to do"))

		return nil
	}

	items := make([]item, 0, len(moves))

	for _, m := range moves {
		it := item{dir: rel(dir, filepath.Dir(m.To)), kept: m.Kept}
		if m.To == "" {
			it.dir = rel(dir, filepath.Dir(m.From))
		}

		if m.To == "" {
			it.files = []string{rel(dir, m.From)}
		}

		for _, pair := range moving(m) {
			it.files = append(it.files, rel(dir, pair.From)+" -> "+rel(dir, pair.To))
		}

		items = append(items, it)
	}

	chosen := a.show(ctx, verbMove, items, nil)
	if len(chosen) == 0 {
		return nil
	}

	failed := a.moveAll(ctx, dir, pickFrom(moves, chosen))

	for _, line := range recap(verbMove, items, nil, finished) {
		log.Infof(ctx, "%s", line)
	}

	return failures(failed)
}

// moving is what the move touches; a kept move has no target and touches nothing.
func moving(m sorter.Move) []sorter.Pair {
	if m.To == "" {
		return nil
	}

	return m.Pairs()
}

func (a *app) moveAll(ctx context.Context, dir string, moves []sorter.Move) int {
	failed := 0

	for _, m := range moves {
		if err := m.Run(a.fs); err != nil {
			failed++

			log.Errorf(ctx, "%s: %v", rel(dir, m.From), err)

			continue
		}

		for _, pair := range m.Pairs() {
			log.Infof(ctx, "%s %s -> %s", styleDone.Render("moved"), rel(dir, pair.From), rel(dir, pair.To))
		}
	}

	return failed
}
