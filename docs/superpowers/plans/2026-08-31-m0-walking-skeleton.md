# M0 Walking Skeleton Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the full-stack skeleton for grubsi — one trivial event travelling from a Rust server through a WebSocket into a React page — with no restaurant features at all.

**Architecture:** A Cargo workspace of three crates (`core` pure domain, `escpos` printer transports, `server` axum+sqlx) plus a Vite/React app in `web/`. In development, Vite serves the frontend on :5173 and proxies `/api` and `/ws` to the Rust server on :8080. In release, `rust-embed` bakes `web/dist` into the binary. Every mutation goes through a `write_tx` helper that makes an audit record a required argument and returns domain events for publication after commit.

**Tech Stack:** Rust 1.97 (edition 2024), tokio 1.53, axum 0.8, sqlx 0.9 (SQLite, WAL), utoipa 5, rust-embed 8, React 19, Vite, TanStack Query, Vitest, SQLite 3.50 STRICT tables.

**Spec:** [`docs/superpowers/specs/2026-08-31-grubsi-architecture-design.md`](../specs/2026-08-31-grubsi-architecture-design.md)

## Global Constraints

Every task's requirements implicitly include this section. Values are copied verbatim from the spec.

- **`crates/core` has no I/O dependencies.** No `sqlx`, `axum`, `tokio`, `hyper`, `tower`, or `reqwest` in its `Cargo.toml` or anywhere in its dependency tree. Task 1 makes this a CI failure, not a convention.
- **All timestamps are stored in UTC**, without exception. Conversion to the restaurant's timezone happens only at the presentation and reporting boundary.
- **Money is integer minor units; rates are integer basis points.** No floating point anywhere in the money path. (No money code lands in M0, but the constraint governs any type introduced here.)
- **Entity primary keys are UUIDv7 stored as `BLOB(16)`.**
- **Every mutation goes through `write_tx`**, which takes an `AuditRecord` as a required argument and returns domain events rather than publishing them.
- **Events publish after commit, never inside a transaction.**
- **The write pool holds exactly one connection.** No CPU-bound or network work may happen inside a write transaction.
- **`AppError` carries a mandatory user-facing message.** Internal detail goes only to `tracing` and is never serialized into a response.
- Pragmas on every connection: `journal_mode=WAL`, `foreign_keys=ON`, `synchronous=NORMAL`, `busy_timeout=5000`.
- All SQLite tables are declared `STRICT`.

## Scope Notes

Two deliberate deviations from the spec, both resolved here so no executor has to guess:

1. **`write_tx` in M0 has the signature `write_tx(db, audit, f)`.** The spec shows a third parameter, `idem: Option<IdempotencyKey>`. M0 has no idempotent operation to exercise it, and a half-built mechanism is worse than none. M4 adds the parameter alongside the first real use (firing a ticket).
2. **M0 creates the `audit_logs` table**, which the spec assigns to M1. `write_tx` cannot exist without it. M1 still owns the audit *features* — the viewer, and every feature actually writing meaningful records.

## File Structure

```
grubsi/
├── Cargo.toml                       workspace manifest, shared dep versions
├── rust-toolchain.toml              pins 1.97
├── justfile                         dev/build/test commands
├── .github/workflows/ci.yml
├── scripts/check-core-deps.sh       fails if core gains an I/O dependency
├── migrations/
│   └── 0001_audit_logs.sql
├── crates/
│   ├── core/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       └── event.rs             DomainEvent, EventKind, Topic
│   ├── escpos/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── sink.rs              TicketSink trait, SinkError
│   │       └── transport/
│   │           ├── mod.rs
│   │           ├── tcp.rs           TcpSink — production, TCP:9100
│   │           └── fake.rs          FakePrinter — test double with failure modes
│   └── server/
│       ├── Cargo.toml
│       ├── src/
│       │   ├── main.rs              bootstrap: tracing, db, hub, LAN bind
│       │   ├── lib.rs               build_router(AppState) — shared with tests
│       │   ├── bin/dump_openapi.rs  writes openapi.json for TS codegen
│       │   ├── state.rs             AppState
│       │   ├── infra/
│       │   │   ├── mod.rs
│       │   │   ├── error.rs         AppError
│       │   │   ├── db.rs            Db — dual pools, pragmas, migrations
│       │   │   ├── write.rs         write_tx, AuditRecord, Written<T>
│       │   │   ├── ws.rs            EventHub, Envelope, /ws handler
│       │   │   ├── assets.rs        embedded assets + SPA fallback
│       │   │   └── openapi.rs       utoipa ApiDoc
│       │   └── features/
│       │       ├── mod.rs
│       │       └── health/
│       │           ├── mod.rs
│       │           └── routes.rs
│       └── tests/
│           ├── common/mod.rs        TestApp harness
│           ├── health.rs
│           ├── write_tx.rs
│           ├── ws.rs
│           └── assets.rs
└── web/
    ├── package.json
    ├── vite.config.ts               proxies /api and /ws to :8080
    ├── tsconfig.json
    ├── index.html
    ├── dist/.gitkeep                so rust-embed compiles on a cold clone
    └── src/
        ├── main.tsx
        ├── App.tsx
        └── lib/
            ├── eventStream.ts       pure reducer + socket wrapper
            ├── eventStream.test.ts
            └── api/schema.ts        generated from openapi.json
```

---

### Task 1: Workspace, toolchain, CI, and the `core` dependency boundary

The spec's central claim is that `core` is trustworthy *because the compiler and CI enforce its isolation*. That enforcement is built first, before there is anything to protect, so it can never be added "later".

**Files:**
- Create: `Cargo.toml`, `rust-toolchain.toml`, `.gitignore` (append), `justfile`
- Create: `crates/core/Cargo.toml`, `crates/core/src/lib.rs`
- Create: `crates/escpos/Cargo.toml`, `crates/escpos/src/lib.rs`
- Create: `crates/server/Cargo.toml`, `crates/server/src/lib.rs`, `crates/server/src/main.rs`
- Create: `scripts/check-core-deps.sh`
- Create: `.github/workflows/ci.yml`

**Interfaces:**
- Consumes: nothing
- Produces: workspace packages `grubsi-core`, `grubsi-escpos`, `grubsi-server` (lib target `grubsi_server`); `just` recipes `check`, `test`, `dev`, `build`

- [ ] **Step 1: Write the failing boundary check**

Create `scripts/check-core-deps.sh`:

```bash
#!/usr/bin/env bash
# Fails if grubsi-core gains an I/O dependency, directly or transitively.
# The spec's `core` boundary is only meaningful if it is enforced.
set -euo pipefail

FORBIDDEN=(tokio sqlx axum hyper tower reqwest sqlx-core sqlx-sqlite)

deps="$(cargo tree --package grubsi-core --edges normal --prefix none \
        | awk '{print $1}' | sort -u)"

fail=0
for f in "${FORBIDDEN[@]}"; do
  if grep -qx -- "$f" <<<"$deps"; then
    echo "FORBIDDEN dependency in grubsi-core: $f" >&2
    fail=1
  fi
done

if [ "$fail" -ne 0 ]; then
  echo "" >&2
  echo "crates/core must stay free of I/O. See the spec, section 3." >&2
  exit 1
fi
echo "core dependency boundary OK"
```

Then `chmod +x scripts/check-core-deps.sh`.

- [ ] **Step 2: Run it to verify it fails**

Run: `./scripts/check-core-deps.sh`
Expected: FAIL — `error: package ID specification 'grubsi-core' did not match any packages` (there is no workspace yet).

- [ ] **Step 3: Create the workspace and the three crates**

`rust-toolchain.toml`:

```toml
[toolchain]
channel = "1.97"
components = ["rustfmt", "clippy"]
```

Root `Cargo.toml`:

```toml
[workspace]
resolver = "3"
members = ["crates/core", "crates/escpos", "crates/server"]

[workspace.package]
edition = "2024"
rust-version = "1.97"
license = "MIT"

[workspace.dependencies]
grubsi-core = { path = "crates/core" }
grubsi-escpos = { path = "crates/escpos" }

tokio = { version = "1.53", features = ["rt-multi-thread", "macros", "net", "time", "sync", "signal", "io-util"] }
axum = { version = "0.8", features = ["ws", "macros"] }
sqlx = { version = "0.9", default-features = false, features = ["runtime-tokio", "sqlite", "uuid", "chrono", "migrate", "macros"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
uuid = { version = "1", features = ["v7", "serde"] }
chrono = { version = "0.4", features = ["serde"] }
utoipa = { version = "5", features = ["axum_extras", "uuid", "chrono"] }
rust-embed = "8"
async-trait = "0.1"
futures = "0.3"
mime_guess = "2"
tower = "0.5"
tempfile = "3"
```

`crates/core/Cargo.toml`:

```toml
[package]
name = "grubsi-core"
edition.workspace = true
rust-version.workspace = true
license.workspace = true
version = "0.1.0"

# NOTE: no I/O dependencies. Enforced by scripts/check-core-deps.sh in CI.
[dependencies]
serde = { workspace = true }
serde_json = { workspace = true }
uuid = { workspace = true }
chrono = { workspace = true }
thiserror = { workspace = true }
```

`crates/core/src/lib.rs`:

```rust
//! Pure domain logic. No I/O, by construction — see scripts/check-core-deps.sh.

pub mod event;
```

Create `crates/core/src/event.rs` as an empty file for now; Task 6 fills it. To keep this task compiling, put a placeholder in it:

```rust
//! Domain events. Populated in Task 6.
```

`crates/escpos/Cargo.toml`:

```toml
[package]
name = "grubsi-escpos"
edition.workspace = true
rust-version.workspace = true
license.workspace = true
version = "0.1.0"

[dependencies]
async-trait = { workspace = true }
thiserror = { workspace = true }
tokio = { workspace = true }
tracing = { workspace = true }
```

`crates/escpos/src/lib.rs`:

```rust
//! ESC/POS ticket rendering and printer transports.
```

`crates/server/Cargo.toml`:

```toml
[package]
name = "grubsi-server"
edition.workspace = true
rust-version.workspace = true
license.workspace = true
version = "0.1.0"

[lib]
name = "grubsi_server"
path = "src/lib.rs"

[[bin]]
name = "grubsi-server"
path = "src/main.rs"

[dependencies]
grubsi-core = { workspace = true }
grubsi-escpos = { workspace = true }
axum = { workspace = true }
tokio = { workspace = true }
sqlx = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
tracing = { workspace = true }
tracing-subscriber = { workspace = true }
uuid = { workspace = true }
chrono = { workspace = true }
utoipa = { workspace = true }
rust-embed = { workspace = true }
futures = { workspace = true }
mime_guess = { workspace = true }

[dev-dependencies]
tower = { workspace = true, features = ["util"] }
tempfile = { workspace = true }
```

`crates/server/src/lib.rs`:

```rust
//! grubsi server library. The binary is a thin wrapper so integration
//! tests can build the same router the production binary serves.
```

`crates/server/src/main.rs`:

```rust
fn main() {
    println!("grubsi-server");
}
```

Append to `.gitignore`:

```
/target
node_modules
web/dist/*
!web/dist/.gitkeep
openapi.json
*.db
*.db-wal
*.db-shm
```

- [ ] **Step 4: Run the boundary check to verify it passes**

