package sorter

import (
	"context"
	"encoding/binary"
	"os"
	"path/filepath"
	"reflect"
	"sort"
	"testing"
	"time"

	"github.com/spf13/afero"
)

// tiffWith builds the head of a TIFF-like file whose EXIF says it was shot at
// stamp ("2026:06:20 10:00:00"), the way a camera raw lays it out.
func tiffWith(stamp string) string {
	b := make([]byte, 128)
	copy(b, "II")
	binary.LittleEndian.PutUint16(b[2:], 42)
	binary.LittleEndian.PutUint32(b[4:], 8)
	binary.LittleEndian.PutUint16(b[8:], 1)
	e := b[10:]
	binary.LittleEndian.PutUint16(e, tagExifIFD)
	binary.LittleEndian.PutUint16(e[2:], typeLong)
	binary.LittleEndian.PutUint32(e[4:], 1)
	binary.LittleEndian.PutUint32(e[8:], 30)
	binary.LittleEndian.PutUint16(b[30:], 1)
	e = b[32:]
	binary.LittleEndian.PutUint16(e, tagDateTimeOrig)
	binary.LittleEndian.PutUint16(e[2:], typeASCII)
	binary.LittleEndian.PutUint32(e[4:], 20)
	binary.LittleEndian.PutUint32(e[8:], 60)
	copy(b[60:], stamp+"\x00")

	return string(b)
}

func shot(day string) string { return tiffWith("2026:" + day[:2] + ":" + day[2:] + " 10:00:00") }

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

func lines(moves []Move) []string {
	out := []string{}

	for _, m := range moves {
		line := m.From + " -> " + m.To
		if m.Kept != "" {
			line += " [" + m.Kept + "]"
		}

		out = append(out, line)
	}

	sort.Strings(out)

	return out
}

func TestShotDate(t *testing.T) {
	t.Parallel()

	fs := newTree(t, map[string]string{"a.rw2": shot("0620"), "b.png": "no exif"})

	got, ok := shotDate(fs, "a.rw2")
	if want := time.Date(2026, 6, 20, 10, 0, 0, 0, time.Local); !ok || !got.Equal(want) {
		t.Errorf("got %v %v, want %v", got, ok, want)
	}

	if _, ok := shotDate(fs, "b.png"); ok {
		t.Error("a file without EXIF has a shot date")
	}
}

