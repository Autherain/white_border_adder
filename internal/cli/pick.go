package cli

import (
	"context"
	"fmt"
	"strings"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/evaneecloo/white_border_adder/internal/log"
)

const (
	verbDelete = "delete"
	verbWrite  = "write"
)

// An item is one planned action: the files it touches, relative to the
// argument, and the bytes involved.
type item struct {
	files []string
	size  int64
}

// A picker shows the plan and returns the indexes of the items to run.
// Without --apply it returns none: showing the plan is the whole job.
type picker interface {
	pick(ctx context.Context, verb string, items []item) ([]int, error)
}

func (a *app) picker() picker {
	if a.plain || !isTerminal(a.stdin) || !isTerminal(a.stdout) {
		return plainPicker{a: a}
	}

	return teaPicker{a: a}
}

// plainPicker prints one line per file and never asks anything.
type plainPicker struct{ a *app }

func (p plainPicker) pick(ctx context.Context, verb string, items []item) ([]int, error) {
	if p.a.apply {
		return everything(len(items)), nil
	}

	for _, it := range items {
		for _, f := range it.files {
			log.Infof(ctx, "%s %s", styleWould.Render("would "+verb), f)
		}
	}

	log.Infof(ctx, "%s Re-run with --apply to %s them.", styleBold.Render(summary(items)+"."), verb)

	return nil, nil
}

// teaPicker shows the plan as a list; with --apply the list is a checklist
// and enter runs what is checked.
type teaPicker struct{ a *app }

func (p teaPicker) pick(ctx context.Context, verb string, items []item) ([]int, error) {
	program := tea.NewProgram(newChecklist(verb, items, p.a.apply),
		tea.WithInput(p.a.stdin), tea.WithOutput(p.a.stdout), tea.WithContext(ctx))

	final, err := program.Run()
	if err != nil {
		return nil, fmt.Errorf("interactive list: %w", err)
	}

	m, ok := final.(checklist)
	if !ok || !m.confirmed {
		if p.a.apply {
			log.Infof(ctx, "%s", styleFaint.Render("nothing done"))
		}

		return nil, nil
	}

	return m.chosen(), nil
}

func everything(n int) []int {
	idx := make([]int, n)
	for i := range idx {
		idx[i] = i
	}

	return idx
}

func pickFrom[T any](all []T, idx []int) []T {
	out := make([]T, 0, len(idx))
	for _, i := range idx {
		out = append(out, all[i])
	}

	return out
}

// summary is "3 files, 49 MB", or just the count when sizes are unknown.
func summary(items []item) string {
	n, total := 0, int64(0)

	for _, it := range items {
		n += len(it.files)
		total += it.size
	}

	if total == 0 {
		return files(n)
	}

	return files(n) + ", " + human(total)
}

type checklist struct {
	verb      string
	apply     bool
	items     []item
	checked   []bool
	cursor    int
	confirmed bool
}

func newChecklist(verb string, items []item, apply bool) checklist {
	checked := make([]bool, len(items))
	for i := range checked {
		checked[i] = true
	}

	return checklist{verb: verb, apply: apply, items: items, checked: checked}
}

func (checklist) Init() tea.Cmd { return nil }

func (m checklist) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	key, ok := msg.(tea.KeyMsg)
	if !ok {
		return m, nil
	}

	switch key.String() {
	case "ctrl+c", "q", "esc":
		return m, tea.Quit
	case "enter":
		m.confirmed = m.apply

		return m, tea.Quit
	case "up", "k":
		m.cursor = max(m.cursor-1, 0)
	case "down", "j":
		m.cursor = min(m.cursor+1, len(m.items)-1)
	default:
		m.toggle(key.String())
	}

	return m, nil
}

// toggle only answers in apply mode: a dry run has nothing to choose.
func (m checklist) toggle(key string) {
	if !m.apply {
		return
	}

	switch key {
	case " ":
		m.checked[m.cursor] = !m.checked[m.cursor]
	case "a":
		m.toggleAll()
	default:
	}
}

func (m checklist) toggleAll() {
	all := true
	for _, c := range m.checked {
		all = all && c
	}

	for i := range m.checked {
		m.checked[i] = !all
	}
}

func (m checklist) chosen() []int {
	idx := []int{}

	for i, c := range m.checked {
		if c {
			idx = append(idx, i)
		}
	}

	return idx
}

func (m checklist) View() string {
	var b strings.Builder

	b.WriteString(styleBold.Render(m.title()) + "\n\n")

	for i, it := range m.items {
		b.WriteString(m.line(i, it) + "\n")
	}

	b.WriteString("\n" + styleFaint.Render(m.help()) + "\n")

	return b.String()
}

func (m checklist) title() string {
	if m.apply {
		return "Choose what to " + m.verb
	}

	return "Dry run: would " + m.verb + " " + summary(m.items)
}

func (m checklist) line(i int, it item) string {
	cursor := "  "
	if i == m.cursor {
		cursor = "> "
	}

	label := strings.Join(it.files, " + ")
	if it.size > 0 {
		label += styleFaint.Render("  " + human(it.size))
	}

	if !m.apply {
		return cursor + label
	}

	if m.checked[i] {
		return cursor + accent(m.verb).Render("[x]") + " " + label
	}

	return cursor + styleFaint.Render("[ ] "+label)
}

func (m checklist) help() string {
	if !m.apply {
		return "nothing will be " + m.verb + "d · q quit · re-run with --apply to choose and " + m.verb
	}

	return fmt.Sprintf("space toggle · a all/none · enter %s %s · q quit",
		m.verb, summary(pickFrom(m.items, m.chosen())))
}
