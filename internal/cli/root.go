package cli

import (
	"fmt"
	"io"
	"path/filepath"

	"github.com/spf13/afero"
	"github.com/spf13/cobra"
)

// Set at build time with -ldflags "-X .../internal/cli.version=...".
var version = "dev"

type app struct {
	fs        afero.Fs
	stdout    io.Writer
	stderr    io.Writer
	apply     bool
	recursive bool
	verbose   bool
}

func Root(fs afero.Fs, stdout, stderr io.Writer) *cobra.Command {
	a := &app{fs: fs, stdout: stdout, stderr: stderr}

	root := &cobra.Command{
		Use:           "wba",
		Short:         "White borders, smaller copies and cleanup of undeveloped raws",
		SilenceErrors: true,
		PersistentPreRun: func(cmd *cobra.Command, _ []string) {
			// Flags are parsed by now: a failure from here on is a runtime
			// error, and repeating the usage block would bury it.
			cmd.SilenceUsage = true
		},
	}
	root.SetOut(stdout)
	root.SetErr(stderr)

	flags := root.PersistentFlags()
	flags.BoolVar(&a.apply, "apply", false, "do it; without this flag only print what would be done")
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

// skipWriter is where the packages report what they skipped; nil keeps them quiet.
func (a *app) skipWriter() io.Writer {
	if a.verbose {
		return a.stdout
	}

	return nil
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

// Output lines are best effort: a closed stdout must not turn a successful
// run into a failure, so write errors are dropped on purpose.
func (a *app) out(format string, args ...any) {
	_, _ = fmt.Fprintf(a.stdout, format+"\n", args...)
}

func (a *app) fail(format string, args ...any) {
	_, _ = fmt.Fprintf(a.stderr, format+"\n", args...)
}