Run: `cargo build --workspace && ./scripts/check-core-deps.sh`
Expected: build succeeds; script prints `core dependency boundary OK`.

If a `sqlx` feature name is rejected, run `cargo add sqlx --features sqlite --dry-run` to list valid features for 0.9 and correct the workspace manifest. Do not silently drop the `uuid` or `chrono` features — later tasks bind those types.

- [ ] **Step 5: Prove the check actually catches a violation**

Temporarily add `tokio = { workspace = true }` to `crates/core/Cargo.toml`, then run:

Run: `cargo build --workspace && ./scripts/check-core-deps.sh`
Expected: FAIL with `FORBIDDEN dependency in grubsi-core: tokio`

Now remove that line again and re-run to confirm it passes. A guard that has never been seen to fail is not a guard.

- [ ] **Step 6: Add the justfile and CI**

`just` is not installed on this machine. Install it first:

Run: `cargo install just`
Verify: `just --version`

Every recipe below is a plain shell command, so `just` is a convenience rather than a dependency — if it is unavailable, run the commands directly.

`justfile`:

```make
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
```

`.github/workflows/ci.yml`:

```yaml
name: CI
on:
  push:
    branches: [master]
  pull_request:

jobs:
  rust:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@1.97
        with:
          components: rustfmt, clippy
      - uses: Swatinem/rust-cache@v2
      # Must succeed on a clean checkout with no frontend build.
      - name: Cold build
        run: cargo build --workspace
      - run: cargo fmt --all -- --check
      - run: cargo clippy --workspace --all-targets -- -D warnings
      - run: ./scripts/check-core-deps.sh
      - run: cargo test --workspace

  web:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version: '26'
      - run: npm --prefix web ci
      - run: npm --prefix web run build
      - run: npm --prefix web run test
```

- [ ] **Step 7: Verify everything passes**

Run: `just check && cargo test --workspace`
Expected: fmt clean, clippy clean, boundary check OK, `0 passed` tests (no tests yet — that is correct at this point).

- [ ] **Step 8: Commit**

```bash
git add Cargo.toml rust-toolchain.toml justfile .gitignore .github scripts crates
git commit -m "feat: workspace skeleton with enforced core dependency boundary"
```

---

### Task 2: `AppError`

**Files:**
- Create: `crates/server/src/infra/mod.rs`
- Create: `crates/server/src/infra/error.rs`
- Modify: `crates/server/src/lib.rs`

**Interfaces:**
- Consumes: nothing
- Produces:
  - `pub struct AppError { status: StatusCode, code: &'static str, message: String, internal: Option<String> }`
  - `AppError::new(StatusCode, &'static str, impl Into<String>) -> AppError`
  - `AppError::internal(impl Into<String>) -> AppError`
  - `AppError::not_found(impl Into<String>) -> AppError`
  - `AppError::with_internal(self, impl Into<String>) -> AppError`
  - `impl IntoResponse for AppError` → `{"code": ..., "message": ...}`
  - `impl From<sqlx::Error> for AppError`
  - `pub type AppResult<T> = Result<T, AppError>;`

- [ ] **Step 1: Write the failing test**

Create `crates/server/src/infra/error.rs` containing only this test module for now:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use axum::response::IntoResponse;
    use http_body_util::BodyExt;

    #[tokio::test]
    async fn internal_detail_never_reaches_the_response() {
        let err = AppError::internal("SQLITE_CONSTRAINT_UNIQUE on tables.name");

        let response = err.into_response();
        assert_eq!(response.status(), axum::http::StatusCode::INTERNAL_SERVER_ERROR);

        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();

        assert_eq!(body["code"], "internal");
        assert!(!body["message"].as_str().unwrap().is_empty());
        // The whole point of the type: the detail is not in the payload.
        let rendered = body.to_string();
        assert!(!rendered.contains("SQLITE_CONSTRAINT_UNIQUE"));
        assert!(!rendered.contains("tables.name"));
    }

    #[tokio::test]
    async fn user_facing_errors_keep_their_message() {
        let err = AppError::new(
            axum::http::StatusCode::CONFLICT,
            "table_name_taken",
            "A table with this name already exists.",
        );

        let response = err.into_response();
        assert_eq!(response.status(), axum::http::StatusCode::CONFLICT);

        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();

        assert_eq!(body["code"], "table_name_taken");
        assert_eq!(body["message"], "A table with this name already exists.");
    }
}
```

Add `http-body-util = "0.1"` to `crates/server/Cargo.toml` `[dev-dependencies]`.

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test --package grubsi-server error::`
Expected: FAIL — `cannot find type AppError in this scope`.

- [ ] **Step 3: Write the implementation**

Put this *above* the test module in `crates/server/src/infra/error.rs`:

```rust
use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;

/// An error on its way to a client.
///
/// The user-facing `message` is mandatory: it is a constructor argument, not
/// an option. Internal detail is carried separately and is logged, never
/// serialized — so a database string cannot reach a steward's tablet.
#[derive(Debug)]
pub struct AppError {
    status: StatusCode,
    code: &'static str,
    message: String,
    internal: Option<String>,
}

pub type AppResult<T> = Result<T, AppError>;

#[derive(Serialize)]
struct ErrorBody<'a> {
    code: &'a str,
    message: &'a str,
}

impl AppError {
    pub fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
        Self { status, code, message: message.into(), internal: None }
    }

    /// An unexpected failure. The caller supplies detail for the log; the
    /// client gets a generic message.
    pub fn internal(internal: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            code: "internal",
            message: "Something went wrong on the server. Please try again.".to_owned(),
            internal: Some(internal.into()),
        }
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, "not_found", message)
    }

    pub fn with_internal(mut self, detail: impl Into<String>) -> Self {
        self.internal = Some(detail.into());
        self
    }

    pub fn code(&self) -> &'static str {
        self.code
    }

    pub fn status(&self) -> StatusCode {
        self.status
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        if let Some(detail) = &self.internal {
            tracing::error!(code = self.code, status = %self.status, detail, "request failed");
        } else if self.status.is_server_error() {
            tracing::error!(code = self.code, status = %self.status, "request failed");
        } else {
            tracing::debug!(code = self.code, status = %self.status, "request rejected");
        }

        let body = ErrorBody { code: self.code, message: &self.message };
        (self.status, Json(body)).into_response()
    }
}

impl From<sqlx::Error> for AppError {
    fn from(err: sqlx::Error) -> Self {
        AppError::internal(format!("sqlx: {err}"))
    }
}
```

Create `crates/server/src/infra/mod.rs`:

```rust
pub mod error;
```

Replace `crates/server/src/lib.rs` with:

```rust
//! grubsi server library. The binary is a thin wrapper so integration
//! tests can build the same router the production binary serves.

pub mod infra;
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test --package grubsi-server error::`
Expected: PASS, 2 tests.

- [ ] **Step 5: Commit**

```bash
git add crates/server
git commit -m "feat: AppError with mandatory user-facing message"
```

---

### Task 3: HTTP router, health endpoint, OpenAPI, and bootstrap

This is the first walking-skeleton moment: after this task, `cargo run` serves a real endpoint on the LAN.

**Files:**
- Create: `crates/server/src/state.rs`
- Create: `crates/server/src/features/mod.rs`, `crates/server/src/features/health/mod.rs`, `crates/server/src/features/health/routes.rs`
- Create: `crates/server/src/infra/openapi.rs`
- Create: `crates/server/src/bin/dump_openapi.rs`
- Modify: `crates/server/src/lib.rs`, `crates/server/src/main.rs`, `crates/server/src/infra/mod.rs`

**Interfaces:**
- Consumes: `AppError`, `AppResult` (Task 2)
- Produces:
  - `pub struct AppState { pub started_at: DateTime<Utc> }` — grows in Tasks 4–7
  - `pub fn build_router(state: AppState) -> axum::Router`
  - `GET /api/v1/health` → `200 {"status":"ok","version":"0.1.0"}`
  - `GET /api-docs/openapi.json` → the OpenAPI document
  - `pub struct ApiDoc` implementing `utoipa::OpenApi`

- [ ] **Step 1: Write the failing test**

Create `crates/server/tests/health.rs`:

```rust
use axum::body::Body;
use axum::http::{Request, StatusCode};
use grubsi_server::{AppState, build_router};
use http_body_util::BodyExt;
use tower::ServiceExt;

#[tokio::test]
async fn health_reports_ok() {
    let app = build_router(AppState::new());

    let response = app
        .oneshot(Request::builder().uri("/api/v1/health").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["status"], "ok");
}

#[tokio::test]
async fn unknown_api_routes_return_json_not_html() {
    // A steward's tablet parses JSON. An unknown /api path must not fall
    // through to the SPA and hand it an HTML document.
    let app = build_router(AppState::new());

    let response = app
        .oneshot(Request::builder().uri("/api/v1/nope").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_owned();
    assert!(content_type.contains("application/json"), "got {content_type}");
}

#[tokio::test]
async fn openapi_document_lists_the_health_path() {
    let app = build_router(AppState::new());

    let response = app
        .oneshot(Request::builder().uri("/api-docs/openapi.json").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let doc: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(doc["paths"]["/api/v1/health"].is_object(), "health path missing from OpenAPI");
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test --package grubsi-server --test health`
Expected: FAIL — `unresolved import grubsi_server::build_router`.

- [ ] **Step 3: Write the implementation**

`crates/server/src/state.rs`:

```rust
use chrono::{DateTime, Utc};

/// Shared application state. Grows through M0: the database lands in Task 4
/// and the event hub in Task 6.
#[derive(Clone)]
pub struct AppState {
    pub started_at: DateTime<Utc>,
}

impl AppState {
    pub fn new() -> Self {
        Self { started_at: Utc::now() }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
```

`crates/server/src/features/health/routes.rs`:

```rust
use axum::Json;
use axum::extract::State;
use serde::Serialize;
use utoipa::ToSchema;

use crate::state::AppState;

#[derive(Serialize, ToSchema)]
pub struct HealthResponse {
    /// Always "ok" when the process is serving requests.
    pub status: &'static str,
    pub version: &'static str,
    /// Seconds since the process started.
    pub uptime_seconds: i64,
}

/// Liveness probe.
#[utoipa::path(
    get,
    path = "/api/v1/health",
    tag = "system",
    responses((status = 200, description = "Server is serving requests", body = HealthResponse))
)]
pub async fn health(State(state): State<AppState>) -> Json<HealthResponse> {
    let uptime = (chrono::Utc::now() - state.started_at).num_seconds();
    Json(HealthResponse {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
        uptime_seconds: uptime,
    })
}
```

`crates/server/src/features/health/mod.rs`:

```rust
pub mod routes;
```

`crates/server/src/features/mod.rs`:

```rust
pub mod health;
```

`crates/server/src/infra/openapi.rs`:

```rust
use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    info(title = "grubsi", version = "0.1.0", description = "Local-first restaurant POS"),
    paths(crate::features::health::routes::health),
    components(schemas(crate::features::health::routes::HealthResponse)),
    tags((name = "system", description = "Health and diagnostics"))
)]
pub struct ApiDoc;
```

Add `pub mod openapi;` to `crates/server/src/infra/mod.rs`.

Replace `crates/server/src/lib.rs`:

