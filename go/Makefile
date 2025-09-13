BINARY_NAME := builder
VERSION     := $(shell git describe --tags --always 2>/dev/null || echo "unknown")
LDFLAGS     := -s -w -X main.version=$(VERSION)
GOOS        ?= linux
GOARCH      ?= amd64

.PHONY: build clean

build:
	@echo "==> Building $(BINARY_NAME) for $(GOOS)/$(GOARCH)..."
	@CGO_ENABLED=0 GOOS=$(GOOS) GOARCH=$(GOARCH) \
	    go build -trimpath \
	    -ldflags "$(LDFLAGS)" \
	    -o $(BINARY_NAME) \
	    main.go
	@echo "✅ Built: $(BINARY_NAME)"

# Clean up
clean:
	rm -f $(BINARY_NAME) pkgbuild-archlinux-*
	@echo "✅ Cleaned up"


