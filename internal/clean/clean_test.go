package clean

import (
	"context"
	"io"
	"log/slog"
	"os"
	"path/filepath"
	"reflect"
	"slices"
	"sort"
	"strings"
	"testing"

	"github.com/evaneecloo/white_border_adder/internal/log"
	"github.com/spf13/afero"
)

func newTree(t *testing.T, files map[string]string) afero.Fs {
	t.Helper()

	fs := afero.NewMemMapFs()

	for p, content := range files {
		if err := fs.MkdirAll(filepath.Dir(p), 0o755); err != nil {
			t.Fatal(err)
		}

		if err := afero.WriteFile(fs, p, []byte(content), 0o644); err != nil {
			t.Fatal(err)
		}
	}

	return fs
}

func paths(t *testing.T, fs afero.Fs) []string {
	t.Helper()

	found := []string{}

	err := afero.Walk(fs, ".", func(p string, info os.FileInfo, err error) error {
		if err != nil {
			return err
		}

		if !info.IsDir() {
			found = append(found, p)
		}

		return nil
	})
	if err != nil {
		t.Fatal(err)
	}

	sort.Strings(found)

	return found
}

func candidatePaths(cs []Candidate) []string {
	out := []string{}
	for _, c := range cs {
		out = append(out, c.Path)
	}

	return out
}

func TestPlan(t *testing.T) {
	defaults := Options{Pattern: "*.rw2"}

	tests := []struct {
		name  string
		tree  map[string]string
		dir   string
		opts  Options
		want  []string
		files []string // every file Delete removes, raws and sidecars
	}{
		{
			name: "developed raws are kept whatever the converted name",
			tree: map[string]string{
				"paris/IMG_1264.RW2":                     "raw",
				"paris/IMG_1265.RW2":                     "raw",
				"paris/IMG_1266.RW2":                     "raw",
				"paris/IMG_1267.RW2":                     "raw",
				"paris/converted/IMG_1264.jpg":           "jpeg",
				"paris/converted/IMG_1265_converted.jpg": "jpeg",
				"paris/converted/img_1266-bw.tif":        "tiff",
			},
			dir:   "paris",
			opts:  defaults,
			want:  []string{"paris/IMG_1267.RW2"},
			files: []string{"paris/IMG_1267.RW2"},
		},
		{
			name: "stem must be a prefix, extension alone does not count",
			tree: map[string]string{
				"paris/IMG_1264.RW2":           "raw",
				"paris/converted/IMG_9999.RW2": "raw",
			},
			dir:   "paris",
			opts:  defaults,
			want:  []string{"paris/IMG_1264.RW2"},
			files: []string{"paris/IMG_1264.RW2"},
		},
		{
			name: "sidecar goes with its raw and stays with a kept raw",
			tree: map[string]string{
				"paris/IMG_1264.RW2":           "raw",
				"paris/IMG_1264.RW2.pp3":       "sidecar",
				"paris/IMG_1271.RW2":           "raw",
				"paris/IMG_1271.RW2.pp3":       "sidecar",
				"paris/converted/IMG_1264.jpg": "jpeg",
			},
			dir:   "paris",
			opts:  defaults,
			want:  []string{"paris/IMG_1271.RW2"},
			files: []string{"paris/IMG_1271.RW2", "paris/IMG_1271.RW2.pp3"},
		},
		{
			name: "excluded stems leave the list",
			tree: map[string]string{
				"paris/IMG_1264.RW2":           "raw",
				"paris/IMG_1271.RW2":           "raw",
				"paris/IMG_1300.RW2":           "raw",
				"paris/IMG_2000.RW2":           "raw",
				"paris/converted/IMG_0001.jpg": "jpeg",
			},
			dir:   "paris",
			opts:  Options{Pattern: "*.rw2", Exclude: []string{"IMG_12*", "IMG_1300"}},
			want:  []string{"paris/IMG_2000.RW2"},
			files: []string{"paris/IMG_2000.RW2"},
		},
		{
			name: "no converted directory means nothing is a candidate",
			tree: map[string]string{
				"lyon/IMG_2000.RW2": "raw",
				"lyon/converted":    "a file, not a directory",
			},
			dir:   "lyon",
			opts:  defaults,
			want:  []string{},
			files: []string{},
		},
		{
			name: "only the root of the photo directory is scanned",
			tree: map[string]string{
				"paris/IMG_1264.RW2":           "raw",
				"paris/converted/IMG_5000.RW2": "raw",
				"paris/backup/IMG_6000.RW2":    "raw",
				"paris/converted/IMG_0001.jpg": "jpeg",
			},
			dir:   "paris",
			opts:  defaults,
			want:  []string{"paris/IMG_1264.RW2"},
			files: []string{"paris/IMG_1264.RW2"},
		},
		{
			name: "pattern is case-insensitive and other files are ignored",
			tree: map[string]string{
				"paris/a.rw2":                  "raw",
				"paris/B.RW2":                  "raw",
				"paris/c.jpg":                  "jpeg",
				"paris/converted/IMG_0001.jpg": "jpeg",
			},
			dir:   "paris",
			opts:  defaults,
			want:  []string{"paris/B.RW2", "paris/a.rw2"},
			files: []string{"paris/B.RW2", "paris/a.rw2"},
		},
		{
			name: "recursive visits every photo directory below the argument",
			tree: map[string]string{
				"2024-10-paris/IMG_1264.RW2":           "raw",
				"2024-10-paris/IMG_1264.RW2.pp3":       "sidecar",
				"2024-10-paris/IMG_1271.RW2":           "raw",
				"2024-10-paris/converted/IMG_1264.jpg": "jpeg",
				"2024-11-lyon/IMG_2000.RW2":            "raw", // no converted/: skipped
				"2024-11-lyon/day2/IMG_2100.RW2":       "raw",
				"2024-11-lyon/day2/converted/x.jpg":    "jpeg",
				"2024-11-lyon/day2/converted/IMG.RW2":  "raw",
				".cache/IMG_3000.RW2":                  "raw",
				".cache/converted/x.jpg":               "jpeg",
				"IMG_0001.RW2":                         "raw", // the root itself has no converted/
			},
			dir:  ".",
			opts: Options{Pattern: "*.rw2", Recursive: true},
			want: []string{"2024-10-paris/IMG_1271.RW2", "2024-11-lyon/day2/IMG_2100.RW2"},
			files: []string{
				"2024-10-paris/IMG_1271.RW2",
				"2024-11-lyon/day2/IMG_2100.RW2",
			},
		},
		{
			name: "non-recursive stops at the argument",
			tree: map[string]string{
				"paris/IMG_1264.RW2":           "raw",
				"paris/converted/x.jpg":        "jpeg",
				"paris/day2/IMG_2100.RW2":      "raw",
				"paris/day2/converted/x.jpg":   "jpeg",
				"paris/converted/IMG_0001.jpg": "jpeg",
			},
			dir:   "paris",
			opts:  defaults,
			want:  []string{"paris/IMG_1264.RW2"},
			files: []string{"paris/IMG_1264.RW2"},
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			fs := newTree(t, tt.tree)
			before := paths(t, fs)

			all, err := Plan(context.Background(), fs, tt.dir, tt.opts)
			if err != nil {
				t.Fatal(err)
			}

			got := toDelete(all)

			if gotPaths := candidatePaths(got); !reflect.DeepEqual(gotPaths, tt.want) {
				t.Fatalf("candidates = %v, want %v", gotPaths, tt.want)
			}

			if after := paths(t, fs); !reflect.DeepEqual(after, before) {
				t.Fatalf("Plan changed the tree: %v, was %v", after, before)
			}

			removed := []string{}

			for _, c := range got {
				removed = append(removed, c.Files()...)

				if err := c.Delete(fs); err != nil {
					t.Fatal(err)
				}
			}

			sort.Strings(removed)

			if !reflect.DeepEqual(removed, tt.files) {
				t.Fatalf("Delete removed %v, want %v", removed, tt.files)
			}

			if after, want := paths(t, fs), without(before, removed); !reflect.DeepEqual(after, want) {
				t.Fatalf("tree after Delete = %v, want %v", after, want)
			}
		})
	}
}

