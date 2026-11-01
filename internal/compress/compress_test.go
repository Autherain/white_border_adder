package compress

import (
	"bytes"
	"context"
	"image"
	"image/jpeg"
	"image/png"
	"io"
	"log/slog"
	"path/filepath"
	"reflect"
	"testing"

	"github.com/evaneecloo/white_border_adder/internal/log"
	"github.com/spf13/afero"
)

func defaults() Options {
	return Options{JPEGQuality: 85, Prefix: "compressed_"}
}

func newTree(t *testing.T, files map[string]string) afero.Fs {
	t.Helper()

	fs := afero.NewMemMapFs()

	for path, content := range files {
		if err := fs.MkdirAll(filepath.Dir(path), 0o755); err != nil {
			t.Fatal(err)
		}

		if err := afero.WriteFile(fs, path, []byte(content), 0o644); err != nil {
			t.Fatal(err)
		}
	}

	return fs
}

func TestPlan(t *testing.T) {
	tree := map[string]string{
		"photos/IMG_1.jpg":                       "jpeg",
		"photos/IMG_2.JPEG":                      "jpeg-bigger",
		"photos/IMG_3.png":                       "png",
		"photos/IMG_4.RW2":                       "raw",
		"photos/notes.txt":                       "text",
		"photos/converted/IMG_5.jpg":             "jpeg",
		"photos/bordered/bordered_IMG_1.jpg":     "jpeg",
		"photos/compressed/compressed_IMG_1.jpg": "jpeg",
		"photos/.cache/IMG_6.jpg":                "jpeg",
		"photos/2024-10-paris/IMG_7.jpg":         "jpeg",
	}

	tests := []struct {
		name string
		opts func(Options) Options
		want []Job
		skip string
	}{
		{
			name: "non-recursive skips existing output",
			opts: func(o Options) Options { return o },
			want: []Job{
				{"photos/IMG_2.JPEG", "photos/compressed/compressed_IMG_2.JPEG", 11},
				{"photos/IMG_3.png", "photos/compressed/compressed_IMG_3.png", 3},
			},
			skip: "skip IMG_1.jpg: output exists\n",
		},
		{
			name: "force overwrites existing output",
			opts: func(o Options) Options { o.Force = true; return o },
			want: []Job{
				{"photos/IMG_1.jpg", "photos/compressed/compressed_IMG_1.jpg", 4},
				{"photos/IMG_2.JPEG", "photos/compressed/compressed_IMG_2.JPEG", 11},
				{"photos/IMG_3.png", "photos/compressed/compressed_IMG_3.png", 3},
			},
		},
		{
			name: "recursive enters converted, skips bordered, compressed and dot dirs",
			opts: func(o Options) Options { o.Recursive = true; return o },
			want: []Job{
				{"photos/2024-10-paris/IMG_7.jpg", "photos/2024-10-paris/compressed/compressed_IMG_7.jpg", 4},
				{"photos/IMG_2.JPEG", "photos/compressed/compressed_IMG_2.JPEG", 11},
				{"photos/IMG_3.png", "photos/compressed/compressed_IMG_3.png", 3},
				{"photos/converted/IMG_5.jpg", "photos/converted/compressed/compressed_IMG_5.jpg", 4},
			},
			skip: "skip IMG_1.jpg: output exists\n",
		},
		{
			name: "custom relative out dir",
			opts: func(o Options) Options { o.Out = "small"; return o },
			want: []Job{
				{"photos/IMG_1.jpg", "photos/small/compressed_IMG_1.jpg", 4},
				{"photos/IMG_2.JPEG", "photos/small/compressed_IMG_2.JPEG", 11},
				{"photos/IMG_3.png", "photos/small/compressed_IMG_3.png", 3},
			},
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			fs := newTree(t, tree)
			var verbose bytes.Buffer

			ctx := log.With(context.Background(), log.New(&verbose, io.Discard, slog.LevelDebug))

			got, err := Plan(ctx, fs, "photos", tt.opts(defaults()))
			if err != nil {
				t.Fatal(err)
			}

			if !reflect.DeepEqual(got, tt.want) {
				t.Errorf("jobs = %v, want %v", got, tt.want)
			}

			if verbose.String() != tt.skip {
				t.Errorf("verbose = %q, want %q", verbose.String(), tt.skip)
			}
		})
	}
}

