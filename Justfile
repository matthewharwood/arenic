lint:
    cargo clippy --all-targets --all-features -- -D warnings

gate: lint
