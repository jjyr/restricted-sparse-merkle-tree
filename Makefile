default: fmt clippy test

test:
	cargo test --all --all-features

clippy:
	cargo clippy  --all --all-features

fmt:
	cargo fmt --all -- --check