```rust
//! grubsi server library. The binary is a thin wrapper so integration
//! tests can build the same router the production binary serves.

pub mod features;
pub mod infra;
pub mod state;

use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use utoipa::OpenApi;

pub use state::AppState;

use infra::error::AppError;
use infra::openapi::ApiDoc;

async fn openapi_json() -> Json<utoipa::openapi::OpenApi> {
    Json(ApiDoc::openapi())
}

/// Unknown `/api` paths must answer in JSON — clients parse it, and an
/// HTML fallback would be a confusing parse error rather than a 404.
async fn api_not_found() -> AppError {
    AppError::new(StatusCode::NOT_FOUND, "not_found", "That endpoint does not exist.")
}

pub fn build_router(state: AppState) -> Router {
    // Do not call `.with_state` on the inner router: `nest` requires both
    // routers to share a state type, and applying state early turns this
    // into a `Router<()>` that will not nest into a stateful parent.
    let api = Router::new()
        .route("/health", get(features::health::routes::health))
        .fallback(api_not_found);

    Router::new()
        .nest("/api/v1", api)
        .route("/api-docs/openapi.json", get(openapi_json))
        .with_state(state)
}
```

`crates/server/src/main.rs`:

```rust
use std::net::{Ipv4Addr, SocketAddr};

use grubsi_server::{AppState, build_router};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,grubsi_server=debug")),
        )
        .init();

    let port: u16 = std::env::var("GRUBSI_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);

    let app = build_router(AppState::new());

    // Bind all interfaces: staff tablets and customer phones reach this
    // over the restaurant LAN, not over loopback.
    let addr = SocketAddr::from((Ipv4Addr::UNSPECIFIED, port));
    let listener = tokio::net::TcpListener::bind(addr).await?;

    tracing::info!(%addr, "grubsi server started");
    for ip in local_addresses() {
        tracing::info!("reachable at http://{ip}:{port}");
    }

    axum::serve(listener, app).await?;
    Ok(())
}

/// Best-effort list of this machine's LAN addresses, printed at startup so
/// staff know what to type into a tablet. Never fatal.
fn local_addresses() -> Vec<String> {
    use std::process::Command;
    let output = Command::new("hostname").arg("-I").output();
    match output {
        Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout)
            .split_whitespace()
            .filter(|s| !s.contains(':'))
            .map(|s| s.to_owned())
            .collect(),
        _ => Vec::new(),
    }
}
```

`crates/server/src/bin/dump_openapi.rs`:

```rust
//! Writes the OpenAPI document to stdout. The TypeScript client is
//! generated from this, so the API contract is a build artifact.

use grubsi_server::infra::openapi::ApiDoc;
use utoipa::OpenApi;

fn main() {
    let doc = ApiDoc::openapi();
    println!("{}", serde_json::to_string_pretty(&doc).expect("serialize OpenAPI"));
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test --package grubsi-server --test health`
Expected: PASS, 3 tests.

- [ ] **Step 5: Verify the server actually runs**

Run in one shell: `cargo run --package grubsi-server`
Run in another: `curl -s localhost:8080/api/v1/health`
Expected: `{"status":"ok","version":"0.1.0","uptime_seconds":N}`, and the server log lists at least one `reachable at http://…` line.

Then stop the server.

- [ ] **Step 6: Commit**

```bash
git add crates/server
git commit -m "feat: health endpoint, OpenAPI document, and LAN bootstrap"
```

---

### Task 4: Database layer — dual pools, pragmas, migrations

**Files:**
- Create: `migrations/0001_audit_logs.sql`
- Create: `crates/server/src/infra/db.rs`
- Modify: `crates/server/src/infra/mod.rs`, `crates/server/src/state.rs`, `crates/server/src/lib.rs`, `crates/server/src/main.rs`

**Interfaces:**
- Consumes: `AppError` (Task 2), `AppState` (Task 3)
- Produces:
  - `pub struct Db { pub read: SqlitePool, pub write: SqlitePool }`
  - `Db::open(path: &std::path::Path) -> Result<Db, sqlx::Error>`
  - `AppState::new()` becomes `AppState::new(db: Db)`; field `pub db: Db`

- [ ] **Step 1: Write the failing test**

Create `crates/server/src/infra/db.rs` with only this test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    async fn temp_db() -> (tempfile::TempDir, Db) {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open(&dir.path().join("grubsi.db")).await.unwrap();
        (dir, db)
    }

    #[tokio::test]
    async fn wal_and_foreign_keys_are_enabled_on_both_pools() {
        let (_dir, db) = temp_db().await;

        for (name, pool) in [("write", &db.write), ("read", &db.read)] {
            let journal: String =
                sqlx::query_scalar("PRAGMA journal_mode").fetch_one(pool).await.unwrap();
            assert_eq!(journal, "wal", "{name} pool journal_mode");

            let fk: i64 =
                sqlx::query_scalar("PRAGMA foreign_keys").fetch_one(pool).await.unwrap();
            assert_eq!(fk, 1, "{name} pool foreign_keys");
        }
    }

    #[tokio::test]
    async fn the_write_pool_holds_exactly_one_connection() {
        // The spec's defence against SQLITE_BUSY. If this ever changes,
        // concurrent writers become possible and the guarantee is gone.
        let (_dir, db) = temp_db().await;
        assert_eq!(db.write.options().get_max_connections(), 1);
        assert!(db.read.options().get_max_connections() > 1);
    }

    #[tokio::test]
    async fn migrations_create_the_audit_log() {
        let (_dir, db) = temp_db().await;
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM audit_logs")
            .fetch_one(&db.read)
            .await
            .unwrap();
        assert_eq!(count, 0);
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test --package grubsi-server db::`
Expected: FAIL — `cannot find type Db in this scope`.

- [ ] **Step 3: Write the migration**

`migrations/0001_audit_logs.sql`:

```sql
-- Audit log. Created in M0 because write_tx cannot exist without it;
-- M1 owns the audit features that read and populate it meaningfully.
--
-- STRICT rejects values of the wrong type at write time rather than
-- silently coercing them. All ids are UUIDv7 as BLOB(16); all timestamps
-- are ISO-8601 UTC.
CREATE TABLE audit_logs (
    id          BLOB NOT NULL PRIMARY KEY,
    user_id     BLOB,
    action      TEXT NOT NULL,
    entity_type TEXT NOT NULL,
    entity_id   BLOB,
    before_json TEXT,
    after_json  TEXT,
    created_at  TEXT NOT NULL
) STRICT;

CREATE INDEX idx_audit_logs_created_at ON audit_logs (created_at);
CREATE INDEX idx_audit_logs_entity ON audit_logs (entity_type, entity_id);
```

- [ ] **Step 4: Write the implementation**

Put this above the test module in `crates/server/src/infra/db.rs`:

```rust
use std::path::Path;
use std::time::Duration;

use sqlx::SqlitePool;
use sqlx::sqlite::{
    SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous,
};

/// Split database handles.
///
/// The write pool holds exactly one connection, so all writers serialize
/// and `SQLITE_BUSY` cannot occur. At restaurant scale this costs nothing.
/// Because that connection is a global lock, no CPU-bound or network work
/// may happen inside a write transaction.
#[derive(Clone)]
pub struct Db {
    pub read: SqlitePool,
    pub write: SqlitePool,
}

const READ_POOL_SIZE: u32 = 8;

impl Db {
    pub async fn open(path: &Path) -> Result<Self, sqlx::Error> {
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Normal)
            .foreign_keys(true)
            .busy_timeout(Duration::from_secs(5));

        let write = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options.clone())
            .await?;

        // Migrations run on the write pool before readers are handed out,
        // so no reader can observe a half-migrated schema.
        sqlx::migrate!("../../migrations").run(&write).await?;

        let read = SqlitePoolOptions::new()
            .max_connections(READ_POOL_SIZE)
            .connect_with(options)
            .await?;

        Ok(Self { read, write })
    }
}
```

Add `pub mod db;` to `crates/server/src/infra/mod.rs`.

Update `crates/server/src/state.rs`:

```rust
use chrono::{DateTime, Utc};

use crate::infra::db::Db;

/// Shared application state. The event hub lands in Task 6.
#[derive(Clone)]
pub struct AppState {
    pub started_at: DateTime<Utc>,
    pub db: Db,
}

impl AppState {
    pub fn new(db: Db) -> Self {
        Self { started_at: Utc::now(), db }
    }
}
```

Remove the `Default` impl — `AppState` now requires a database.

Update `crates/server/tests/health.rs` to build state with a temp database. Add this helper at the top of that file and replace each `AppState::new()` with `state().await`:

```rust
use grubsi_server::infra::db::Db;

async fn state() -> (tempfile::TempDir, AppState) {
    let dir = tempfile::tempdir().unwrap();
    let db = Db::open(&dir.path().join("grubsi.db")).await.unwrap();
    (dir, AppState::new(db))
}
```

Each test then reads, for example:

```rust
let (_dir, st) = state().await;
let app = build_router(st);
```

Update `crates/server/src/main.rs` — after the tracing init and before building the router:

```rust
    let db_path = std::env::var("GRUBSI_DB")
        .unwrap_or_else(|_| "grubsi.db".to_owned());
    let db = grubsi_server::infra::db::Db::open(std::path::Path::new(&db_path)).await?;
    tracing::info!(path = %db_path, "database ready");

    let app = build_router(AppState::new(db));
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test --package grubsi-server`
Expected: PASS — 3 db tests and 3 health tests.

- [ ] **Step 6: Commit**

```bash
git add migrations crates/server
git commit -m "feat: dual SQLite pools with WAL pragmas and migrations"
```

---

### Task 5: `write_tx` — the audited write path

**Files:**
- Create: `crates/server/src/infra/write.rs`
- Create: `crates/server/tests/write_tx.rs`
- Modify: `crates/server/src/infra/mod.rs`

**Interfaces:**
- Consumes: `Db` (Task 4), `AppError`/`AppResult` (Task 2)
- Produces:
  - `pub struct AuditRecord { user_id: Option<Uuid>, action: String, entity_type: String, entity_id: Option<Uuid>, before: Option<Value>, after: Option<Value> }`
  - `AuditRecord::new(action: impl Into<String>, entity_type: impl Into<String>) -> AuditRecord`
  - builders `.by(Uuid)`, `.entity(Uuid)`, `.before(Value)`, `.after(Value)`
  - `pub struct Written<T> { pub value: T, pub events: Vec<DomainEvent> }`
  - `pub async fn write_tx<T, F>(db: &Db, audit: AuditRecord, f: F) -> AppResult<Written<T>>` where `F: for<'c> FnOnce(&'c mut SqliteConnection) -> BoxFuture<'c, AppResult<(T, Vec<DomainEvent>)>>`

`DomainEvent` does not exist until Task 6. Until then this task uses a temporary alias defined in `write.rs`; Task 6 replaces it. This is called out explicitly so the executor does not invent a type.

- [ ] **Step 1: Write the failing test**

Create `crates/server/tests/write_tx.rs`:

