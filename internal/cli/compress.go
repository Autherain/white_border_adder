package cli

import (
	"context"
	"fmt"
	"path/filepath"
	"runtime"

	"github.com/evaneecloo/white_border_adder/internal/compress"
	"github.com/evaneecloo/white_border_adder/internal/log"
	"github.com/spf13/cobra"
)

func (a *app) compressCmd() *cobra.Command {
	o := compress.Options{}
	workers := 0

	cmd := &cobra.Command{
		Use:   "compress <dir>",
		Short: "Write a smaller copy of every JPEG and PNG in <dir>",
		Long: `Write a smaller copy of every JPEG and PNG at the root of <dir>.

JPEGs are re-encoded at --jpeg-quality, PNGs at the best compression
level, same pixel size, and written as <dir>/compressed/compressed_<name>.
The originals are never modified.

The only lever is --jpeg-quality. At 100 a JPEG usually comes out larger
than the original; 85 is where the loss stops being visible on a screen.

The dry run encodes 5 photos spread across the list in memory and prints
an estimate of the whole saving, "6.2 GB -> 2.2 GB, 63% smaller", before
the total. Nothing is written without --apply.`,
		Args: cobra.ExactArgs(1),
		RunE: func(cmd *cobra.Command, args []string) error {
			o.Recursive = a.recursive

			return a.runCompress(cmd.Context(), args[0], o, workers)
		},
	}

	f := cmd.Flags()
	f.IntVar(&o.JPEGQuality, "jpeg-quality", defaultJPEGQuality, "JPEG output quality, 1-100")
	f.StringVar(&o.Prefix, "prefix", "compressed_", "prefix of the output file name")
	f.StringVar(&o.Out, "out", "", "output directory (default <dir>/compressed, per photo directory with --recursive)")
	f.BoolVar(&o.Force, "force", false, "overwrite an output file that already exists")
	f.IntVar(&workers, "workers", runtime.NumCPU(), "number of photos processed at once")

	return cmd
}

func (a *app) runCompress(ctx context.Context, dir string, o compress.Options, workers int) error {
	jobs, err := compress.Plan(ctx, a.fs, dir, o)
	if err != nil {
		return err
	}

	if len(jobs) == 0 {
		log.Infof(ctx, "%s", styleFaint.Render("nothing to do"))

		return nil
	}

	items := make([]item, 0, len(jobs))
	for _, j := range jobs {
		items = append(items, item{dir: rel(dir, filepath.Dir(j.In)), files: []string{rel(dir, j.Out)}, size: j.Size})
	}

	chosen := a.show(ctx, verbWrite, items, a.estimate(ctx, dir, jobs, o, workers)...)
	if len(chosen) == 0 {
		return nil
	}

	picked := pickFrom(jobs, chosen)
	failed, saved := a.compressAll(ctx, dir, picked, o, workers)
	written := files(len(picked) - failed)

	// Quality 100 without a size limit can grow a JPEG; say so instead of
	// printing a negative saving.
	if saved < 0 {
		log.Infof(ctx, "%s", styleBold.Render(written+" written, "+human(-saved)+" larger."))
	} else {
		log.Infof(ctx, "%s", styleBold.Render(written+" written, "+human(saved)+" saved."))
	}

	return failures(failed)
}

// sampleSize is how many photos a dry run really encodes to estimate the
// saving: enough to average out one odd photo, few enough to stay quick.
const sampleSize = 5

// estimate encodes a spread of the jobs in memory and scales their ratio up to
// the whole list. Only a dry run calls it; --apply measures for real.
func (a *app) estimate(ctx context.Context, dir string, jobs []compress.Job, o compress.Options, workers int,
) []string {
	if a.apply || len(jobs) == 0 {
		return nil
	}

	sample := spread(jobs, sampleSize)
	results := runAll(workers, sample, func(j compress.Job) compressed {
		n, err := j.Estimate(a.fs, o)

		return compressed{job: j, out: n, err: err}
	})

	in, out, measured := int64(0), int64(0), 0

	for _, r := range results {
		if r.err != nil {
			log.Debugf(ctx, "skip estimate on %s: %v", rel(dir, r.job.In), r.err)

			continue
		}

		log.Debugf(ctx, "measured %s: %s -> %s", rel(dir, r.job.In), human(r.job.Size), human(r.out))

		in += r.job.Size
		out += r.out
		measured++
	}

	if measured == 0 || in == 0 {
		return nil
	}

	total := int64(0)
	for _, j := range jobs {
		total += j.Size
	}

	after := total * out / in
	change := "smaller"
	percent := (total - after) * 100 / total

	if after > total {
		change = "larger"
		percent = (after - total) * 100 / total
	}

	return []string{fmt.Sprintf("estimate: %s -> %s, %d%% %s, measured on %d of %d photos",
		human(total), human(after), percent, change, measured, len(jobs))}
}

// spread picks n items evenly along the list, first and last included, so a
// shoot that starts in the dark and ends in daylight is sampled across.
func spread[T any](all []T, n int) []T {
	if len(all) <= n {
		return all
	}

	picked := make([]T, 0, n)
	for i := range n {
		picked = append(picked, all[i*(len(all)-1)/(n-1)])
	}

	return picked
}

type compressed struct {
	job compress.Job
	out int64
	err error
}

func (a *app) compressAll(ctx context.Context, dir string, jobs []compress.Job, o compress.Options, workers int,
) (int, int64) {
	results := runAll(workers, jobs, func(j compress.Job) compressed {
		n, err := j.Run(a.fs, o)

		return compressed{job: j, out: n, err: err}
	})

	failed, saved := 0, int64(0)

	for _, r := range results {
		if r.err != nil {
			failed++

			log.Errorf(ctx, "%s: %v", rel(dir, r.job.In), r.err)

			continue
		}

		saved += r.job.Size - r.out

		log.Infof(ctx, "%s %s, %s -> %s",
			styleDone.Render("wrote"), rel(dir, r.job.Out), human(r.job.Size), human(r.out))
	}

	return failed, saved
}
