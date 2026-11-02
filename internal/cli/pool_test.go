package cli

import (
	"slices"
	"testing"
)

func TestRunEachReportsEveryResult(t *testing.T) {
	t.Parallel()

	got := []int{}

	runEach(4, []int{1, 2, 3, 4, 5}, func(n int) int { return n * n }, func(r int) { got = append(got, r) })

	slices.Sort(got)

	if !slices.Equal(got, []int{1, 4, 9, 16, 25}) {
		t.Errorf("got %v", got)
	}
}
