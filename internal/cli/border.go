package cli

import (
	"context"
	"runtime"

	"github.com/evaneecloo/white_border_adder/internal/border"
	"github.com/evaneecloo/white_border_adder/internal/log"
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
		Long: `Write a white-bordered copy of every JPEG and PNG at the root of <dir>.

Each photo is scaled to fit inside a white 1080x1080 canvas (Instagram's
square) minus the border ratios, centred, and written as
<dir>/bordered/bordered_<name>. Landscape and portrait photos get their
own ratios. The originals are never modified.

An output file that already exists is skipped unless --force. Nothing is
written without --apply.`,
		Args: cobra.ExactArgs(1),
		RunE: func(cmd *cobra.Command, args []string) error {
			o.Recursive = a.recursive

			return a.runBorder(cmd.Context(), args[0], o, workers)
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

func (a *app) runBorder(ctx context.Context, dir string, o border.Options, workers int) error {
	jobs, err := border.Plan(ctx, a.fs, dir, o)
	if err != nil {
		return err
	}

	if len(jobs) == 0 {
		log.Infof(ctx, "%s", styleFaint.Render("nothing to do"))

		return nil
	}

	items := make([]item, 0, len(jobs))
	for _, j := range jobs {
		items = append(items, item{files: []string{rel(dir, j.Out)}})
	}

	chosen := a.show(ctx, verbWrite, items)
	if len(chosen) == 0 {
		return nil
	}

	picked := pickFrom(jobs, chosen)
	failed := a.writeAll(ctx, dir, picked, o, workers)
	log.Infof(ctx, "%s", styleBold.Render(files(len(picked)-failed)+" written."))

	return failures(failed)
}

type result struct {
	job border.Job
	err error
}

func (a *app) writeAll(ctx context.Context, dir string, jobs []border.Job, o border.Options, workers int) int {
	results := runAll(workers, jobs, func(j border.Job) result {
		return result{job: j, err: j.Run(a.fs, o)}
	})

	failed := 0

	for _, r := range results {
		if r.err != nil {
			failed++

			log.Errorf(ctx, "%s: %v", rel(dir, r.job.In), r.err)

			continue
		}

		log.Infof(ctx, "%s %s", styleDone.Render("wrote"), rel(dir, r.job.Out))
	}

	return failed
}
