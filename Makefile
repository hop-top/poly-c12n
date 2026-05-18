.PHONY: build test lint check clean fmt vet \
  build-rust build-go test-rust test-rs-sdk test-go test-py

# Default: full polyglot build + test
build: build-rust build-go
test: test-rust test-rs-sdk test-go test-py
check: lint test

# Rust engine (core/) — produces libc12n_core.{dylib,so,dll}
build-rust:
	cargo build --workspace

test-rust:
	cargo test -p hop-top-c12n-core

# Rust SDK (rs/)
test-rs-sdk:
	cargo test -p hop-top-c12n

# Go bindings (go/) — cgo path links libc12n_core
build-go: build-rust
	cd go && CGO_LDFLAGS="-L$$(pwd)/../target/debug" go build ./...

test-go: build-rust
	cd go && CGO_LDFLAGS="-L$$(pwd)/../target/debug" \
	  DYLD_LIBRARY_PATH="$$(pwd)/../target/debug" \
	  go test -race -count=1 ./...

# Go stub path (no Rust required)
test-go-stub:
	cd go && CGO_ENABLED=0 go test -race -count=1 ./...

# Python bindings — pure-Python tests
test-py:
	@command -v pytest >/dev/null 2>&1 \
		&& PYTHONPATH=py/python pytest -q py/tests/ \
		|| echo "pytest not found; skipping Python tests"

# Linters
fmt:
	cargo fmt --all
	cd go && gofmt -w .

vet:
	cd go && go vet ./...

lint: vet
	cargo clippy --workspace -- -D warnings
	@if [ -n "$$(cd go && gofmt -l .)" ]; then \
		echo "gofmt needed:"; cd go && gofmt -l .; exit 1; \
	fi

clean:
	cargo clean
	cd go && go clean
