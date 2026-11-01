package cli

import (
	"context"

	"github.com/evaneecloo/white_border_adder/internal/clean"
	"github.com/evaneecloo/white_border_adder/internal/log"
	"github.com/spf13/cobra"
)

func (a *app) cleanCmd() *cobra.Command {
	o := clean.Options{}

	cmd := &cobra.Command{
		Use:   "clean <dir>",
		Short: "Delete raws that RawTherapee never developed",
		Args:  cobra.ExactArgs(1),
		RunE: func(cmd *cobra.Command, args []string) error {
			o.Recursive = a.recursive

			return a.runClean(cmd.Context(), args[0], o)
		},
	}

	f := cmd.Flags()
	f.StringVar(&o.Pattern, "pattern", "*.rw2", "glob on the file name that marks a raw")
	f.StringArrayVar(&o.Exclude, "exclude", nil, "glob on the stem to keep; repeatable")

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
		items = append(items, item{files: relAll(dir, c.Files()), size: c.Size + c.SidecarSize})
	}

	chosen, err := a.picker().pick(ctx, verbDelete, items)
	if err != nil || len(chosen) == 0 {
		return err
	}

	failed, done := a.deleteAll(ctx, dir, pickFrom(candidates, chosen))
	log.Infof(ctx, "%s", styleBold.Render(files(done.count)+" deleted, "+human(done.bytes)+" freed."))

	return failures(failed)
}

type tally struct {
	count int
	bytes int64
}

// deleteAll removes the candidates one by one and tallies what actually went,
// so a failure in the middle does not inflate the freed total.
func (a *app) deleteAll(ctx context.Context, dir string, candidates []clean.Candidate) (int, tally) {
	failed, done := 0, tally{}

	for _, c := range candidates {
		if err := c.Delete(a.fs); err != nil {
			failed++

			log.Errorf(ctx, "%s: %v", rel(dir, c.Path), err)

			continue
		}

		for _, p := range c.Files() {
			log.Infof(ctx, "%s %s", styleGone.Render("deleted"), rel(dir, p))
		}

		done.count += len(c.Files())
		done.bytes += c.Size + c.SidecarSize
	}

	return failed, done
}

func relAll(dir string, paths []string) []string {
	out := make([]string, 0, len(paths))
	for _, p := range paths {
		out = append(out, rel(dir, p))
	}

	return out
}