func TestPlan(t *testing.T) {
	t.Parallel()

	tests := []struct {
		name    string
		tree    map[string]string
		sources []string
		opts    Options
		want    []string
	}{
		{
			name:    "a dumped photo goes in the existing folder of its day, place name kept",
			tree:    map[string]string{"r/dump/a.rw2": shot("0620"), "r/2026/20260620_toulouse/b.rw2": shot("0620")},
			sources: []string{"r/dump"},
			want:    []string{"r/dump/a.rw2 -> r/2026/20260620_toulouse/a.rw2"},
		},
		{
			name:    "no folder for that day creates one named after the day",
			tree:    map[string]string{"r/dump/a.rw2": shot("0801")},
			sources: []string{"r/dump"},
			want:    []string{"r/dump/a.rw2 -> r/2026/20260801/a.rw2"},
		},
		{
			name: "a photo in a wrong day folder goes back to its day",
			tree: map[string]string{
				"r/2026/20260620_toulouse/a.rw2": shot("0701"), "r/2026/20260701_lyon/b.rw2": shot("0701"),
			},
			opts: Options{Recursive: true},
			want: []string{"r/2026/20260620_toulouse/a.rw2 -> r/2026/20260701_lyon/a.rw2"},
		},
		{
			name: "a photo in the right folder is left alone, even below a subfolder",
			tree: map[string]string{"r/2026/20260620_toulouse/a.rw2": shot("0620"), "r/2026/20260620_toulouse/sub/b.rw2": shot("0620")},
			opts: Options{Recursive: true},
			want: []string{},
		},
		{
			name: "the old layout without a year folder keeps its folder name",
			tree: map[string]string{"r/20260620_toulouse/a.rw2": shot("0620")},
			opts: Options{Recursive: true},
			want: []string{"r/20260620_toulouse/a.rw2 -> r/2026/20260620_toulouse/a.rw2"},
		},
		{
			name:    "the sidecar goes with its raw",
			tree:    map[string]string{"r/d/a.rw2": shot("0801"), "r/d/a.rw2.pp3": "pp3"},
			sources: []string{"r/d"},
			want:    []string{"r/d/a.rw2 -> r/2026/20260801/a.rw2"},
		},
		{
			name: "converted and bordered are never read",
			tree: map[string]string{
				"r/d/converted/a.jpg": shot("0801"), "r/d/bordered/b.jpg": shot("0801"),
				"r/d/bordered_images/c.jpg": shot("0801"), "r/d/.hidden/e.jpg": shot("0801"),
			},
			opts: Options{Recursive: true},
			want: []string{},
		},
		{
			name:    "never replaces a photo that already exists at the target",
			tree:    map[string]string{"r/dump/a.rw2": shot("0620"), "r/2026/20260620_toulouse/a.rw2": "mine"},
			sources: []string{"r/dump"},
			want:    []string{"r/dump/a.rw2 -> r/2026/20260620_toulouse/a.rw2 [already exists in 20260620_toulouse]"},
		},
		{
			name:    "two photos with the same name do not collide",
			tree:    map[string]string{"r/dump/a/IMG_1.RW2": shot("0801"), "r/dump/b/IMG_1.RW2": shot("0801")},
			sources: []string{"r/dump"},
			opts:    Options{Recursive: true},
			want: []string{
				"r/dump/a/IMG_1.RW2 -> r/2026/20260801/IMG_1.RW2",
				"r/dump/b/IMG_1.RW2 -> r/2026/20260801/IMG_1.RW2 [same name as r/dump/a/IMG_1.RW2]",
			},
		},
		{
			name:    "several folders for one day keep the photo and say so",
			tree:    map[string]string{"r/dump/a.rw2": shot("0620"), "r/2026/20260620_a/x.rw2": shot("0620"), "r/2026/20260620_b/y.rw2": shot("0620")},
			sources: []string{"r/dump"},
			want:    []string{"r/dump/a.rw2 ->  [2 folders for 20260620 in 2026/: 20260620_a, 20260620_b]"},
		},
		{
			name:    "not recursive stays in the directory",
			tree:    map[string]string{"r/dump/a.rw2": shot("0801"), "r/dump/sub/b.rw2": shot("0801")},
			sources: []string{"r/dump"},
			want:    []string{"r/dump/a.rw2 -> r/2026/20260801/a.rw2"},
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			t.Parallel()

			moves, err := Plan(context.Background(), newTree(t, tt.tree), "r", tt.sources, tt.opts)
			if err != nil {
				t.Fatal(err)
			}

			if got := lines(moves); !reflect.DeepEqual(got, tt.want) {
				t.Errorf("got\n%q\nwant\n%q", got, tt.want)
			}
		})
	}
}

func TestConvertedFollowsItsOriginal(t *testing.T) {
	t.Parallel()

	fs := newTree(t, map[string]string{
		"r/d/IMG_1.RW2": shot("0801"), "r/d/IMG_1.RW2.pp3": "pp3",
		"r/d/converted/IMG_1.jpg": "a", "r/d/converted/img_1-2.PNG": "b",
		"r/d/converted/IMG_10.jpg": "other", "r/d/bordered/bordered_IMG_1.jpg": "never",
	})

	moves, err := Plan(context.Background(), fs, "r", []string{"r/d"}, Options{})
	if err != nil {
		t.Fatal(err)
	}

	if len(moves) != 1 {
		t.Fatalf("got %d moves, want 1 (IMG_10 has no original here)", len(moves))
	}

	if err := moves[0].Run(fs); err != nil {
		t.Fatal(err)
	}

	got := map[string]bool{}

	_ = afero.Walk(fs, "r", func(p string, info os.FileInfo, _ error) error {
		if !info.IsDir() {
			got[p] = true
		}

		return nil
	})

	want := map[string]bool{
		"r/2026/20260801/IMG_1.RW2": true, "r/2026/20260801/IMG_1.RW2.pp3": true,
		"r/2026/20260801/converted/IMG_1.jpg": true, "r/2026/20260801/converted/img_1-2.PNG": true,
		"r/d/converted/IMG_10.jpg": true, "r/d/bordered/bordered_IMG_1.jpg": true,
	}
	if !reflect.DeepEqual(got, want) {
		t.Errorf("got %v\nwant %v", got, want)
	}
}

