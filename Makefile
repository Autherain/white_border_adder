BINARY  := wba
VERSION := $(shell git describe --tags --always --dirty 2>/dev/null || echo dev)
LDFLAGS := -ldflags "-X github.com/evaneecloo/white_border_adder/internal/cli.version=$(VERSION)"

.PHONY: lint test build clean

lint:
	golangci-lint run ./...

test:
	go test ./...

build:
	go build $(LDFLAGS) -o $(BINARY) ./cmd/wba

clean:
	rm -f $(BINARY)
