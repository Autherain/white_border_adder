package cli

import (
	"sync"

	"github.com/alitto/pond/v2"
)

// runEach calls fn on every item from a pool of workers and hands each result
// to done as soon as it is ready, so the caller can print progress while the
// rest is still running. done is called from one goroutine at a time, in
// completion order.
func runEach[T, R any](workers int, items []T, fn func(T) R, done func(R)) {
	if workers < 1 {
		workers = 1
	}

	pool := pond.NewPool(workers)
	results := make(chan R)

	var wg sync.WaitGroup

	wg.Add(len(items))

	for _, item := range items {
		pool.Submit(func() {
			defer wg.Done()

			results <- fn(item)
		})
	}

	go func() {
		wg.Wait()
		close(results)
	}()

	for r := range results {
		done(r)
	}

	pool.StopAndWait()
}
