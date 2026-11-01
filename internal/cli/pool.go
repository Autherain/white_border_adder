package cli

import "sync"

// runAll calls fn on every item from a fixed number of goroutines and returns
// the results in completion order, which is the order the user sees them.
func runAll[T, R any](workers int, items []T, fn func(T) R) []R {
	if workers < 1 {
		workers = 1
	}

	queue := make(chan T, len(items))
	results := make(chan R, len(items))

	var wg sync.WaitGroup

	for range workers {
		wg.Add(1)

		go func() {
			defer wg.Done()

			for item := range queue {
				results <- fn(item)
			}
		}()
	}

	for _, item := range items {
		queue <- item
	}

	close(queue)
	wg.Wait()
	close(results)

	out := make([]R, 0, len(items))
	for r := range results {
		out = append(out, r)
	}

	return out
}