```rust
use futures::FutureExt;
use grubsi_server::infra::db::Db;
use grubsi_server::infra::error::AppError;
use grubsi_server::infra::write::{AuditRecord, write_tx};

async fn temp_db() -> (tempfile::TempDir, Db) {
    let dir = tempfile::tempdir().unwrap();
    let db = Db::open(&dir.path().join("grubsi.db")).await.unwrap();
    // A throwaway table to mutate, so the test does not depend on a
    // feature table that M0 has not built yet.
    sqlx::query("CREATE TABLE probe (id INTEGER NOT NULL PRIMARY KEY, note TEXT NOT NULL) STRICT")
        .execute(&db.write)
        .await
        .unwrap();
    (dir, db)
}

async fn audit_count(db: &Db) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM audit_logs").fetch_one(&db.read).await.unwrap()
}

async fn probe_count(db: &Db) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM probe").fetch_one(&db.read).await.unwrap()
}

#[tokio::test]
async fn commits_the_mutation_and_its_audit_record_together() {
    let (_dir, db) = temp_db().await;

    let written = write_tx(
        &db,
        AuditRecord::new("probe.create", "probe").after(serde_json::json!({"note": "hello"})),
        |conn| {
            async move {
                sqlx::query("INSERT INTO probe (id, note) VALUES (1, 'hello')")
                    .execute(&mut *conn)
                    .await?;
                Ok((42_i32, Vec::new()))
            }
            .boxed()
        },
    )
    .await
    .unwrap();

    assert_eq!(written.value, 42);
    assert_eq!(probe_count(&db).await, 1);
    assert_eq!(audit_count(&db).await, 1);

    let action: String = sqlx::query_scalar("SELECT action FROM audit_logs")
        .fetch_one(&db.read)
        .await
        .unwrap();
    assert_eq!(action, "probe.create");
}

#[tokio::test]
async fn a_failing_mutation_leaves_no_audit_record() {
    // Audit and mutation share one transaction. A partial write that
    // recorded the audit but not the change would be worse than neither.
    let (_dir, db) = temp_db().await;

    let result = write_tx(
        &db,
        AuditRecord::new("probe.create", "probe"),
        |conn| {
            async move {
                sqlx::query("INSERT INTO probe (id, note) VALUES (1, 'hello')")
                    .execute(&mut *conn)
                    .await?;
                Err::<((), Vec<grubsi_core::event::DomainEvent>), AppError>(
                    AppError::internal("deliberate failure"),
                )
            }
            .boxed()
        },
    )
    .await;

    assert!(result.is_err());
    assert_eq!(probe_count(&db).await, 0);
    assert_eq!(audit_count(&db).await, 0);
}

#[tokio::test]
async fn events_are_returned_for_the_caller_to_publish_after_commit() {
    // write_tx must not publish. Returning events is what keeps the
    // after-commit rule structurally hard to violate.
    let (_dir, db) = temp_db().await;

    let written = write_tx(
        &db,
        AuditRecord::new("probe.create", "probe"),
        |conn| {
            async move {
                sqlx::query("INSERT INTO probe (id, note) VALUES (7, 'x')")
                    .execute(&mut *conn)
                    .await?;
                Ok(((), vec![grubsi_core::event::DomainEvent::ping()]))
            }
            .boxed()
        },
    )
    .await
    .unwrap();

    assert_eq!(written.events.len(), 1);
}
```

The third test references `grubsi_core::event::DomainEvent::ping()`, which Task 6 builds. No manifest change is needed — `grubsi-core` is already a normal dependency of `grubsi-server`. **Write all three tests now and expect the third to fail to compile until Task 6.** If executing tasks strictly one at a time, comment the third test out behind a `// TODO(Task 6)` marker and restore it in Task 6, Step 5.

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test --package grubsi-server --test write_tx`
Expected: FAIL — `unresolved import grubsi_server::infra::write`.

- [ ] **Step 3: Write the implementation**

`crates/server/src/infra/write.rs`:

```rust
use chrono::Utc;
use futures::future::BoxFuture;
use serde_json::Value;
use sqlx::SqliteConnection;
use uuid::Uuid;

use crate::infra::db::Db;
use crate::infra::error::{AppError, AppResult};

// Replaced by grubsi_core::event::DomainEvent in Task 6.
use grubsi_core::event::DomainEvent;

/// What happened, who did it, and what changed.
///
/// This is a required argument to `write_tx`. The spec argues that `core`'s
/// boundary works because the compiler enforces it; audit logging is the
/// most cross-cutting concern in the system and the easiest to forget, so
/// it gets the same treatment rather than a convention.
#[derive(Debug, Clone)]
pub struct AuditRecord {
    pub user_id: Option<Uuid>,
    pub action: String,
    pub entity_type: String,
    pub entity_id: Option<Uuid>,
    pub before: Option<Value>,
    pub after: Option<Value>,
}

impl AuditRecord {
    pub fn new(action: impl Into<String>, entity_type: impl Into<String>) -> Self {
        Self {
            user_id: None,
            action: action.into(),
            entity_type: entity_type.into(),
            entity_id: None,
            before: None,
            after: None,
        }
    }

    pub fn by(mut self, user_id: Uuid) -> Self {
        self.user_id = Some(user_id);
        self
    }

    pub fn entity(mut self, entity_id: Uuid) -> Self {
        self.entity_id = Some(entity_id);
        self
    }

    pub fn before(mut self, before: Value) -> Self {
        self.before = Some(before);
        self
    }

    pub fn after(mut self, after: Value) -> Self {
        self.after = Some(after);
        self
    }
}

/// The result of a write: the value produced, plus events the caller
/// publishes *after* the transaction has committed.
#[derive(Debug)]
pub struct Written<T> {
    pub value: T,
    pub events: Vec<DomainEvent>,
}

