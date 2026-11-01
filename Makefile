BINARY  := wba
VERSION := $(shell git describe --tags --always --dirty 2>/dev/null || echo dev)
LDFLAGS := -ldflags "-s -w -X github.com/evaneecloo/white_border_adder/internal/cli.version=$(VERSION)"

.PHONY: lint test build build-windows clean

lint:
	golangci-lint run ./...

test:
	go test ./...

build:
	go build $(LDFLAGS) -o $(BINARY) ./cmd/wba

build-windows:
	GOOS=windows GOARCH=amd64 go build $(LDFLAGS) -o $(BINARY).exe ./cmd/wba

clean:
	rm -f $(BINARY) $(BINARY).exe