func without(all, drop []string) []string {
	dropped := map[string]bool{}
	for _, p := range drop {
		dropped[p] = true
	}

	kept := []string{}

	for _, p := range all {
		if !dropped[p] {
			kept = append(kept, p)
		}
	}

	return kept
}

func TestPlanSizes(t *testing.T) {
	fs := newTree(t, map[string]string{
		"paris/IMG_1271.RW2":     "12345",
		"paris/IMG_1271.RW2.pp3": "abc",
		"paris/converted/x.jpg":  "jpeg",
	})

	got, err := Plan(context.Background(), fs, "paris", Options{Pattern: "*.rw2"})
	if err != nil {
		t.Fatal(err)
	}

	want := []Candidate{{
		Path:        "paris/IMG_1271.RW2",
		Size:        5,
		Sidecar:     "paris/IMG_1271.RW2.pp3",
		SidecarSize: 3,
	}}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("candidates = %+v, want %+v", got, want)
	}
}

func TestPlanErrors(t *testing.T) {
	fs := newTree(t, map[string]string{"paris/converted/x.jpg": "jpeg", "file.txt": "x"})

	tests := []struct {
		name string
		dir  string
		opts Options
	}{
		{name: "missing directory", dir: "nowhere", opts: Options{Pattern: "*.rw2"}},
		{name: "argument is a file", dir: "file.txt", opts: Options{Pattern: "*.rw2"}},
		{name: "bad pattern", dir: "paris", opts: Options{Pattern: "[.rw2"}},
		{name: "bad exclude", dir: "paris", opts: Options{Pattern: "*.rw2", Exclude: []string{"["}}},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if _, err := Plan(context.Background(), fs, tt.dir, tt.opts); err == nil {
				t.Fatal("expected an error")
			}
		})
	}
}

