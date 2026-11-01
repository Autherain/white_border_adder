package cli

import "github.com/alitto/pond/v2"

// runAll calls fn on every item from a pool of workers and returns the
// results in item order, so the printed lines follow the planned list.
func runAll[T, R any](workers int, items []T, fn func(T) R) []R {
	if workers < 1 {
		workers = 1
	}

	pool := pond.NewResultPool[R](workers)
	tasks := make([]pond.ResultTask[R], 0, len(items))

	for _, item := range items {
		tasks = append(tasks, pool.Submit(func() R { return fn(item) }))
	}

	out := make([]R, 0, len(items))

	for _, t := range tasks {
		// fn never panics and returns its own error inside R, so the task
		// error has nothing to add.
		r, _ := t.Wait()
		out = append(out, r)
	}

	pool.StopAndWait()

	return out
}
