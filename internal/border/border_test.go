package border

import (
	"bytes"
	"context"
	"image"
	"io"
	"log/slog"
	"path/filepath"
	"reflect"
	"testing"

	"github.com/evaneecloo/white_border_adder/internal/log"
	"github.com/spf13/afero"
)

func defaults() Options {
	return Options{
		Width: 1080, Height: 1080,
		LandscapeVert: 0.05, LandscapeHoriz: 0.03,
		PortraitVert: 0.005, PortraitHoriz: 0.18,
		JPEGQuality: 100,
		Prefix:      "bordered_",
	}
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

func TestLayout(t *testing.T) {
	tests := []struct {
		name                         string
		w, h                         int
		scaledW, scaledH, offX, offY int
	}{
		// available 1015.2 x 972, scale 0.2538 -> 1015 x 761
		{"landscape", 4000, 3000, 1015, 761, 32, 159},
		// available 691.2 x 1069.2, scale 0.2304 -> 691 x 921
		{"portrait", 3000, 4000, 691, 921, 194, 79},
		// square uses portrait ratios, scale 0.6912 -> 691 x 691
		{"square", 1000, 1000, 691, 691, 194, 194},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := layout(tt.w, tt.h, defaults())
			want := image.Rect(tt.offX, tt.offY, tt.offX+tt.scaledW, tt.offY+tt.scaledH)

			if got != want {
				t.Errorf("layout(%d, %d) = %v, want %v", tt.w, tt.h, got, want)
			}
		})
	}
}

func TestPlan(t *testing.T) {
	tree := map[string]string{
		"photos/IMG_1.jpg":                   "jpeg",
		"photos/IMG_2.JPEG":                  "jpeg",
		"photos/IMG_3.png":                   "png",
		"photos/IMG_4.RW2":                   "raw",
		"photos/notes.txt":                   "text",
		"photos/converted/IMG_5.jpg":         "jpeg",
		"photos/bordered/bordered_IMG_1.jpg": "jpeg",
		"photos/.cache/IMG_6.jpg":            "jpeg",
		"photos/2024-10-paris/IMG_7.jpg":     "jpeg",
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
				{"photos/IMG_2.JPEG", "photos/bordered/bordered_IMG_2.JPEG"},
				{"photos/IMG_3.png", "photos/bordered/bordered_IMG_3.png"},
			},
			skip: "skip IMG_1.jpg: output exists\n",
		},
		{
			name: "force overwrites existing output",
			opts: func(o Options) Options { o.Force = true; return o },
			want: []Job{
				{"photos/IMG_1.jpg", "photos/bordered/bordered_IMG_1.jpg"},
				{"photos/IMG_2.JPEG", "photos/bordered/bordered_IMG_2.JPEG"},
				{"photos/IMG_3.png", "photos/bordered/bordered_IMG_3.png"},
			},
		},
		{
			name: "recursive enters converted, skips bordered and dot dirs",
			opts: func(o Options) Options { o.Recursive = true; return o },
			want: []Job{
				{"photos/2024-10-paris/IMG_7.jpg", "photos/2024-10-paris/bordered/bordered_IMG_7.jpg"},
				{"photos/IMG_2.JPEG", "photos/bordered/bordered_IMG_2.JPEG"},
				{"photos/IMG_3.png", "photos/bordered/bordered_IMG_3.png"},
				{"photos/converted/IMG_5.jpg", "photos/converted/bordered/bordered_IMG_5.jpg"},
			},
			skip: "skip IMG_1.jpg: output exists\n",
		},
		{
			name: "custom relative out dir",
			opts: func(o Options) Options { o.Out = "insta"; return o },
			want: []Job{
				{"photos/IMG_1.jpg", "photos/insta/bordered_IMG_1.jpg"},
				{"photos/IMG_2.JPEG", "photos/insta/bordered_IMG_2.JPEG"},
				{"photos/IMG_3.png", "photos/insta/bordered_IMG_3.png"},
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

	jobs, err := Plan(context.Background(), fs, "2025", Options{Prefix: "bordered_", Recursive: true})
	if err != nil {
		t.Fatal(err)
	}

	want := []Job{
		{
			"2025/20250804_luchon/converted/_1310721_converted.jpg",
			"2025/20250804_luchon/converted/bordered/bordered__1310721_converted.jpg",
		},
		{
			"2025/20251129_pot_depart1_gaspard/_1330976.JPG",
			"2025/20251129_pot_depart1_gaspard/bordered/bordered__1330976.JPG",
		},
	}
	if !reflect.DeepEqual(jobs, want) {
		t.Fatalf("jobs = %v, want %v", jobs, want)
	}
}