func TestPlanVerbose(t *testing.T) {
	fs := newTree(t, map[string]string{
		"2024-10-paris/IMG_1264.RW2":    "raw",
		"2024-10-paris/converted/x.jpg": "jpeg",
		"2024-11-lyon/IMG_2000.RW2":     "raw",
	})

	var out strings.Builder

	ctx := log.With(context.Background(), log.New(&out, io.Discard, slog.LevelDebug))

	if _, err := Plan(ctx, fs, ".", Options{Pattern: "*.rw2", Recursive: true}); err != nil {
		t.Fatal(err)
	}

	want := "skip .: no converted/\nskip 2024-11-lyon: no converted/\n"
	if out.String() != want {
		t.Fatalf("verbose output = %q, want %q", out.String(), want)
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
	"2025/20250715_toulouse/_1300001.RW2":                                  "raw",
	"2025/20251129_pot_depart1_gaspard/_1330976.JPG":                       "jpeg",
	"2025/.stfolder/marker":                                                "",
}

func TestPlanRawTherapeeLayout(t *testing.T) {
	fs := newTree(t, rawTherapeeLayout)

	all, err := Plan(context.Background(), fs, "2025", Options{Pattern: "*.rw2", Recursive: true})
	if err != nil {
		t.Fatal(err)
	}

	got := toDelete(all)

	// _1310721 is developed, 20250715_toulouse has no converted/, the JPEG
	// shoot has no raws: only the two undeveloped Luchon raws remain.
	want := []string{"2025/20250804_luchon/_1310726.RW2", "2025/20250804_luchon/_1310752.RW2"}

	paths := []string{}
	for _, c := range got {
		paths = append(paths, c.Files()...)
	}

	if !reflect.DeepEqual(paths, want) {
		t.Fatalf("candidates = %v, want %v", paths, want)
	}
}

func TestPlanExcludeDirectory(t *testing.T) {
	tests := []struct {
		name    string
		exclude []string
		want    []string
	}{
		{"by shoot directory", []string{"*luchon*"}, []string{}},
		{"by stem", []string{"_1310726"}, []string{"2025/20250804_luchon/_1310752.RW2"}},
		{"by path", []string{"20250804_luchon/_131075*"}, []string{"2025/20250804_luchon/_1310726.RW2"}},
		{"no match", []string{"luchon"}, []string{"2025/20250804_luchon/_1310726.RW2", "2025/20250804_luchon/_1310752.RW2"}},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			fs := newTree(t, rawTherapeeLayout)

			all, err := Plan(context.Background(), fs, "2025", Options{Pattern: "*.rw2", Recursive: true, Exclude: tt.exclude})
			if err != nil {
				t.Fatal(err)
			}

			paths := []string{}
			for _, c := range toDelete(all) {
				paths = append(paths, c.Path)
			}

			// Kept candidates stay in the plan, marked with the glob that kept them.
			for _, c := range all {
				if c.Kept == "" {
					continue
				}

				if !slices.Contains(tt.exclude, strings.TrimPrefix(c.Kept, "--exclude ")) {
					t.Errorf("%s kept by %q, not one of %v", c.Path, c.Kept, tt.exclude)
				}
			}

			if !reflect.DeepEqual(paths, tt.want) {
				t.Fatalf("candidates = %v, want %v", paths, tt.want)
			}
		})
	}
}

func toDelete(all []Candidate) []Candidate {
	out := []Candidate{}

	for _, c := range all {
		if c.Kept == "" {
			out = append(out, c)
		}
	}

	return out
}

func TestPlanOnly(t *testing.T) {
	tests := []struct {
		name string
		only []string
		want []string
		kept string
	}{
		{"by shoot directory", []string{"*luchon*"},
			[]string{"2025/20250804_luchon/_1310726.RW2", "2025/20250804_luchon/_1310752.RW2"}, ""},
		{"by stem", []string{"_1310726"}, []string{"2025/20250804_luchon/_1310726.RW2"}, "not in --only _1310726"},
		{"no match keeps everything", []string{"*paris*"}, []string{}, "not in --only *paris*"},
		{"exclude wins over only", []string{"*luchon*"}, []string{"2025/20250804_luchon/_1310752.RW2"}, "--exclude _1310726"},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			fs := newTree(t, rawTherapeeLayout)

			opts := Options{Pattern: "*.rw2", Recursive: true, Only: tt.only}
			if tt.name == "exclude wins over only" {
				opts.Exclude = []string{"_1310726"}
			}

			all, err := Plan(context.Background(), fs, "2025", opts)
			if err != nil {
				t.Fatal(err)
			}

			paths := []string{}
			for _, c := range toDelete(all) {
				paths = append(paths, c.Path)
			}

			if !reflect.DeepEqual(paths, tt.want) {
				t.Fatalf("candidates = %v, want %v", paths, tt.want)
			}

			for _, c := range all {
				if c.Kept != "" && c.Kept != tt.kept {
					t.Errorf("%s kept: %q, want %q", c.Path, c.Kept, tt.kept)
				}
			}
		})
	}
}
