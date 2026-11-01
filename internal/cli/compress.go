package cli

import (
	"context"
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
		Args:  cobra.ExactArgs(1),
		RunE: func(cmd *cobra.Command, args []string) error {
			o.Recursive = a.recursive

			return a.runCompress(cmd.Context(), args[0], o, workers)
		},
	}

	f := cmd.Flags()
	f.IntVar(&o.JPEGQuality, "jpeg-quality", defaultJPEGQuality, "JPEG output quality, 1-100")
	f.IntVar(&o.MaxWidth, "max-width", 0, "downscale to fit this width; 0 keeps the size")
	f.IntVar(&o.MaxHeight, "max-height", 0, "downscale to fit this height; 0 keeps the size")
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
		items = append(items, item{files: []string{rel(dir, j.Out)}, size: j.Size})
	}

	chosen, err := a.picker().pick(ctx, verbWrite, items)
	if err != nil || len(chosen) == 0 {
		return err
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
