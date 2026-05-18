.PHONY: build test lint check clean fmt vet build-rust build-go test-rust test-go test-py

# Default: full polyglot build + test
build: build-rust build-go
test: test-rust test-go test-py
check: lint test

# Rust core (c12n-core/) — produces libc12n_core.{dylib,so}
build-rust:
	cargo build --workspace

test-rust:
	cargo test -p c12n-core

# Go bindings — cgo path links libc12n_core
build-go: build-rust
	CGO_LDFLAGS="-L$$(pwd)/target/debug" go build ./...

test-go: build-rust
	CGO_LDFLAGS="-L$$(pwd)/target/debug" \
	DYLD_LIBRARY_PATH="$$(pwd)/target/debug" \
	go test -race -count=1 ./...

# Go stub path (no Rust required)
test-go-stub:
	CGO_ENABLED=0 go test -race -count=1 ./...

# Python bindings — pure-Python tests (native cdylib tests gated by markers)
test-py:
	@command -v pytest >/dev/null 2>&1 \
		&& PYTHONPATH=c12n-py/python pytest -q c12n-py/tests/ \
		|| echo "pytest not found; skipping Python tests"

# Linters
fmt:
	cargo fmt --all
	gofmt -w .

vet:
	go vet ./...

lint: vet
	cargo clippy --workspace -- -D warnings
	@if [ -n "$$(gofmt -l .)" ]; then \
		echo "gofmt needed:"; gofmt -l .; exit 1; \
	fi

clean:
	cargo clean
	go clean
