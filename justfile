# Run the API server (port 8080). Pair with `just web` for the dev loop.
dev:
    cargo run --package grubsi-server

# Run the Vite dev server (port 5173, proxies /api and /ws to :8080).
web:
    npm --prefix web run dev

check:
    cargo fmt --all -- --check
    cargo clippy --workspace --all-targets -- -D warnings
    ./scripts/check-core-deps.sh

test:
    cargo test --workspace
    npm --prefix web run test

build:
    npm --prefix web ci
    npm --prefix web run build
    cargo build --release --package grubsi-server
