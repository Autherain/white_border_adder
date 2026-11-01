package cli

import (
	"fmt"
	"io"
	"log/slog"
	"path/filepath"

	"github.com/evaneecloo/white_border_adder/internal/log"
	"github.com/spf13/afero"
	"github.com/spf13/cobra"
	"golang.org/x/term"
)

// Set at build time with -ldflags "-X .../internal/cli.version=...".
var version = "dev"

type app struct {
	fs        afero.Fs
	stdin     io.Reader
	stdout    io.Writer
	stderr    io.Writer
	apply     bool
	plain     bool
	recursive bool
	verbose   bool
}

func Root(fs afero.Fs, stdin io.Reader, stdout, stderr io.Writer) *cobra.Command {
	a := &app{fs: fs, stdin: stdin, stdout: stdout, stderr: stderr}

	root := &cobra.Command{
		Use:           "wba",
		Short:         "White borders, smaller copies and cleanup of undeveloped raws",
		SilenceErrors: true,
		PersistentPreRun: func(cmd *cobra.Command, _ []string) {
			// Flags are parsed by now: a failure from here on is a runtime
			// error, and repeating the usage block would bury it.
			cmd.SilenceUsage = true

			level := slog.LevelInfo
			if a.verbose {
				level = slog.LevelDebug
			}

			cmd.SetContext(log.With(cmd.Context(), log.New(stdout, stderr, level)))
		},
	}
	root.SetOut(stdout)
	root.SetErr(stderr)

	flags := root.PersistentFlags()
	flags.BoolVar(&a.apply, "apply", false, "do it; without this flag nothing is written or deleted")
	flags.BoolVar(&a.plain, "plain", false, "print lines instead of the interactive list; implied outside a terminal")
	flags.BoolVar(&a.recursive, "recursive", false, "walk the tree below <dir>")
	flags.BoolVar(&a.verbose, "verbose", false, "also print skipped items and why")

	root.AddCommand(a.borderCmd(), a.cleanCmd(), a.compressCmd(), versionCmd())

	return root
}

func versionCmd() *cobra.Command {
	return &cobra.Command{
		Use:   "version",
		Short: "Print the version",
		Args:  cobra.NoArgs,
		Run: func(cmd *cobra.Command, _ []string) {
			_, _ = fmt.Fprintln(cmd.OutOrStdout(), version)
		},
	}
}

func isTerminal(stream any) bool {
	f, ok := stream.(interface{ Fd() uintptr })

	return ok && term.IsTerminal(int(f.Fd()))
}

// rel prints paths the way the user typed the argument, not as the fs joined them.
func rel(dir, path string) string {
	r, err := filepath.Rel(dir, path)
	if err != nil {
		return path
	}

	return r
}

func files(n int) string {
	if n == 1 {
		return "1 file"
	}

	return fmt.Sprintf("%d files", n)
}

func failures(n int) error {
	if n == 0 {
		return nil
	}

	return fmt.Errorf("%s failed", files(n))
}

// human rounds to the unit a photographer thinks in: raws are tens of MB.
func human(bytes int64) string {
	const (
		kb = 1000
		mb = 1000 * kb
		gb = 1000 * mb
	)

	switch {
	case bytes >= gb:
		return fmt.Sprintf("%.1f GB", float64(bytes)/gb)
	case bytes >= 10*mb:
		return fmt.Sprintf("%d MB", (bytes+mb/2)/mb)
	case bytes >= mb:
		return fmt.Sprintf("%.1f MB", float64(bytes)/mb)
	case bytes >= kb:
		return fmt.Sprintf("%d kB", (bytes+kb/2)/kb)
	default:
		return fmt.Sprintf("%d B", bytes)
	}
}
