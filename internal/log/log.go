// Package log carries a line logger in the context so that packages print
// what they skipped without being handed a writer.
package log

import (
	"context"
	"fmt"
	"io"
	"log/slog"

	"github.com/charmbracelet/lipgloss"
)

type ctxKey struct{}

var (
	styleDebug = lipgloss.NewStyle().Faint(true)
	styleError = lipgloss.NewStyle().Foreground(lipgloss.Color("1"))
)

// New returns a logger that prints bare lines from level up: info on out,
// errors on errOut. No timestamps, no levels in the text.
func New(out, errOut io.Writer, level slog.Level) *slog.Logger {
	return slog.New(handler{out: out, errOut: errOut, level: level})
}

func With(ctx context.Context, l *slog.Logger) context.Context {
	return context.WithValue(ctx, ctxKey{}, l)
}

// From returns the context's logger, or one that prints nothing so that a
// package called without a logger still works.
func From(ctx context.Context) *slog.Logger {
	if l, ok := ctx.Value(ctxKey{}).(*slog.Logger); ok {
		return l
	}

	return slog.New(handler{out: io.Discard, errOut: io.Discard, level: slog.LevelError + 1})
}

func Debugf(ctx context.Context, format string, args ...any) {
	From(ctx).DebugContext(ctx, fmt.Sprintf(format, args...))
}

func Infof(ctx context.Context, format string, args ...any) {
	From(ctx).InfoContext(ctx, fmt.Sprintf(format, args...))
}

func Errorf(ctx context.Context, format string, args ...any) {
	From(ctx).ErrorContext(ctx, fmt.Sprintf(format, args...))
}

type handler struct {
	out    io.Writer
	errOut io.Writer
	level  slog.Level
}

func (h handler) Enabled(_ context.Context, level slog.Level) bool {
	return level >= h.level
}

func (h handler) Handle(_ context.Context, r slog.Record) error {
	line, w := r.Message, h.out

	switch {
	case r.Level >= slog.LevelError:
		line, w = styleError.Render(line), h.errOut
	case r.Level <= slog.LevelDebug:
		line = styleDebug.Render(line)
	default:
	}

	// A closed stdout must not turn a successful run into a failure.
	_, _ = fmt.Fprintln(w, line)

	return nil
}

// Attributes and groups are dropped: the message is the whole line.
func (h handler) WithAttrs(_ []slog.Attr) slog.Handler { return h }

func (h handler) WithGroup(_ string) slog.Handler { return h }
