package cli

import "github.com/charmbracelet/lipgloss"

// Colour marks the verb only; the path stays plain so it can be copied.
// lipgloss drops the colour by itself when stdout is not a terminal.
var (
	styleWould = lipgloss.NewStyle().Foreground(lipgloss.Color("3"))
	styleDone  = lipgloss.NewStyle().Foreground(lipgloss.Color("2"))
	styleGone  = lipgloss.NewStyle().Foreground(lipgloss.Color("1"))
	styleFaint = lipgloss.NewStyle().Faint(true)
	styleBold  = lipgloss.NewStyle().Bold(true)
)
