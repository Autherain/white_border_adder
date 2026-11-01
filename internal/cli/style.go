package cli

import "github.com/charmbracelet/lipgloss"

// Colour marks the verb only; the path stays plain so it can be copied.
var (
	styleWould = lipgloss.NewStyle().Foreground(lipgloss.Color("3"))
	styleDone  = lipgloss.NewStyle().Foreground(lipgloss.Color("2"))
	styleGone  = lipgloss.NewStyle().Foreground(lipgloss.Color("1"))
	styleFaint = lipgloss.NewStyle().Faint(true)
	styleBold  = lipgloss.NewStyle().Bold(true)
)

// accent is the colour of a done action: red for a deletion, green for a write.
func accent(verb string) lipgloss.Style {
	if verb == verbDelete {
		return styleGone
	}

	return styleDone
}
