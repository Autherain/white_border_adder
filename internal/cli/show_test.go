package cli

import (
	"bytes"
	"context"
	"io"
	"log/slog"
	"slices"
	"testing"

	"github.com/evaneecloo/white_border_adder/internal/log"
)

func testApp(apply bool) (*app, context.Context, *bytes.Buffer) {
	var out bytes.Buffer

	a := &app{apply: apply, stdout: &out, stderr: io.Discard}
	ctx := log.With(context.Background(), log.New(&out, io.Discard, slog.LevelInfo))

	return a, ctx, &out
}

func TestShow(t *testing.T) {
	t.Parallel()

	oneDir := []item{
		{files: []string{"IMG_1264.RW2", "IMG_1264.RW2.pp3"}, size: 24_012_000},
		{files: []string{"IMG_1271.RW2"}, size: 25_000_000},
	}
	twoDirs := []item{
		{dir: "paris", files: []string{"paris/IMG_1264.RW2", "paris/IMG_1264.RW2.pp3"}, size: 24_012_000},
		{dir: "lyon", files: []string{"lyon/IMG_2000.RW2"}, size: 1_000_000},
		{dir: "paris", files: []string{"paris/IMG_1271.RW2"}, size: 25_000_000},
	}
	noSizes := []item{{files: []string{"bordered/bordered_a.jpg"}}, {files: []string{"bordered/bordered_b.jpg"}}}
	warned := []item{
		{files: []string{"IMG_1264.RW2", "IMG_1264.RW2.pp3"}, size: 24_012_000, warn: "has a .pp3 but nothing in converted/"},
		{files: []string{"IMG_1271.RW2"}, size: 25_000_000},
	}

	tests := []struct {
		name  string
		verb  string
		items []item
		want  string
	}{
		{"one directory", verbDelete, oneDir, "would delete IMG_1264.RW2\n" +
			"would delete IMG_1264.RW2.pp3\n" +
			"would delete IMG_1271.RW2\n" +
			"3 files, 49 MB. Re-run with --apply to delete them.\n"},
		{"banner per directory", verbDelete, twoDirs, "####### paris: 3 files, 49 MB\n" +
			"would delete paris/IMG_1264.RW2\n" +
			"would delete paris/IMG_1264.RW2.pp3\n" +
			"would delete paris/IMG_1271.RW2\n" +
			"####### lyon: 1 file, 1.0 MB\n" +
			"would delete lyon/IMG_2000.RW2\n" +
			"4 files, 50 MB. Re-run with --apply to delete them.\n"},
		{"no sizes", verbWrite, noSizes, "would write bordered/bordered_a.jpg\n" +
			"would write bordered/bordered_b.jpg\n" +
			"2 files. Re-run with --apply to write them.\n"},
		{"warning banner", verbDelete, warned, "would delete IMG_1264.RW2  ####### has a .pp3 but nothing in converted/\n" +
			"would delete IMG_1264.RW2.pp3\n" +
			"would delete IMG_1271.RW2\n" +
			"3 files, 49 MB. Re-run with --apply to delete them.\n"},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			t.Parallel()

			a, ctx, out := testApp(false)

			if chosen := a.show(ctx, tt.verb, tt.items); chosen != nil {
				t.Fatalf("dry run chose %v", chosen)
			}

			if out.String() != tt.want {
				t.Fatalf("output:\n%s\nwant:\n%s", out.String(), tt.want)
			}
		})
	}
}

func TestShowApply(t *testing.T) {
	t.Parallel()

	a, ctx, out := testApp(true)

	chosen := a.show(ctx, verbDelete, []item{{files: []string{"a"}}, {files: []string{"b"}}, {files: []string{"c"}}})
	if !slices.Equal(chosen, []int{0, 1, 2}) || out.Len() != 0 {
		t.Fatalf("apply: chosen = %v, output = %q", chosen, out.String())
	}
}
