package compress

import (
	"bytes"
	"context"
	"io"
	"log/slog"
	"path/filepath"
	"reflect"
	"testing"

	"github.com/spf13/afero"

	"github.com/evaneecloo/white_border_adder/internal/log"
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

func TestFit(t *testing.T) {
	tests := []struct {
		name             string
		w, h, maxW, maxH int
		wantW, wantH     int
	}{
		{"no limit", 4000, 3000, 0, 0, 4000, 3000},
		{"width bound", 4000, 3000, 2000, 0, 2000, 1500},
		{"height bound", 4000, 3000, 0, 1500, 2000, 1500},
		{"both, tighter side wins", 4000, 3000, 2000, 600, 800, 600},
		{"smaller than limit stays", 800, 600, 2000, 2000, 800, 600},
		{"portrait width only", 3000, 4000, 1080, 0, 1080, 1440},
		{"floors odd ratios", 4000, 3000, 1001, 0, 1001, 750},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			w, h := fit(tt.w, tt.h, tt.maxW, tt.maxH)
			if w != tt.wantW || h != tt.wantH {
				t.Errorf("fit(%d, %d, %d, %d) = %d x %d, want %d x %d",
					tt.w, tt.h, tt.maxW, tt.maxH, w, h, tt.wantW, tt.wantH)
			}
		})
	}
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
			name: "recursive skips converted, bordered, compressed and dot dirs",
			opts: func(o Options) Options { o.Recursive = true; return o },
			want: []Job{
				{"photos/2024-10-paris/IMG_7.jpg", "photos/2024-10-paris/compressed/compressed_IMG_7.jpg", 4},
				{"photos/IMG_2.JPEG", "photos/compressed/compressed_IMG_2.JPEG", 11},
				{"photos/IMG_3.png", "photos/compressed/compressed_IMG_3.png", 3},
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
