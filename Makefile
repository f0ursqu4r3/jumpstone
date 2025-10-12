.PHONY: fmt lint check test test-metrics

fmt:
	cargo fmt --all

lint:
	cargo clippy --workspace --all-targets --manifest-path backend/Cargo.toml -- -D warnings

check:
	cargo check --workspace --manifest-path backend/Cargo.toml

test:
	cargo test -p openguild-server

test-metrics:
	cargo test -p openguild-server --features metrics
