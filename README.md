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
