package cli

import (
	"runtime"
	"sync"

	"github.com/evaneecloo/white_border_adder/internal/border"
	"github.com/spf13/cobra"
)

// Instagram's square post; the ratios are what looked right on the owner's photos.
const (
	defaultSize           = 1080
	defaultLandscapeVert  = 0.05
	defaultLandscapeHoriz = 0.03
	defaultPortraitVert   = 0.005
	defaultPortraitHoriz  = 0.18
	defaultJPEGQuality    = 100
)

func (a *app) borderCmd() *cobra.Command {
	o := border.Options{}
	workers := 0

	cmd := &cobra.Command{
		Use:   "border <dir>",
		Short: "Write a white-bordered copy of every JPEG and PNG in <dir>",
		Args:  cobra.ExactArgs(1),
		RunE: func(_ *cobra.Command, args []string) error {
			o.Recursive = a.recursive
			o.Verbose = a.skipWriter()

			return a.runBorder(args[0], o, workers)
		},
	}

	f := cmd.Flags()
	f.IntVar(&o.Width, "width", defaultSize, "output width")
	f.IntVar(&o.Height, "height", defaultSize, "output height")
	f.Float64Var(&o.LandscapeVert, "landscape-vert", defaultLandscapeVert, "vertical border ratio for landscape photos")
	f.Float64Var(&o.LandscapeHoriz, "landscape-horiz", defaultLandscapeHoriz,
		"horizontal border ratio for landscape photos")
	f.Float64Var(&o.PortraitVert, "portrait-vert", defaultPortraitVert, "vertical border ratio for portrait photos")
	f.Float64Var(&o.PortraitHoriz, "portrait-horiz", defaultPortraitHoriz,
		"horizontal border ratio for portrait photos")
	f.IntVar(&o.JPEGQuality, "jpeg-quality", defaultJPEGQuality, "JPEG output quality, 1-100")
	f.StringVar(&o.Prefix, "prefix", "bordered_", "prefix of the output file name")
	f.StringVar(&o.Out, "out", "", "output directory (default <dir>/bordered, per photo directory with --recursive)")
	f.BoolVar(&o.Force, "force", false, "overwrite an output file that already exists")
	f.IntVar(&workers, "workers", runtime.NumCPU(), "number of photos processed at once")

	return cmd
}

func (a *app) runBorder(dir string, o border.Options, workers int) error {
	jobs, err := border.Plan(a.fs, dir, o)
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

	failed := a.writeAll(dir, jobs, o, workers)
	a.out("%s written.", files(len(jobs)-failed))

	return failures(failed)
}

type result struct {
	job border.Job
	err error
}

// writeAll runs the jobs on a worker pool and prints each outcome as it
// lands; the caller gets the failure count.
func (a *app) writeAll(dir string, jobs []border.Job, o border.Options, workers int) int {
	if workers < 1 {
		workers = 1
	}

	queue := make(chan border.Job, len(jobs))
	results := make(chan result, len(jobs))

	var wg sync.WaitGroup

	for range workers {
		wg.Add(1)

		go func() {
			defer wg.Done()

			for j := range queue {
				results <- result{job: j, err: j.Run(a.fs, o)}
			}
		}()
	}

	for _, j := range jobs {
		queue <- j
	}

	close(queue)
	wg.Wait()
	close(results)

	failed := 0

	for r := range results {
		if r.err != nil {
			failed++

			a.fail("%s: %v", rel(dir, r.job.In), r.err)

			continue
		}

		a.out("wrote %s", rel(dir, r.job.Out))
	}

	return failed
}