/// Run a mutation inside a single write transaction, recording an audit
/// entry in the same transaction.
///
/// Events are returned rather than published: publishing inside the
/// transaction would eventually broadcast state that gets rolled back.
pub async fn write_tx<T, F>(db: &Db, audit: AuditRecord, f: F) -> AppResult<Written<T>>
where
    F: for<'c> FnOnce(&'c mut SqliteConnection) -> BoxFuture<'c, AppResult<(T, Vec<DomainEvent>)>>,
{
    let mut tx = db.write.begin().await?;

    let (value, events) = f(&mut tx).await?;

    sqlx::query(
        "INSERT INTO audit_logs
            (id, user_id, action, entity_type, entity_id, before_json, after_json, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(Uuid::now_v7())
    .bind(audit.user_id)
    .bind(&audit.action)
    .bind(&audit.entity_type)
    .bind(audit.entity_id)
    .bind(audit.before.as_ref().map(|v| v.to_string()))
    .bind(audit.after.as_ref().map(|v| v.to_string()))
    .bind(Utc::now().to_rfc3339())
    .execute(&mut *tx)
    .await
    .map_err(|e| AppError::internal(format!("audit insert: {e}")))?;

    tx.commit().await?;

    Ok(Written { value, events })
}
```

Add `pub mod write;` to `crates/server/src/infra/mod.rs`.

Add `futures = { workspace = true }` to `crates/server` `[dev-dependencies]` as well as its normal dependencies, since the tests call `.boxed()`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test --package grubsi-server --test write_tx`
Expected: PASS — 2 tests (3 once Task 6 lands).

If `f(&mut tx)` fails to coerce, write `f(&mut *tx)`; `Transaction` derefs to `SqliteConnection`.

- [ ] **Step 5: Commit**

```bash
git add crates/server
git commit -m "feat: write_tx makes the audit record a required argument"
```

---

### Task 6: Domain events and the event hub

**Files:**
- Modify: `crates/core/src/event.rs`
- Create: `crates/server/src/infra/ws.rs` (hub only; the HTTP handler is Task 7)
- Modify: `crates/server/src/infra/mod.rs`, `crates/server/src/state.rs`

**Interfaces:**
- Consumes: nothing from earlier tasks
- Produces:
  - `grubsi_core::event::Topic` — `Staff`, `Station(Uuid)`, `Table(Uuid)`, `Check(Uuid)`; `Topic::as_key(&self) -> String`
  - `grubsi_core::event::EventKind` — `Ping` for M0; serializes SCREAMING_SNAKE_CASE
  - `grubsi_core::event::DomainEvent { kind, topic, payload }`; `DomainEvent::ping() -> DomainEvent`
  - `grubsi_server::infra::ws::Envelope { boot_id, seq, kind, topic, payload, at }`
  - `grubsi_server::infra::ws::EventHub` — `new()`, `publish(DomainEvent) -> Arc<Envelope>`, `publish_all(Vec<DomainEvent>)`, `subscribe() -> broadcast::Receiver<Arc<Envelope>>`, `boot_id() -> Uuid`

- [ ] **Step 1: Write the failing tests**

Replace `crates/core/src/event.rs` with a test module first:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn topics_render_as_stable_keys() {
        let id = uuid::uuid!("018f0000-0000-7000-8000-000000000001");
        assert_eq!(Topic::Staff.as_key(), "staff");
        assert_eq!(Topic::Station(id).as_key(), format!("station:{id}"));
        assert_eq!(Topic::Table(id).as_key(), format!("table:{id}"));
        assert_eq!(Topic::Check(id).as_key(), format!("check:{id}"));
    }

    #[test]
    fn event_kinds_serialize_in_screaming_snake_case() {
        let json = serde_json::to_string(&EventKind::Ping).unwrap();
        assert_eq!(json, "\"PING\"");
    }
}
```

Add `uuid = { workspace = true, features = ["macro-diagnostics"] }`? No — the `uuid!` macro needs the `macro-diagnostics` feature only for better errors. Instead avoid the macro: use `uuid::Uuid::nil()` in the test and compare against `format!("station:{}", uuid::Uuid::nil())`.

Rewrite those two assertions as:

```rust
        let id = uuid::Uuid::nil();
        assert_eq!(Topic::Staff.as_key(), "staff");
        assert_eq!(Topic::Station(id).as_key(), format!("station:{id}"));
```

Create `crates/server/src/infra/ws.rs` with a test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use grubsi_core::event::DomainEvent;

    #[tokio::test]
    async fn every_subscriber_receives_the_same_envelope() {
        let hub = EventHub::new();
        let mut a = hub.subscribe();
        let mut b = hub.subscribe();

        hub.publish(DomainEvent::ping());

        let ea = a.recv().await.unwrap();
        let eb = b.recv().await.unwrap();
        assert_eq!(ea.seq, eb.seq);
        assert_eq!(ea.seq, 1);
    }

    #[tokio::test]
    async fn sequence_numbers_increase_monotonically() {
        let hub = EventHub::new();
        let mut rx = hub.subscribe();

        hub.publish(DomainEvent::ping());
        hub.publish(DomainEvent::ping());
        hub.publish(DomainEvent::ping());

        let seqs = vec![
            rx.recv().await.unwrap().seq,
            rx.recv().await.unwrap().seq,
            rx.recv().await.unwrap().seq,
        ];
        assert_eq!(seqs, vec![1, 2, 3]);
    }

    #[tokio::test]
    async fn a_lagging_subscriber_is_told_to_resync_rather_than_dropped() {
        // A slow KDS tablet must recover by refetching, not by silently
        // missing orders and not by tearing down its socket.
        let hub = EventHub::with_capacity(2);
        let mut rx = hub.subscribe();

        for _ in 0..10 {
            hub.publish(DomainEvent::ping());
        }

        match rx.recv().await {
            Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                assert!(n > 0);
                let frame =
                    frame_for(Err(tokio::sync::broadcast::error::RecvError::Lagged(n)));
                assert!(matches!(frame, Some(Frame::Resync)), "got {frame:?}");
            }
            other => panic!("expected Lagged, got {other:?}"),
        }
    }

    #[test]
    fn a_closed_channel_ends_the_stream() {
        assert!(frame_for(Err(tokio::sync::broadcast::error::RecvError::Closed)).is_none());
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test --package grubsi-core && cargo test --package grubsi-server ws::`
Expected: FAIL — `cannot find type Topic`, `cannot find type EventHub`.

- [ ] **Step 3: Implement `core::event`**

Put above the test module in `crates/core/src/event.rs`:

```rust
use serde::Serialize;
use serde_json::Value;
use uuid::Uuid;

/// Who an event is for.
///
/// Topics are a security boundary, not a convenience: a customer device
/// must never receive restaurant-wide state. The server derives the set a
/// socket may join from its session, at connect time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Topic {
    Staff,
    Station(Uuid),
    Table(Uuid),
    Check(Uuid),
}

impl Topic {
    pub fn as_key(&self) -> String {
        match self {
            Topic::Staff => "staff".to_owned(),
            Topic::Station(id) => format!("station:{id}"),
            Topic::Table(id) => format!("table:{id}"),
            Topic::Check(id) => format!("check:{id}"),
        }
    }
}

impl Serialize for Topic {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.as_key())
    }
}

/// M0 defines only `Ping`. Real kinds arrive with the features that emit
/// them; see the spec, section 6.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EventKind {
    Ping,
}

#[derive(Debug, Clone, Serialize)]
pub struct DomainEvent {
    pub kind: EventKind,
    pub topic: Topic,
    pub payload: Value,
}

impl DomainEvent {
    pub fn new(kind: EventKind, topic: Topic, payload: Value) -> Self {
        Self { kind, topic, payload }
    }

    /// The M0 walking-skeleton event.
    pub fn ping() -> Self {
        Self::new(EventKind::Ping, Topic::Staff, Value::Null)
    }
}
```

- [ ] **Step 4: Implement the hub**

Put above the test module in `crates/server/src/infra/ws.rs`:

```rust
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use chrono::{DateTime, Utc};
use grubsi_core::event::{DomainEvent, EventKind, Topic};
use serde::Serialize;
use serde_json::Value;
use tokio::sync::broadcast;
use uuid::Uuid;

const DEFAULT_CAPACITY: usize = 512;

/// One published event, as it appears on the wire.
///
/// `boot_id` changes on every server start and `seq` increases by exactly
/// one per event, so a client can detect both a restart and a gap and
/// respond with the same action: refetch.
#[derive(Debug, Clone, Serialize)]
pub struct Envelope {
    pub boot_id: Uuid,
    pub seq: u64,
    pub kind: EventKind,
    pub topic: Topic,
    pub payload: Value,
    pub at: DateTime<Utc>,
}

/// What a socket should send for a given receive result.
#[derive(Debug, Clone)]
pub enum Frame {
    Event(Arc<Envelope>),
    /// The subscriber fell behind. Tell it to refetch rather than treating
    /// this as an error or a disconnect — both defaults are wrong.
    Resync,
}

pub fn frame_for(result: Result<Arc<Envelope>, broadcast::error::RecvError>) -> Option<Frame> {
    match result {
        Ok(env) => Some(Frame::Event(env)),
        Err(broadcast::error::RecvError::Lagged(_)) => Some(Frame::Resync),
        Err(broadcast::error::RecvError::Closed) => None,
    }
}

pub struct EventHub {
    boot_id: Uuid,
    seq: AtomicU64,
    tx: broadcast::Sender<Arc<Envelope>>,
}

impl EventHub {
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_CAPACITY)
    }

    pub fn with_capacity(capacity: usize) -> Self {
        let (tx, _rx) = broadcast::channel(capacity);
        Self { boot_id: Uuid::now_v7(), seq: AtomicU64::new(0), tx }
    }

    pub fn boot_id(&self) -> Uuid {
        self.boot_id
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Arc<Envelope>> {
        self.tx.subscribe()
    }

    /// Publish one event. Call this only after the transaction that
    /// produced it has committed.
    pub fn publish(&self, event: DomainEvent) -> Arc<Envelope> {
        let envelope = Arc::new(Envelope {
            boot_id: self.boot_id,
            seq: self.seq.fetch_add(1, Ordering::SeqCst) + 1,
            kind: event.kind,
            topic: event.topic,
            payload: event.payload,
            at: Utc::now(),
        });
        // An error means nobody is listening, which is normal.
        let _ = self.tx.send(Arc::clone(&envelope));
        envelope
    }

    pub fn publish_all(&self, events: Vec<DomainEvent>) {
        for event in events {
            self.publish(event);
        }
    }
}

impl Default for EventHub {
    fn default() -> Self {
        Self::new()
    }
}
```

Add `pub mod ws;` to `crates/server/src/infra/mod.rs`.

Add the hub to `crates/server/src/state.rs`:

```rust
use std::sync::Arc;

use chrono::{DateTime, Utc};

use crate::infra::db::Db;
use crate::infra::ws::EventHub;

#[derive(Clone)]
pub struct AppState {
    pub started_at: DateTime<Utc>,
    pub db: Db,
    pub hub: Arc<EventHub>,
}

impl AppState {
    pub fn new(db: Db) -> Self {
        Self { started_at: Utc::now(), db, hub: Arc::new(EventHub::new()) }
    }
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Uncomment the third `write_tx` test if it was commented out in Task 5.

Run: `cargo test --workspace`
Expected: PASS — 2 core tests, 4 ws tests, 3 write_tx tests, 3 db tests, 3 health tests.

- [ ] **Step 6: Commit**

```bash
git add crates
git commit -m "feat: domain events and the broadcast hub with resync on lag"
```

---

### Task 7: The `/ws` endpoint

**Files:**
- Modify: `crates/server/src/infra/ws.rs` (add the handler)
- Modify: `crates/server/src/lib.rs`
- Create: `crates/server/tests/ws.rs`

**Interfaces:**
- Consumes: `EventHub`, `Envelope`, `frame_for`, `Frame` (Task 6); `AppState` (Task 3)
- Produces:
  - `GET /ws` — WebSocket. First frame is `{"type":"HELLO","boot_id":...,"seq":N}`; then `{"type":"EVENT","envelope":{...}}` per event; `{"type":"RESYNC"}` on lag.
  - `pub async fn ws_handler(ws: WebSocketUpgrade, State<AppState>) -> Response`
  - `POST /api/v1/dev/ping` — publishes a `Ping`, so the skeleton has something to drive it.

- [ ] **Step 1: Write the failing test**

Create `crates/server/tests/ws.rs`:

```rust
mod common;

use common::TestApp;
use futures::{SinkExt, StreamExt};

#[tokio::test]
async fn a_published_event_reaches_a_connected_socket() {
    let app = TestApp::spawn().await;

    let (mut socket, _) =
        tokio_tungstenite::connect_async(format!("ws://{}/ws", app.addr)).await.unwrap();

    // The socket announces itself first so the client can record boot_id.
    let hello = socket.next().await.unwrap().unwrap();
    let hello: serde_json::Value = serde_json::from_str(hello.to_text().unwrap()).unwrap();
    assert_eq!(hello["type"], "HELLO");
    assert!(hello["boot_id"].is_string());

    app.post("/api/v1/dev/ping").await;

    let msg = socket.next().await.unwrap().unwrap();
    let frame: serde_json::Value = serde_json::from_str(msg.to_text().unwrap()).unwrap();
    assert_eq!(frame["type"], "EVENT");
    assert_eq!(frame["envelope"]["kind"], "PING");
    assert_eq!(frame["envelope"]["topic"], "staff");
    assert_eq!(frame["envelope"]["seq"], 1);

    socket.close(None).await.ok();
}
```

This needs the `TestApp` harness. Build it here (Task 10 extends it with the fake printer). Create `crates/server/tests/common/mod.rs`:

```rust
use std::net::SocketAddr;

use grubsi_server::infra::db::Db;
use grubsi_server::{AppState, build_router};
use tempfile::TempDir;

pub struct TestApp {
    pub addr: SocketAddr,
    pub state: AppState,
    _dir: TempDir,
}

impl TestApp {
    /// Bind the real router to an ephemeral loopback port, backed by a
    /// throwaway database file. Not `:memory:` — WAL behaves differently
    /// there, and WAL is what production runs.
    pub async fn spawn() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open(&dir.path().join("grubsi.db")).await.unwrap();
        let state = AppState::new(db);

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let router = build_router(state.clone());

        tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });

        Self { addr, state, _dir: dir }
    }

    pub fn url(&self, path: &str) -> String {
        format!("http://{}{}", self.addr, path)
    }

    pub async fn post(&self, path: &str) -> reqwest::Response {
        reqwest::Client::new().post(self.url(path)).send().await.unwrap()
    }

    pub async fn get_json(&self, path: &str) -> serde_json::Value {
        reqwest::get(self.url(path)).await.unwrap().json().await.unwrap()
    }
}
```

Add to `crates/server` `[dev-dependencies]`:

```toml
reqwest = { version = "0.12", default-features = false, features = ["json"] }
tokio-tungstenite = "0.24"
futures = { workspace = true }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test --package grubsi-server --test ws`
Expected: FAIL — connection refused or 404 on `/ws`.

- [ ] **Step 3: Write the handler**

Append to `crates/server/src/infra/ws.rs` (above the tests):

```rust
use axum::extract::State;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::response::Response;

use crate::state::AppState;

#[derive(Serialize)]
#[serde(tag = "type")]
enum ClientFrame<'a> {
    #[serde(rename = "HELLO")]
    Hello { boot_id: Uuid, seq: u64 },
    #[serde(rename = "EVENT")]
    Event { envelope: &'a Envelope },
    #[serde(rename = "RESYNC")]
    Resync,
}

pub async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> Response {
    ws.on_upgrade(move |socket| pump(socket, state))
}

async fn pump(mut socket: WebSocket, state: AppState) {
    let hub = state.hub;

    // Subscribe before sending HELLO so no event published between the two
    // can slip past unnoticed.
    let mut rx = hub.subscribe();
    let hello = ClientFrame::Hello {
        boot_id: hub.boot_id(),
        seq: hub.current_seq(),
    };
    if send(&mut socket, &hello).await.is_err() {
        return;
    }

    loop {
        match frame_for(rx.recv().await) {
            Some(Frame::Event(envelope)) => {
                if send(&mut socket, &ClientFrame::Event { envelope: &envelope }).await.is_err() {
                    return;
                }
            }
            Some(Frame::Resync) => {
                tracing::warn!("websocket subscriber lagged; asking client to resync");
                if send(&mut socket, &ClientFrame::Resync).await.is_err() {
                    return;
                }
            }
            None => return,
        }
    }
}

async fn send(socket: &mut WebSocket, frame: &ClientFrame<'_>) -> Result<(), axum::Error> {
    let text = serde_json::to_string(frame).expect("frames are serializable");
    socket.send(Message::Text(text.into())).await
}
```

Add `current_seq` to `EventHub`:

```rust
    pub fn current_seq(&self) -> u64 {
        self.seq.load(Ordering::SeqCst)
    }
```

Add the dev ping route. In `crates/server/src/lib.rs`:

```rust
use axum::routing::post;
use grubsi_core::event::DomainEvent;
use axum::extract::State;

