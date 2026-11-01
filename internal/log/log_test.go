package log

import (
	"bytes"
	"context"
	"log/slog"
	"testing"
)

func TestLevels(t *testing.T) {
	t.Parallel()

	for _, verbose := range []bool{false, true} {
		var out, errOut bytes.Buffer

		level := slog.LevelInfo
		if verbose {
			level = slog.LevelDebug
		}

		ctx := With(context.Background(), New(&out, &errOut, level))
		Debugf(ctx, "skip %s", "a")
		Infof(ctx, "would delete %s", "b")
		Errorf(ctx, "%s: boom", "c")

		wantOut := "would delete b\n"
		if verbose {
			wantOut = "skip a\n" + wantOut
		}

		if out.String() != wantOut {
			t.Errorf("verbose=%v stdout = %q, want %q", verbose, out.String(), wantOut)
		}

		if errOut.String() != "####### c: boom\n" {
			t.Errorf("verbose=%v stderr = %q", verbose, errOut.String())
		}
	}
}

func TestFromWithoutLoggerIsSilent(t *testing.T) {
	t.Parallel()

	Infof(context.Background(), "lost")
}
