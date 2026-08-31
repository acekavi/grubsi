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
    ./scripts/check-write-path.sh

test:
    cargo test --workspace
    npm --prefix web run test

build:
    npm --prefix web ci
    npm --prefix web run build
    cargo build --release --package grubsi-server

# Regenerate the TypeScript API client from the server's routes.
gen-api:
    cargo run --quiet --package grubsi-server --bin dump_openapi > openapi.json
    npm --prefix web run gen:api

# Fails if the committed client is stale. Run `just gen-api` to fix.
check-api: gen-api
    git diff --exit-code -- web/src/lib/api/schema.ts