/// Publishes one event. This exists so the M0 skeleton has something to
/// drive the socket; it is removed when real features begin emitting
/// events in M4.
async fn dev_ping(State(state): State<AppState>) -> StatusCode {
    state.hub.publish(DomainEvent::ping());
    StatusCode::ACCEPTED
}
```

and extend `build_router`:

```rust
pub fn build_router(state: AppState) -> Router {
    let api = Router::new()
        .route("/health", get(features::health::routes::health))
        .route("/dev/ping", post(dev_ping))
        .fallback(api_not_found);

    Router::new()
        .nest("/api/v1", api)
        .route("/api-docs/openapi.json", get(openapi_json))
        .route("/ws", get(infra::ws::ws_handler))
        .with_state(state)
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test --package grubsi-server --test ws`
Expected: PASS, 1 test.

- [ ] **Step 5: Commit**

```bash
git add crates/server
git commit -m "feat: /ws endpoint streaming envelopes with resync frames"
```

---

### Task 8: `escpos` — `TicketSink` and `FakePrinter`

No feature prints anything yet. The point of building this now is that M4's print queue arrives with its test double already proven, including the failure modes MVP.md §26 requires.

**Files:**
- Create: `crates/escpos/src/sink.rs`
- Create: `crates/escpos/src/transport/mod.rs`, `tcp.rs`, `fake.rs`
- Modify: `crates/escpos/src/lib.rs`

**Interfaces:**
- Consumes: nothing
- Produces:
  - `pub enum SinkError { Connect(String), Write(String), Timeout }`
  - `#[async_trait] pub trait TicketSink: Send + Sync { async fn send(&self, bytes: &[u8]) -> Result<(), SinkError>; }`
  - `pub struct TcpSink { addr: SocketAddr, timeout: Duration }` — `TcpSink::new(SocketAddr) -> TcpSink`
  - `pub enum FakeMode { Ok, Refuse, Hang, DieMidJob, Offline }`
  - `pub struct FakePrinter` — `FakePrinter::start(FakeMode) -> FakePrinter`, `addr() -> SocketAddr`, `received() -> Vec<Vec<u8>>`

M4 changes `send` to take a `PrintJob`. Bytes are the right abstraction for M0 because no job type exists yet.

- [ ] **Step 1: Write the failing test**

Create `crates/escpos/tests/transport.rs`:

```rust
use grubsi_escpos::transport::fake::{FakeMode, FakePrinter};
use grubsi_escpos::transport::tcp::TcpSink;
use grubsi_escpos::sink::TicketSink;

#[tokio::test]
async fn bytes_arrive_at_a_healthy_printer() {
    let printer = FakePrinter::start(FakeMode::Ok).await;
    let sink = TcpSink::new(printer.addr());

    sink.send(b"HELLO KITCHEN\n").await.unwrap();

    let received = printer.wait_for_job().await;
    assert_eq!(received, b"HELLO KITCHEN\n");
}

#[tokio::test]
async fn an_offline_printer_reports_a_connect_error() {
    let printer = FakePrinter::start(FakeMode::Offline).await;
    let sink = TcpSink::new(printer.addr());

    let err = sink.send(b"anything").await.unwrap_err();
    assert!(matches!(err, grubsi_escpos::sink::SinkError::Connect(_)), "got {err:?}");
}

#[tokio::test]
async fn a_hanging_printer_times_out_rather_than_blocking_forever() {
    // A printer that accepts the connection and then stops reading must
    // not wedge the dispatcher for that station.
    let printer = FakePrinter::start(FakeMode::Hang).await;
    let sink = TcpSink::new(printer.addr()).with_timeout(std::time::Duration::from_millis(200));

    let err = sink.send(&vec![0u8; 4 * 1024 * 1024]).await.unwrap_err();
    assert!(matches!(err, grubsi_escpos::sink::SinkError::Timeout), "got {err:?}");
}
```

Add to `crates/escpos/Cargo.toml`:

```toml
[dev-dependencies]
tokio = { workspace = true, features = ["rt-multi-thread", "macros", "net", "time", "io-util", "sync"] }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test --package grubsi-escpos`
Expected: FAIL — `unresolved import grubsi_escpos::transport`.

- [ ] **Step 3: Write the implementation**

`crates/escpos/src/sink.rs`:

```rust
use async_trait::async_trait;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SinkError {
    #[error("could not reach the printer: {0}")]
    Connect(String),
    #[error("the printer accepted the connection but not the job: {0}")]
    Write(String),
    #[error("the printer stopped responding")]
    Timeout,
}

/// Where a rendered ticket goes.
///
/// Three implementations: TCP for production, a file sink for local
/// development, and a fake for tests. The print queue drives all of them
/// identically.
#[async_trait]
pub trait TicketSink: Send + Sync {
    async fn send(&self, bytes: &[u8]) -> Result<(), SinkError>;
}
```

`crates/escpos/src/transport/tcp.rs`:

```rust
use std::net::SocketAddr;
use std::time::Duration;

use async_trait::async_trait;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;

use crate::sink::{SinkError, TicketSink};

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);

/// Production transport: raw ESC/POS over TCP, conventionally port 9100.
pub struct TcpSink {
    addr: SocketAddr,
    timeout: Duration,
}

impl TcpSink {
    pub fn new(addr: SocketAddr) -> Self {
        Self { addr, timeout: DEFAULT_TIMEOUT }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
}

#[async_trait]
impl TicketSink for TcpSink {
    async fn send(&self, bytes: &[u8]) -> Result<(), SinkError> {
        let connect = tokio::time::timeout(self.timeout, TcpStream::connect(self.addr));
        let mut stream = match connect.await {
            Err(_) => return Err(SinkError::Timeout),
            Ok(Err(e)) => return Err(SinkError::Connect(e.to_string())),
            Ok(Ok(s)) => s,
        };

        let write = async {
            stream.write_all(bytes).await?;
            stream.flush().await
        };

        match tokio::time::timeout(self.timeout, write).await {
            Err(_) => Err(SinkError::Timeout),
            Ok(Err(e)) => Err(SinkError::Write(e.to_string())),
            Ok(Ok(())) => Ok(()),
        }
    }
}
```

`crates/escpos/src/transport/fake.rs`:

```rust
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use tokio::io::AsyncReadExt;
use tokio::net::TcpListener;
use tokio::sync::Notify;

/// How the fake printer misbehaves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FakeMode {
    /// Accepts and reads the whole job.
    Ok,
    /// Accepts the connection, then closes it immediately.
    Refuse,
    /// Accepts the connection and never reads.
    Hang,
    /// Reads part of the job, then drops the connection.
    DieMidJob,
    /// Not listening at all.
    Offline,
}

/// An in-process stand-in for a network thermal printer.
///
/// CI has no printers, and MVP.md section 26's failure paths cannot be
/// exercised any other way.
pub struct FakePrinter {
    addr: SocketAddr,
    received: Arc<Mutex<Vec<Vec<u8>>>>,
    got_job: Arc<Notify>,
}

impl FakePrinter {
    pub async fn start(mode: FakeMode) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind fake printer");
        let addr = listener.local_addr().expect("fake printer addr");

        let received = Arc::new(Mutex::new(Vec::new()));
        let got_job = Arc::new(Notify::new());

        if mode == FakeMode::Offline {
            // Drop the listener so the port is closed and connects are refused.
            drop(listener);
            return Self { addr, received, got_job };
        }

        let received_task = Arc::clone(&received);
        let notify_task = Arc::clone(&got_job);

        tokio::spawn(async move {
            loop {
                let Ok((mut stream, _)) = listener.accept().await else { return };

                match mode {
                    FakeMode::Offline => return,
                    FakeMode::Refuse => {
                        drop(stream);
                    }
                    FakeMode::Hang => {
                        // Hold the connection open and never read from it.
                        std::future::pending::<()>().await;
                    }
                    FakeMode::DieMidJob => {
                        let mut buf = [0u8; 8];
                        let _ = stream.read(&mut buf).await;
                        drop(stream);
                    }
                    FakeMode::Ok => {
                        let mut buf = Vec::new();
                        if stream.read_to_end(&mut buf).await.is_ok() {
                            received_task.lock().expect("fake printer lock").push(buf);
                            notify_task.notify_waiters();
                        }
                    }
                }
            }
        });

        Self { addr, received, got_job }
    }

    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    pub fn received(&self) -> Vec<Vec<u8>> {
        self.received.lock().expect("fake printer lock").clone()
    }

    /// Wait for the next completed job and return its bytes.
    pub async fn wait_for_job(&self) -> Vec<u8> {
        loop {
            if let Some(job) = self.received().into_iter().next() {
                return job;
            }
            self.got_job.notified().await;
        }
    }
}
```

`crates/escpos/src/transport/mod.rs`:

```rust
pub mod fake;
pub mod tcp;
```

`crates/escpos/src/lib.rs`:

```rust
//! ESC/POS ticket rendering and printer transports.
//!
//! M0 provides the transport layer and its test double. Rendering
//! (`Document`, `render`, `encode`) arrives in M4 with the print queue.

pub mod sink;
pub mod transport;
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test --package grubsi-escpos`
Expected: PASS, 3 tests.

If the `Hang` test is flaky, raise the payload size — the write must exceed the OS socket buffer before it blocks.

- [ ] **Step 5: Commit**

```bash
git add crates/escpos
git commit -m "feat: TicketSink with TCP transport and a fake printer for tests"
```

---

### Task 9: Wire the fake printer into the test harness

**Files:**
- Modify: `crates/server/tests/common/mod.rs`
- Create: `crates/server/tests/harness.rs`
- Modify: `crates/server/Cargo.toml`

**Interfaces:**
- Consumes: `TestApp` (Task 7), `FakePrinter`, `TcpSink`, `TicketSink` (Task 8)
- Produces: `TestApp::printer: FakePrinter`, `TestApp::spawn_with_printer(FakeMode) -> TestApp`

- [ ] **Step 1: Write the failing test**

Create `crates/server/tests/harness.rs`:

```rust
mod common;

use common::TestApp;
use grubsi_escpos::sink::TicketSink;
use grubsi_escpos::transport::fake::FakeMode;
use grubsi_escpos::transport::tcp::TcpSink;

#[tokio::test]
async fn the_harness_provides_a_printer_that_records_what_it_receives() {
    // M4's print queue will assert on these bytes. Proving the wiring now
    // means that milestone starts with a working test double.
    let app = TestApp::spawn_with_printer(FakeMode::Ok).await;

    let sink = TcpSink::new(app.printer.addr());
    sink.send(b"KOT TABLE 07\n").await.unwrap();

    assert_eq!(app.printer.wait_for_job().await, b"KOT TABLE 07\n");
}

#[tokio::test]
async fn the_harness_serves_the_real_router() {
    let app = TestApp::spawn().await;
    let body = app.get_json("/api/v1/health").await;
    assert_eq!(body["status"], "ok");
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test --package grubsi-server --test harness`
Expected: FAIL — `no function spawn_with_printer`.

- [ ] **Step 3: Extend the harness**

Update `crates/server/tests/common/mod.rs` — add the import and field, and route `spawn` through the new constructor:

```rust
use grubsi_escpos::transport::fake::{FakeMode, FakePrinter};

pub struct TestApp {
    pub addr: SocketAddr,
    pub state: AppState,
    pub printer: FakePrinter,
    _dir: TempDir,
}

impl TestApp {
    pub async fn spawn() -> Self {
        Self::spawn_with_printer(FakeMode::Ok).await
    }

    pub async fn spawn_with_printer(mode: FakeMode) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open(&dir.path().join("grubsi.db")).await.unwrap();
        let state = AppState::new(db);
        let printer = FakePrinter::start(mode).await;

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let router = build_router(state.clone());

        tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });

        Self { addr, state, printer, _dir: dir }
    }

    // url / post / get_json unchanged
}
```

No manifest change is needed. Cargo makes both `[dependencies]` and `[dev-dependencies]` available to test targets, and `grubsi-escpos` is already a normal dependency of `grubsi-server` from Task 1.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test --package grubsi-server`
Expected: PASS — all suites, including the 2 new harness tests.

- [ ] **Step 5: Commit**

```bash
git add crates/server
git commit -m "test: integration harness with temp database, router, and fake printer"
```

---

### Task 10: Static assets — embedded release build, SPA fallback, cold-clone safety

The dev loop runs Vite on :5173 proxying to the Rust server on :8080, so no Rust-side dev feature is needed. The binary only serves assets in release. A cold clone must still compile, which means `web/dist` has to exist in git.

**Files:**
- Create: `web/dist/.gitkeep`
- Create: `crates/server/src/infra/assets.rs`
- Modify: `crates/server/src/lib.rs`, `crates/server/src/infra/mod.rs`
- Create: `crates/server/tests/assets.rs`

**Interfaces:**
- Consumes: `AppError` (Task 2)
- Produces: `pub async fn serve_asset(uri: Uri) -> Response` — the router's outermost fallback

- [ ] **Step 1: Write the failing test**

Create `crates/server/tests/assets.rs`:

```rust
mod common;

use common::TestApp;

#[tokio::test]
async fn unknown_ui_routes_fall_back_to_the_spa() {
    // TanStack Router owns client-side routing; a deep link typed into a
    // tablet must reach index.html, not a 404.
    let app = TestApp::spawn().await;

    let response = reqwest::get(app.url("/steward/floor/3")).await.unwrap();
    assert_eq!(response.status(), 200);

    let content_type =
        response.headers().get("content-type").unwrap().to_str().unwrap().to_owned();
    assert!(content_type.starts_with("text/html"), "got {content_type}");
}

#[tokio::test]
async fn api_routes_still_win_over_the_spa_fallback() {
    let app = TestApp::spawn().await;

    let response = reqwest::get(app.url("/api/v1/nope")).await.unwrap();
    assert_eq!(response.status(), 404);
    let content_type =
        response.headers().get("content-type").unwrap().to_str().unwrap().to_owned();
    assert!(content_type.contains("application/json"), "got {content_type}");
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test --package grubsi-server --test assets`
Expected: FAIL — the first test 404s, because there is no fallback yet.

- [ ] **Step 3: Write the implementation**

Create `web/dist/.gitkeep` (empty file). The `.gitignore` rule from Task 1 (`web/dist/*` plus `!web/dist/.gitkeep`) keeps the directory in git while ignoring build output. Without it, `rust-embed` fails to compile on a fresh clone.

`crates/server/src/infra/assets.rs`:

```rust
use axum::http::{StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};
use rust_embed::RustEmbed;

/// The built frontend, baked into the binary.
///
/// In development this is empty and unused: Vite serves the app on :5173
/// and proxies /api and /ws here. `web/dist/.gitkeep` keeps the directory
/// present so a clean checkout compiles.
#[derive(RustEmbed)]
#[folder = "../../web/dist"]
struct Assets;

const PLACEHOLDER: &str = "<!doctype html><meta charset=utf-8>\
<title>grubsi</title>\
<body style=\"font-family:system-ui;max-width:34rem;margin:4rem auto;line-height:1.6\">\
<h1>Frontend not built</h1>\
<p>The API is running. To build the interface:</p>\
<pre>npm --prefix web ci &amp;&amp; npm --prefix web run build</pre>\
<p>For the development loop, run <code>just web</code> and open \
<a href=\"http://localhost:5173\">localhost:5173</a> instead.</p>";

pub async fn serve_asset(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');

    if let Some(file) = Assets::get(path) {
        let mime = mime_guess::from_path(path).first_or_octet_stream();
        return ([(header::CONTENT_TYPE, mime.as_ref())], file.data).into_response();
    }

    // Anything else is a client-side route: hand back the shell.
    match Assets::get("index.html") {
        Some(index) => (
            [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
            index.data,
        )
            .into_response(),
        None => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
            PLACEHOLDER,
        )
            .into_response(),
    }
}
```

Add `pub mod assets;` to `crates/server/src/infra/mod.rs`.

Add the fallback in `build_router` — it must be outermost, so `/api` and `/ws` match first:

```rust
    Router::new()
        .nest("/api/v1", api)
        .route("/api-docs/openapi.json", get(openapi_json))
        .route("/ws", get(infra::ws::ws_handler))
        .fallback(infra::assets::serve_asset)
        .with_state(state)
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test --package grubsi-server --test assets`
Expected: PASS, 2 tests. The first serves the placeholder, which is still `text/html`.

- [ ] **Step 5: Verify the cold-clone build**

Run:

```bash
git stash --include-untracked
rm -rf /tmp/grubsi-cold && git clone . /tmp/grubsi-cold
cd /tmp/grubsi-cold && cargo build --workspace
cd - && git stash pop
```

Expected: the clone builds with no frontend present. If `rust-embed` errors that the folder is missing, `web/dist/.gitkeep` was not committed.

- [ ] **Step 6: Commit**

```bash
git add web/dist/.gitkeep crates/server .gitignore
git commit -m "feat: embedded SPA assets with fallback and cold-clone safety"
```

---

### Task 11: The React app and the event stream client

**Files:**
- Create: `web/package.json`, `web/vite.config.ts`, `web/tsconfig.json`, `web/tsconfig.node.json`, `web/index.html`
- Create: `web/src/main.tsx`, `web/src/App.tsx`
- Create: `web/src/lib/eventStream.ts`, `web/src/lib/eventStream.test.ts`

**Interfaces:**
- Consumes: `GET /api/v1/health`, `POST /api/v1/dev/ping`, `GET /ws` (Tasks 3, 7)
- Produces:
  - `export type Frame` — the discriminated union of server frames
  - `export type StreamState = { bootId: string | null; lastSeq: number }`
  - `export function reduce(state: StreamState, frame: Frame): { state: StreamState; action: "event" | "resync" | "ignore" }`
  - `export function connect(url: string, handlers): () => void`

- [ ] **Step 1: Write the failing test**

Create `web/src/lib/eventStream.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { initialState, reduce, type Frame } from "./eventStream";

const hello = (bootId: string, seq = 0): Frame => ({ type: "HELLO", boot_id: bootId, seq });

const event = (bootId: string, seq: number): Frame => ({
  type: "EVENT",
  envelope: { boot_id: bootId, seq, kind: "PING", topic: "staff", payload: null, at: "" },
});

describe("event stream reducer", () => {
  it("accepts events that arrive in order", () => {
    let s = reduce(initialState, hello("boot-1")).state;
    const first = reduce(s, event("boot-1", 1));
    expect(first.action).toBe("event");
    expect(first.state.lastSeq).toBe(1);

    const second = reduce(first.state, event("boot-1", 2));
    expect(second.action).toBe("event");
    expect(second.state.lastSeq).toBe(2);
  });

  it("asks for a resync when a sequence number is skipped", () => {
    // A dropped event means the client's view is stale in ways it cannot
    // reconstruct. Refetching is the only correct response.
    let s = reduce(initialState, hello("boot-1")).state;
    s = reduce(s, event("boot-1", 1)).state;

    const skipped = reduce(s, event("boot-1", 5));
    expect(skipped.action).toBe("resync");
    expect(skipped.state.lastSeq).toBe(5);
  });

  it("asks for a resync when the server has restarted", () => {
    // A new boot_id means sequence numbers started over; everything the
    // client holds may be stale.
    let s = reduce(initialState, hello("boot-1")).state;
    s = reduce(s, event("boot-1", 4)).state;

    const restarted = reduce(s, event("boot-2", 1));
    expect(restarted.action).toBe("resync");
    expect(restarted.state.bootId).toBe("boot-2");
    expect(restarted.state.lastSeq).toBe(1);
  });

  it("treats an explicit RESYNC frame as a resync", () => {
    const s = reduce(initialState, hello("boot-1")).state;
    expect(reduce(s, { type: "RESYNC" }).action).toBe("resync");
  });
});
```

- [ ] **Step 2: Scaffold the frontend and run the test to verify it fails**

`web/package.json`:

```json
{
  "name": "grubsi-web",
  "private": true,
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "tsc -b && vite build",
    "preview": "vite preview",
    "test": "vitest run",
    "gen:api": "openapi-typescript ../openapi.json -o src/lib/api/schema.ts"
  },
  "dependencies": {
    "@tanstack/react-query": "^5.62.0",
    "react": "^19.0.0",
    "react-dom": "^19.0.0"
  },
  "devDependencies": {
    "@types/react": "^19.0.0",
    "@types/react-dom": "^19.0.0",
    "@vitejs/plugin-react": "^4.3.0",
    "openapi-typescript": "^7.4.0",
    "typescript": "^5.7.0",
    "vite": "^6.0.0",
    "vitest": "^2.1.0"
  }
}
```

`web/vite.config.ts`:

```ts
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

// The development loop: Vite serves the app with HMR on 5173 and proxies
// API and socket traffic to the Rust server on 8080. Nothing is embedded
// in the binary until `npm run build`.
export default defineConfig({
  plugins: [react()],
  server: {
    port: 5173,
    proxy: {
      "/api": "http://localhost:8080",
      "/ws": { target: "ws://localhost:8080", ws: true },
    },
  },
  build: { outDir: "dist", emptyOutDir: true },
});
```

`web/tsconfig.json`:

```json
{
  "compilerOptions": {
    "target": "ES2022",
    "lib": ["ES2022", "DOM", "DOM.Iterable"],
    "module": "ESNext",
    "moduleResolution": "bundler",
    "jsx": "react-jsx",
    "strict": true,
    "noUnusedLocals": true,
    "noEmit": true,
    "skipLibCheck": true,
    "types": ["vite/client"]
  },
  "include": ["src"]
}
```

`web/index.html`:

```html
<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>grubsi</title>
  </head>
  <body>
    <div id="root"></div>
    <script type="module" src="/src/main.tsx"></script>
  </body>
</html>
```

Run: `npm --prefix web install && npm --prefix web run test`
Expected: FAIL — `Failed to resolve import "./eventStream"`.

- [ ] **Step 3: Write the event stream client**

`web/src/lib/eventStream.ts`:

```ts
export type Envelope = {
  boot_id: string;
  seq: number;
  kind: string;
  topic: string;
  payload: unknown;
  at: string;
};

export type Frame =
  | { type: "HELLO"; boot_id: string; seq: number }
  | { type: "EVENT"; envelope: Envelope }
  | { type: "RESYNC" };

export type StreamState = { bootId: string | null; lastSeq: number };

export const initialState: StreamState = { bootId: null, lastSeq: 0 };

export type Action = "event" | "resync" | "ignore";

/**
 * Decide what a frame means, given what we have already seen.
 *
 * Two conditions demand a full refetch rather than an incremental update:
 * a sequence gap (an event was dropped) and a changed boot_id (the server
 * restarted, so sequence numbers began again). Both are unrecoverable from
 * the client's cache, and both have the same remedy.
 */
export function reduce(
  state: StreamState,
  frame: Frame,
): { state: StreamState; action: Action } {
  switch (frame.type) {
    case "HELLO":
      return {
        state: { bootId: frame.boot_id, lastSeq: frame.seq },
        action: "ignore",
      };

    case "RESYNC":
      return { state, action: "resync" };

    case "EVENT": {
      const { boot_id: bootId, seq } = frame.envelope;
      const restarted = state.bootId !== null && state.bootId !== bootId;
      const gap = !restarted && seq !== state.lastSeq + 1;

      return {
        state: { bootId, lastSeq: seq },
        action: restarted || gap ? "resync" : "event",
      };
    }
  }
}

type Handlers = {
  onEvent: (envelope: Envelope) => void;
  /** Everything the client holds may be stale. Refetch. */
  onResync: () => void;
};

/** Connect, reconnecting with backoff. Returns a teardown function. */
export function connect(url: string, handlers: Handlers): () => void {
  let state = initialState;
  let socket: WebSocket | null = null;
  let retry = 0;
  let timer: ReturnType<typeof setTimeout> | undefined;
  let closed = false;

  const open = () => {
    if (closed) return;
    socket = new WebSocket(url);

    socket.onopen = () => {
      retry = 0;
    };

    socket.onmessage = (message) => {
      const frame = JSON.parse(message.data as string) as Frame;
      const result = reduce(state, frame);
      state = result.state;

      if (result.action === "event" && frame.type === "EVENT") {
        handlers.onEvent(frame.envelope);
      } else if (result.action === "resync") {
        handlers.onResync();
      }
    };

    socket.onclose = () => {
      if (closed) return;
      // A dropped socket means missed events; refetch once reconnected.
      const delay = Math.min(1000 * 2 ** retry, 15_000);
      retry += 1;
      timer = setTimeout(open, delay);
      handlers.onResync();
    };
  };

  open();

  return () => {
    closed = true;
    if (timer) clearTimeout(timer);
    socket?.close();
  };
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `npm --prefix web run test`
Expected: PASS, 4 tests.

- [ ] **Step 5: Build the page**

`web/src/main.tsx`:

```tsx
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import React from "react";
import ReactDOM from "react-dom/client";
import { App } from "./App";

const queryClient = new QueryClient();

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <QueryClientProvider client={queryClient}>
      <App />
    </QueryClientProvider>
  </React.StrictMode>,
);
```

`web/src/App.tsx`:

```tsx
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { connect } from "./lib/eventStream";

type Health = { status: string; version: string; uptime_seconds: number };

export function App() {
  const queryClient = useQueryClient();
  const [received, setReceived] = useState(0);

  const health = useQuery({
    queryKey: ["health"],
    queryFn: async (): Promise<Health> => {
      const response = await fetch("/api/v1/health");
      if (!response.ok) throw new Error("Could not reach the server.");
      return response.json();
    },
  });

  useEffect(() => {
    const url = `${location.protocol === "https:" ? "wss" : "ws"}://${location.host}/ws`;
    return connect(url, {
      onEvent: () => setReceived((n) => n + 1),
      // The rule from the spec: an event never patches the cache, it only
      // invalidates. The server stays authoritative by construction.
      onResync: () => queryClient.invalidateQueries(),
    });
  }, [queryClient]);

  return (
    <main style={{ fontFamily: "system-ui", maxWidth: "34rem", margin: "4rem auto", lineHeight: 1.6 }}>
      <h1>grubsi</h1>
      <p>
        Server:{" "}
        {health.isPending ? "checking…" : health.isError ? "unreachable" : `ok, v${health.data.version}`}
      </p>
      <p>Events received: {received}</p>
      <button onClick={() => fetch("/api/v1/dev/ping", { method: "POST" })}>
        Publish an event
      </button>
    </main>
  );
}
```

- [ ] **Step 6: Verify the build and the dev loop**

Run: `npm --prefix web run build`
Expected: `web/dist/index.html` and hashed assets exist.

Then, in two shells: `just dev` and `just web`. Open `http://localhost:5173`, click **Publish an event**, and watch the counter increase.

