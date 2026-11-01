package cli

import (
	"runtime"

	"github.com/evaneecloo/white_border_adder/internal/compress"
	"github.com/spf13/cobra"
)

// 85 is where JPEG artefacts stop being visible on a phone screen while the
// file drops to a fraction of the camera's quality-100 output.
const defaultCompressQuality = 85

func (a *app) compressCmd() *cobra.Command {
	o := compress.Options{}
	workers := 0

	cmd := &cobra.Command{
		Use:   "compress <dir>",
		Short: "Write a smaller copy of every JPEG and PNG in <dir>",
		Args:  cobra.ExactArgs(1),
		RunE: func(_ *cobra.Command, args []string) error {
			o.Recursive = a.recursive
			o.Verbose = a.skipWriter()

			return a.runCompress(args[0], o, workers)
		},
	}

	f := cmd.Flags()
	f.IntVar(&o.JPEGQuality, "jpeg-quality", defaultCompressQuality, "JPEG output quality, 1-100")
	f.IntVar(&o.MaxWidth, "max-width", 0, "downscale to fit this width; 0 keeps the size")
	f.IntVar(&o.MaxHeight, "max-height", 0, "downscale to fit this height; 0 keeps the size")
	f.StringVar(&o.Prefix, "prefix", "compressed_", "prefix of the output file name")
	f.StringVar(&o.Out, "out", "", "output directory (default <dir>/compressed, per photo directory with --recursive)")
	f.BoolVar(&o.Force, "force", false, "overwrite an output file that already exists")
	f.IntVar(&workers, "workers", runtime.NumCPU(), "number of photos processed at once")

	return cmd
}

func (a *app) runCompress(dir string, o compress.Options, workers int) error {
	jobs, err := compress.Plan(a.fs, dir, o)
	if err != nil {
		return err
	}

	if len(jobs) == 0 {
		a.out("nothing to do")

		return nil
	}

	if !a.apply {
		for _, j := range jobs {
			a.out("would write %s", rel(dir, j.Out))
		}

		a.out("%s. Re-run with --apply to write them.", files(len(jobs)))

		return nil
	}

	failed, saved := a.compressAll(dir, jobs, o, workers)
	a.out("%s written, %s saved.", files(len(jobs)-failed), human(saved))

	return failures(failed)
}

type compressed struct {
	job compress.Job
	out int64
	err error
}

func (a *app) compressAll(dir string, jobs []compress.Job, o compress.Options, workers int) (int, int64) {
	results := runAll(workers, jobs, func(j compress.Job) compressed {
		n, err := j.Run(a.fs, o)

		return compressed{job: j, out: n, err: err}
	})

	failed, saved := 0, int64(0)

	for _, r := range results {
		if r.err != nil {
			failed++

			a.fail("%s: %v", rel(dir, r.job.In), r.err)

			continue
		}

		saved += r.job.Size - r.out

		a.out("wrote %s, %s -> %s", rel(dir, r.job.Out), human(r.job.Size), human(r.out))
	}

	return failed, saved
}