func TestPlanRejectsMissingOrFileArgument(t *testing.T) {
	fs := newTree(t, map[string]string{"photos/IMG_1.jpg": "jpeg"})

	if _, err := Plan(context.Background(), fs, "nowhere", defaults()); err == nil {
		t.Error("missing directory: want error")
	}

	if _, err := Plan(context.Background(), fs, "photos/IMG_1.jpg", defaults()); err == nil {
		t.Error("file argument: want error")
	}
}

// rawTherapeeLayout is the owner's real tree: one directory per shoot, raws
// and sidecars at its root, exports in converted/, and the outputs of wba and
// of the script before it under converted/.
var rawTherapeeLayout = map[string]string{
	"2025/20250804_luchon/_1310721.RW2":                                    "raw",
	"2025/20250804_luchon/_1310721.RW2.pp3":                                "sidecar",
	"2025/20250804_luchon/_1310726.RW2":                                    "raw",
	"2025/20250804_luchon/_1310752.RW2":                                    "raw",
	"2025/20250804_luchon/converted/_1310721_converted.jpg":                "jpeg",
	"2025/20250804_luchon/converted/bordered_images/bordered__1310721.jpg": "jpeg",
	"2025/20250804_luchon/converted/bordered/bordered__1310721.jpg":        "jpeg",
	"2025/20250804_luchon/converted/compressed/compressed__1310721.jpg":    "jpeg",
	"2025/20250715_toulouse/_1300001.RW2":                                  "raw",
	"2025/20251129_pot_depart1_gaspard/_1330976.JPG":                       "jpeg",
	"2025/.stfolder/marker":                                                "",
}

func TestPlanRawTherapeeLayout(t *testing.T) {
	fs := newTree(t, rawTherapeeLayout)

	jobs, err := Plan(context.Background(), fs, "2025", Options{Prefix: "compressed_", Recursive: true})
	if err != nil {
		t.Fatal(err)
	}

	want := []Job{
		{
			"2025/20250804_luchon/converted/_1310721_converted.jpg",
			"2025/20250804_luchon/converted/compressed/compressed__1310721_converted.jpg", 4,
		},
		{
			"2025/20251129_pot_depart1_gaspard/_1330976.JPG",
			"2025/20251129_pot_depart1_gaspard/compressed/compressed__1330976.JPG", 4,
		},
	}
	if !reflect.DeepEqual(jobs, want) {
		t.Fatalf("jobs = %v, want %v", jobs, want)
	}
}

// Estimate must announce the size Run then produces, for both encoders.
func TestEstimateMatchesRun(t *testing.T) {
	fs := afero.NewMemMapFs()
	img := image.NewRGBA(image.Rect(0, 0, 8, 6))

	for i := range img.Pix {
		img.Pix[i] = byte(i * 7)
	}

	write := func(name string, enc func(io.Writer, image.Image) error) {
		t.Helper()

		var buf bytes.Buffer
		if err := enc(&buf, img); err != nil {
			t.Fatal(err)
		}

		if err := afero.WriteFile(fs, name, buf.Bytes(), 0o644); err != nil {
			t.Fatal(err)
		}
	}

	write("a.jpg", func(w io.Writer, m image.Image) error { return jpeg.Encode(w, m, &jpeg.Options{Quality: 100}) })
	write("b.png", png.Encode)

	for _, name := range []string{"a.jpg", "b.png"} {
		job := Job{In: name, Out: "out/" + name}
		o := Options{JPEGQuality: 85}

		estimated, err := job.Estimate(fs, o)
		if err != nil {
			t.Fatal(err)
		}

		written, err := job.Run(fs, o)
		if err != nil {
			t.Fatal(err)
		}

		if estimated != written || estimated == 0 {
			t.Errorf("%s: estimate = %d, run wrote %d", name, estimated, written)
		}
	}
}
