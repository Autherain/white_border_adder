package cli

import (
	"testing"
	"time"

	"github.com/evaneecloo/white_border_adder/internal/sorter"
	"github.com/spf13/afero"
)

func TestRunSort(t *testing.T) {
	t.Parallel()

	tests := []struct {
		name  string
		apply bool
		want  string
	}{
		{"dry run", false, "would move d/a.png -> 2026/20260620_toulouse/a.png\n" +
			"would move d/a.png.pp3 -> 2026/20260620_toulouse/a.png.pp3\n" +
			"2 files. Re-run with --apply to move them.\n"},
		{"apply", true, "moved d/a.png -> 2026/20260620_toulouse/a.png\n" +
			"moved d/a.png.pp3 -> 2026/20260620_toulouse/a.png.pp3\n" +
			"2 files moved.\n"},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			t.Parallel()

			fs := afero.NewMemMapFs()
			mtime := time.Date(2026, 6, 20, 12, 0, 0, 0, time.Local)

			for p, c := range map[string]string{"r/d/a.png": "x", "r/d/a.png.pp3": "y", "r/2026/20260620_toulouse/b.png": "z"} {
				if err := afero.WriteFile(fs, p, []byte(c), 0o644); err != nil {
					t.Fatal(err)
				}

				_ = fs.Chtimes(p, mtime, mtime)
			}

			a, ctx, out := testApp(tt.apply)
			a.fs = fs

			if err := a.runSort(ctx, "r", []string{"r/d"}, sorter.Options{}); err != nil {
				t.Fatal(err)
			}

			if got := out.String(); got != tt.want {
				t.Errorf("got\n%s\nwant\n%s", got, tt.want)
			}

			if moved, _ := afero.Exists(fs, "r/2026/20260620_toulouse/a.png"); moved != tt.apply {
				t.Errorf("moved = %v, want %v", moved, tt.apply)
			}
		})
	}
}
