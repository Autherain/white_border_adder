package main

import (
	"fmt"
	"os"

	"github.com/evaneecloo/white_border_adder/internal/cli"
	"github.com/spf13/afero"
)

func main() {
	if err := cli.Root(afero.NewOsFs(), os.Stdout, os.Stderr).Execute(); err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
}