func TestConvertedNeverOverwrites(t *testing.T) {
	t.Parallel()

	fs := newTree(t, map[string]string{
		"r/d/IMG_1.RW2": shot("0801"), "r/d/converted/IMG_1.jpg": "new",
		"r/2026/20260801/converted/IMG_1.jpg": "mine",
	})

	moves, err := Plan(context.Background(), fs, "r", []string{"r/d"}, Options{})
	if err != nil {
		t.Fatal(err)
	}

	want := []string{"r/d/IMG_1.RW2 -> r/2026/20260801/IMG_1.RW2 [already exists in 20260801]"}
	if got := lines(moves); !reflect.DeepEqual(got, want) {
		t.Errorf("got %q, want %q", got, want)
	}
}

func TestRunFromInsideAYearFolder(t *testing.T) {
	t.Parallel()

	fs := newTree(t, map[string]string{
		"r/2026/20260620_toulouse/a.rw2": shot("0620"),
		"r/2026/20260620_toulouse/b.rw2": tiffWith("2025:12:24 10:00:00"),
	})

	moves, err := Plan(context.Background(), fs, "r/2026", nil, Options{Recursive: true})
	if err != nil {
		t.Fatal(err)
	}

	want := []string{"r/2026/20260620_toulouse/b.rw2 -> r/2025/20251224/b.rw2"}
	if got := lines(moves); !reflect.DeepEqual(got, want) {
		t.Errorf("got %q, want %q", got, want)
	}
}

func TestExcludeLeavesPhotosAndDirectoriesAlone(t *testing.T) {
	t.Parallel()

	fs := newTree(t, map[string]string{
		"r/dump/DSC_0001.NEF": shot("0801"), "r/dump/IMG_1.RW2": shot("0801"),
		"r/dump/Nikon old/b.jpg": shot("0801"), "r/dump/DSC_0001.NEF.pp3": "pp3",
	})

	moves, err := Plan(context.Background(), fs, "r", []string{"r/dump"},
		Options{Recursive: true, Exclude: []string{"dsc_*", "*nikon*"}})
	if err != nil {
		t.Fatal(err)
	}

	want := []string{"r/dump/IMG_1.RW2 -> r/2026/20260801/IMG_1.RW2"}
	if got := lines(moves); !reflect.DeepEqual(got, want) {
		t.Errorf("got %q, want %q", got, want)
	}
}

func TestExcludeRejectsAMalformedGlob(t *testing.T) {
	t.Parallel()

	_, err := Plan(context.Background(), newTree(t, map[string]string{"r/a.jpg": ""}), "r", nil, Options{Exclude: []string{"["}})
	if err == nil {
		t.Error("expected an error")
	}
}

func TestPlanNeverWrites(t *testing.T) {
	t.Parallel()

	fs := newTree(t, map[string]string{"r/dump/a.rw2": shot("0801")})

	if _, err := Plan(context.Background(), fs, "r", []string{"r/dump"}, Options{}); err != nil {
		t.Fatal(err)
	}

	if ok, _ := afero.DirExists(fs, "r/2026"); ok {
		t.Error("planning created a directory")
	}
}

func TestRunMovesRawAndSidecarAndNeverOverwrites(t *testing.T) {
	t.Parallel()

	fs := newTree(t, map[string]string{"r/dump/a.rw2": shot("0801"), "r/dump/a.rw2.pp3": "pp3"})

	moves, err := Plan(context.Background(), fs, "r", []string{"r/dump"}, Options{})
	if err != nil {
		t.Fatal(err)
	}

	if err := moves[0].Run(fs); err != nil {
		t.Fatal(err)
	}

	for _, p := range []string{"r/2026/20260801/a.rw2", "r/2026/20260801/a.rw2.pp3"} {
		if ok, _ := afero.Exists(fs, p); !ok {
			t.Errorf("%s missing", p)
		}
	}

	if err := afero.WriteFile(fs, "r/dump/a.rw2", []byte("new"), 0o644); err != nil {
		t.Fatal(err)
	}

	if err := (Move{From: "r/dump/a.rw2", To: "r/2026/20260801/a.rw2"}).Run(fs); err == nil {
		t.Error("Run replaced an existing file")
	}

	if b, _ := afero.ReadFile(fs, "r/2026/20260801/a.rw2"); string(b) != shot("0801") {
		t.Error("the existing file changed")
	}
}