- [ ] **Step 7: Commit**

```bash
git add web
git commit -m "feat: React app with a resync-aware event stream client"
```

---

### Task 12: TypeScript client generation and drift check

**Files:**
- Modify: `justfile`, `.github/workflows/ci.yml`
- Create: `web/src/lib/api/schema.ts` (generated, committed)

**Interfaces:**
- Consumes: `dump_openapi` binary (Task 3)
- Produces: `just gen-api`; CI fails when the committed client no longer matches the routes

- [ ] **Step 1: Write the failing check**

Add to `justfile`:

```make
# Regenerate the TypeScript API client from the server's routes.
gen-api:
    cargo run --quiet --package grubsi-server --bin dump_openapi > openapi.json
    npm --prefix web run gen:api

# Fails if the committed client is stale. Run `just gen-api` to fix.
check-api: gen-api
    git diff --exit-code -- web/src/lib/api/schema.ts
```

- [ ] **Step 2: Run it to verify it fails**

Run: `just check-api`
Expected: FAIL — `web/src/lib/api/schema.ts` does not exist yet, so `git diff` reports it as untracked/new. (`openapi-typescript` must also be installed; it is in the Task 11 devDependencies.)

- [ ] **Step 3: Generate and commit the client**

Run: `just gen-api`

Confirm `web/src/lib/api/schema.ts` now contains a `paths` interface with `/api/v1/health`. Add a line to `.gitignore` for `openapi.json` if Task 1's version is missing it — the JSON is a build intermediate, the generated `.ts` is committed.

