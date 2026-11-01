package cli

import (
	"bytes"
	"context"
	"io"
	"log/slog"
	"slices"
	"strings"
	"testing"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/x/exp/teatest"

	"github.com/evaneecloo/white_border_adder/internal/log"
)

var demo = []item{
	{files: []string{"IMG_1264.RW2", "IMG_1264.RW2.pp3"}, size: 24_012_000},
	{files: []string{"IMG_1271.RW2"}, size: 25_000_000},
	{files: []string{"IMG_1300.RW2"}, size: 1_000},
}

func key(name string) tea.KeyMsg {
	switch name {
	case "space":
		return tea.KeyMsg{Type: tea.KeySpace}
	case "enter":
		return tea.KeyMsg{Type: tea.KeyEnter}
	case "down":
		return tea.KeyMsg{Type: tea.KeyDown}
	case "up":
		return tea.KeyMsg{Type: tea.KeyUp}
	case "esc":
		return tea.KeyMsg{Type: tea.KeyEsc}
	default:
		return tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune(name)}
	}
}

func press(t *testing.T, m checklist, keys ...string) checklist {
	t.Helper()

	for _, k := range keys {
		next, _ := m.Update(key(k))

		var ok bool
		if m, ok = next.(checklist); !ok {
			t.Fatalf("Update returned %T", next)
		}
	}

	return m
}

func TestChecklistKeys(t *testing.T) {
	t.Parallel()

	tests := []struct {
		name      string
		apply     bool
		keys      []string
		confirmed bool
		chosen    []int
	}{
		{"enter keeps everything", true, []string{"enter"}, true, []int{0, 1, 2}},
		{"space unchecks the first", true, []string{"space", "enter"}, true, []int{1, 2}},
		{"down moves the cursor", true, []string{"down", "space", "enter"}, true, []int{0, 2}},
		{"cursor stops at the ends", true, []string{"up", "down", "down", "down", "down", "space", "enter"}, true, []int{0, 1}},
		{"a unchecks all", true, []string{"a", "enter"}, true, []int{}},
		{"a twice rechecks all", true, []string{"a", "a", "enter"}, true, []int{0, 1, 2}},
		{"q never confirms", true, []string{"space", "q"}, false, nil},
		{"esc never confirms", true, []string{"esc"}, false, nil},
		{"dry run ignores space and a", false, []string{"space", "a", "enter"}, false, nil},
		{"dry run q", false, []string{"q"}, false, nil},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			t.Parallel()

			m := press(t, newChecklist(verbDelete, demo, tt.apply), tt.keys...)

			if m.confirmed != tt.confirmed {
				t.Fatalf("confirmed = %v, want %v", m.confirmed, tt.confirmed)
			}

			if tt.confirmed && !slices.Equal(m.chosen(), tt.chosen) {
				t.Fatalf("chosen = %v, want %v", m.chosen(), tt.chosen)
			}
		})
	}
}

func TestChecklistView(t *testing.T) {
	t.Parallel()

	apply := newChecklist(verbDelete, demo, true)
	for _, want := range []string{
		"Choose what to delete",
		"> [x] IMG_1264.RW2 + IMG_1264.RW2.pp3  24 MB",
		"  [x] IMG_1271.RW2  25 MB",
		"enter delete 4 files, 49 MB",
	} {
		if !strings.Contains(apply.View(), want) {
			t.Errorf("apply view lacks %q:\n%s", want, apply.View())
		}
	}

	unchecked := press(t, apply, "space")
	if !strings.Contains(unchecked.View(), "> [ ] IMG_1264.RW2") ||
		!strings.Contains(unchecked.View(), "enter delete 2 files, 25 MB") {
		t.Errorf("view after space:\n%s", unchecked.View())
	}

	dry := newChecklist(verbWrite, demo[:1], false)
	if !strings.Contains(dry.View(), "Dry run: would write 2 files, 24 MB") ||
		strings.Contains(dry.View(), "[") ||
		!strings.Contains(dry.View(), "re-run with --apply") {
		t.Errorf("dry view:\n%s", dry.View())
	}
}

func testApp(apply bool) (*app, context.Context, *bytes.Buffer) {
	var out bytes.Buffer

	a := &app{apply: apply, plain: true, stdout: &out, stderr: io.Discard}
	ctx := log.With(context.Background(), log.New(&out, io.Discard, slog.LevelInfo))

	return a, ctx, &out
}

func TestPlainPicker(t *testing.T) {
	t.Parallel()

	a, ctx, out := testApp(false)

	chosen, err := a.picker().pick(ctx, verbDelete, demo)
	if err != nil || chosen != nil {
		t.Fatalf("dry run: chosen = %v, err = %v", chosen, err)
	}

	want := "would delete IMG_1264.RW2\n" +
		"would delete IMG_1264.RW2.pp3\n" +
		"would delete IMG_1271.RW2\n" +
		"would delete IMG_1300.RW2\n" +
		"4 files, 49 MB. Re-run with --apply to delete them.\n"
	if out.String() != want {
		t.Fatalf("dry run output:\n%s\nwant:\n%s", out.String(), want)
	}

	a, ctx, out = testApp(true)

	chosen, err = a.picker().pick(ctx, verbDelete, demo)
	if err != nil || !slices.Equal(chosen, []int{0, 1, 2}) || out.Len() != 0 {
		t.Fatalf("apply: chosen = %v, err = %v, output = %q", chosen, err, out.String())
	}
}

// The model is covered above; this runs the real bubbletea loop once, on a
// pipe, to make sure the keys reach it the way a terminal sends them.
func TestChecklistProgram(t *testing.T) {
	t.Parallel()

	tm := teatest.NewTestModel(t, newChecklist(verbDelete, demo, true), teatest.WithInitialTermSize(80, 24))

	teatest.WaitFor(t, tm.Output(), func(b []byte) bool {
		return bytes.Contains(b, []byte("Choose what to delete"))
	}, teatest.WithDuration(2*time.Second))

	tm.Send(key("space"))
	tm.Send(key("enter"))

	m, ok := tm.FinalModel(t, teatest.WithFinalTimeout(2*time.Second)).(checklist)
	if !ok || !m.confirmed || !slices.Equal(m.chosen(), []int{1, 2}) {
		t.Fatalf("final model: %+v", m)
	}
}
