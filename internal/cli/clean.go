package cli

import (
	"fmt"

	"github.com/evaneecloo/white_border_adder/internal/clean"
	"github.com/spf13/cobra"
)

func (a *app) cleanCmd() *cobra.Command {
	o := clean.Options{}

	cmd := &cobra.Command{
		Use:   "clean <dir>",
		Short: "Delete raws that RawTherapee never developed",
		Args:  cobra.ExactArgs(1),
		RunE: func(_ *cobra.Command, args []string) error {
			o.Recursive = a.recursive
			o.Verbose = a.skipWriter()

			return a.runClean(args[0], o)
		},
	}

	f := cmd.Flags()
	f.StringVar(&o.Pattern, "pattern", "*.rw2", "glob on the file name that marks a raw")
	f.StringArrayVar(&o.Exclude, "exclude", nil, "glob on the stem to keep; repeatable")

	return cmd
}

func (a *app) runClean(dir string, o clean.Options) error {
	candidates, err := clean.Plan(a.fs, dir, o)
	if err != nil {
		return err
	}

	if len(candidates) == 0 {
		a.out("nothing to do")

		return nil
	}

	count, bytes := 0, int64(0)

	for _, c := range candidates {
		count += len(c.Files())
		bytes += c.Size + c.SidecarSize
	}

	if !a.apply {
		a.listDeletions("would delete", dir, candidates)
		a.out("%s, %s. Re-run with --apply to delete them.", files(count), human(bytes))

		return nil
	}

	failed := a.deleteAll(dir, candidates)
	a.out("%s deleted, %s freed.", files(count), human(bytes))

	return failures(failed)
}

func (a *app) listDeletions(verb, dir string, candidates []clean.Candidate) {
	for _, c := range candidates {
		for _, p := range c.Files() {
			a.out("%s %s", verb, rel(dir, p))
		}
	}
}

func (a *app) deleteAll(dir string, candidates []clean.Candidate) int {
	failed := 0

	for _, c := range candidates {
		if err := c.Delete(a.fs); err != nil {
			failed++

			a.fail("%s: %v", rel(dir, c.Path), err)

			continue
		}

		a.listDeletions("deleted", dir, []clean.Candidate{c})
	}

	return failed
}

// human rounds to the unit a photographer thinks in: raws are tens of MB.
func human(bytes int64) string {
	const (
		kb = 1000
		mb = 1000 * kb
		gb = 1000 * mb
	)

	switch {
	case bytes >= gb:
		return fmt.Sprintf("%.1f GB", float64(bytes)/gb)
	case bytes >= 10*mb:
		return fmt.Sprintf("%d MB", (bytes+mb/2)/mb)
	case bytes >= mb:
		return fmt.Sprintf("%.1f MB", float64(bytes)/mb)
	case bytes >= kb:
		return fmt.Sprintf("%d kB", (bytes+kb/2)/kb)
	default:
		return fmt.Sprintf("%d B", bytes)
	}
}