- [ ] **Step 4: Verify the check now passes**

Run: `just check-api`
Expected: PASS, no diff.

- [ ] **Step 5: Add it to CI**

In `.github/workflows/ci.yml`, add a job:

```yaml
  api-contract:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@1.97
      - uses: Swatinem/rust-cache@v2
      - uses: actions/setup-node@v4
        with:
          node-version: '26'
      - run: npm --prefix web ci
      - name: Generated API client is up to date
        run: |
          cargo run --quiet --package grubsi-server --bin dump_openapi > openapi.json
          npm --prefix web run gen:api
          git diff --exit-code -- web/src/lib/api/schema.ts
```

- [ ] **Step 6: Commit**

```bash
git add justfile .github web/src/lib/api/schema.ts .gitignore
git commit -m "build: generate the TypeScript API client and gate drift in CI"
```

---

### Task 13: M0 acceptance

No new production code. This task proves the milestone's definition of done and records how to run the system.

**Files:**
- Create: `README.md` (replace the stub)

**Interfaces:**
- Consumes: everything
- Produces: a verified walking skeleton

- [ ] **Step 1: Run the whole test suite**

Run: `just check && just test`
Expected: fmt clean, clippy clean with `-D warnings`, core boundary OK, all Rust tests pass, all Vitest tests pass.

- [ ] **Step 2: Verify the release path end to end**

Run:

```bash
just build
GRUBSI_DB=/tmp/grubsi-acceptance.db ./target/release/grubsi-server
```

Expected: the log prints `reachable at http://<lan-ip>:8080`.

- [ ] **Step 3: Verify the milestone's definition of done**

From a **different device on the same network**, open `http://<lan-ip>:8080`. Confirm:

1. The page loads (embedded assets, not Vite).
2. It reads `Server: ok, v0.1.0`.
3. Clicking **Publish an event** increases the counter.
4. Opening the page on a second device and clicking there increases the counter on **both** — one broadcast, two subscribers.

That fourth check is the milestone: an event travelling from the server to every connected client.

- [ ] **Step 4: Verify restart recovery**

With both pages open, stop the server (Ctrl-C) and start it again. Expected: each page reconnects within a few seconds and refetches — the health line returns to `ok` without a manual reload. This is the `boot_id` change driving a resync.

- [ ] **Step 5: Write the README**

Replace `README.md`:

```markdown
# grubsi

A local-first restaurant and bar POS. The restaurant keeps operating when
the internet does not: one Rust binary on the LAN is the source of truth.

- **Design:** [docs/superpowers/specs/2026-08-31-grubsi-architecture-design.md](docs/superpowers/specs/2026-08-31-grubsi-architecture-design.md)
- **Requirements:** [docs/MVP.md](docs/MVP.md)

## Requirements

Rust 1.97, Node 26, SQLite 3.37+ (for STRICT tables).

## Development

Two processes. Vite serves the frontend with hot reload and proxies API
and socket traffic to the Rust server:

```bash
just dev    # Rust API on :8080
just web    # Vite on :5173  ← open this one
```

## Checks

```bash
just check      # fmt, clippy, core dependency boundary
just test       # Rust + Vitest
just check-api  # the generated TS client matches the routes
```

## Release

```bash
just build
./target/release/grubsi-server
```

Builds the frontend into `web/dist`, embeds it in the binary, and serves
everything on port 8080 across all interfaces. Override with `GRUBSI_PORT`
and `GRUBSI_DB`.

## Layout

| Path | Contents |
|---|---|
| `crates/core` | Pure domain logic. No I/O — enforced by CI. |
| `crates/escpos` | Printer transports and the test double. |
| `crates/server` | axum, sqlx, features, the write path. |
| `web` | React app for all four surfaces. |
| `migrations` | Versioned SQLite schema. |
```

- [ ] **Step 6: Commit**

```bash
git add README.md
git commit -m "docs: README covering the dev loop, checks, and release build"
```

---

## Self-Review

**Spec coverage.** Every item in the spec's M0 — workspace and three crates, CI, migrations and the dual pool, `write_tx`, axum plus embedded React with SPA fallback, `AppError`, `tracing`, one WebSocket round-trip, OpenAPI → TypeScript codegen, and the integration test harness — maps to a task. The deferred Tier-4 dev-loop finding is covered by Tasks 10 through 13. Two deliberate deviations are recorded under Scope Notes rather than left implicit.

**Not in M0, by design.** Money types, permissions, and the `Permission` enum belong to M1 and M3. `core` ships with only `event.rs` in this milestone; that is intentional, not an omission.

**Known sequencing hazard.** Task 5's third test depends on `DomainEvent` from Task 6. The task says so explicitly and gives the workaround for strict one-task-at-a-time execution.

**Type consistency.** Names were checked across task boundaries: `AppState::new` gains its `db` argument in Task 4 and Task 4 updates the Task 3 tests that call it; `build_router` applies state only at the outermost router, so `nest` sees matching state types in both Task 3 and Task 7; `Frame` is matched with `matches!` rather than `PartialEq`, since it wraps a non-comparable `Envelope`; `frame_for`, `EventHub::current_seq`, `FakePrinter::wait_for_job`, and `TcpSink::with_timeout` are each defined in the task before the one that calls them.

**Verified against the installed toolchain rather than assumed.** Rust 1.97, Node 26.5, npm 12, SQLite 3.50.4. `sqlx-cli`, `cargo-watch` and `just` are absent — only `just` is used, and Task 1 installs it. Crate versions were read from crates.io at the time of writing: axum 0.8.9, tokio 1.53.1, sqlx 0.9.0, utoipa 5.5.0, rust-embed 8.12.0. The sqlx 0.9 pool and migration API was checked against its documentation, and axum 0.8's `{param}` path syntax and `axum::serve` signature against its 0.8.4 docs.
