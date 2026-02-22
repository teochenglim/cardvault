# CardVault — Rust

Rust implementation of CardVault using Axum for HTTP, Tokio for async, and rusqlite for SQLite.

## Stack

| Component | Crate |
|---|---|
| HTTP framework | `axum` |
| Async runtime | `tokio` |
| SQLite | `rusqlite` with `bundled` feature (statically linked) |
| Static files | `rust-embed` |
| Serialization | `serde` / `serde_json` |

## Files

```
rust/
├── src/
│   ├── main.rs       # Entry point, CLI flags, router setup, graceful shutdown
│   ├── store.rs      # SQLite schema, connection pool, CRUD
│   ├── models.rs     # Struct definitions, Serialize/Deserialize
│   └── handlers.rs   # Axum handler functions, multipart parsing
├── static/
│   └── index.html    # SPA (embedded into binary via rust-embed)
├── uploads/          # Runtime photo storage (gitignored)
└── Cargo.toml
```

## Requirements

- Rust (via `brew install rust`)
- Xcode Command Line Tools for the C compiler used by `rusqlite --features bundled`:
  ```bash
  xcode-select --install
  ```

## Build

```bash
cd rust
cargo build --release
strip target/release/cardvault   # optional: sheds debug symbols
```

The binary is at `target/release/cardvault`.

## Run

```bash
# Default settings (port 8080, cardvault.db, uploads/)
./target/release/cardvault

# Custom settings
./target/release/cardvault --port 9090 --db /data/cards.db --uploads-dir /data/uploads

# Load seed data
./target/release/cardvault --seed

# Via environment variables
PORT=9090 CARDVAULT_DB=/data/cards.db ./target/release/cardvault
```

Then open [http://localhost:8080](http://localhost:8080) in your browser.

## CLI Flags

| Flag | ENV | Default | Description |
|---|---|---|---|
| `--port` | `PORT` | `8080` | HTTP listen port |
| `--db` | `CARDVAULT_DB` | `cardvault.db` | SQLite database file |
| `--uploads-dir` | `CARDVAULT_UPLOADS` | `uploads/` | Directory for uploaded photos |
| `--seed` | — | false | Insert seed data if DB is empty |

## Development

Run in dev mode:
```bash
cargo run
```

Run with auto-reload (requires `cargo-watch`):
```bash
cargo install cargo-watch
cargo watch -x run
```

Check without building:
```bash
cargo check
```

## Design Notes

- `rusqlite` with `--features bundled` compiles SQLite from source into the binary — no system SQLite dependency
- `rust-embed` bakes `static/index.html` into the binary at compile time
- Axum's `multipart` extractor handles photo uploads; files are streamed to disk
- `tokio::signal` is used for graceful shutdown — in-flight requests complete before the server stops
- A `Mutex<Connection>` or connection pool (e.g. `r2d2`) wraps the SQLite connection for concurrent access
- `uploads/` is created at startup if it does not exist; it is `.gitignore`d
