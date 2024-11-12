BINARY_NAME=wbi
VERSION?=1.0.0
BUILD_DIR=build
LDFLAGS=-ldflags "-X main.Version=${VERSION}"

# Supported platforms
PLATFORMS=windows linux darwin
WINDOWS_BIN=$(BINARY_NAME).exe
LINUX_BIN=$(BINARY_NAME)_linux
DARWIN_BIN=$(BINARY_NAME)_darwin

.PHONY: all build clean test help windows linux darwin cross-platform

# Default target
all: clean cross-platform

help:
	@echo "Available commands:"
	@echo "make build         - Build for current OS"
	@echo "make windows      - Build for Windows"
	@echo "make linux        - Build for Linux"
	@echo "make darwin       - Build for macOS"
	@echo "make clean        - Remove build artifacts"
	@echo "make test         - Run tests"
	@echo "make all          - Clean and build for all platforms"
	@echo "make cross-platform - Build for all platforms"

clean:
	rm -rf ${BUILD_DIR}
	go clean

test:
	go test ./...

build:
	go build ${LDFLAGS} -o ${BUILD_DIR}/${BINARY_NAME} main.go

# Individual platform builds
windows:
	mkdir -p ${BUILD_DIR}
	GOOS=windows GOARCH=amd64 go build ${LDFLAGS} -o ${BUILD_DIR}/${WINDOWS_BIN} main.go

linux:
	mkdir -p ${BUILD_DIR}
	GOOS=linux GOARCH=amd64 go build ${LDFLAGS} -o ${BUILD_DIR}/${LINUX_BIN} main.go

darwin:
	mkdir -p ${BUILD_DIR}
	GOOS=darwin GOARCH=amd64 go build ${LDFLAGS} -o ${BUILD_DIR}/${DARWIN_BIN} main.go

# Build for all platforms
cross-platform: windows linux darwin
	@echo "Built binaries for all platforms:"
	@ls -l ${BUILD_DIR}/
