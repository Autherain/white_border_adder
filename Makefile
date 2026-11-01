BINARY := wba

.PHONY: lint test build clean

lint:
	golangci-lint run ./...

test:
	go test ./...

build:
	go build -o $(BINARY) .

clean:
	rm -f $(BINARY)
