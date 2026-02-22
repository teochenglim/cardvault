# CardVault — Go

Go implementation of CardVault using the standard library and a pure-Go SQLite driver (no CGO required).

## Stack

| Component | Library |
|---|---|
| HTTP server | `net/http` stdlib |
| SQLite | `modernc.org/sqlite` (pure Go, no CGO) |
| Static files | `//go:embed static/*` baked into binary |
| No framework | stdlib only |

## Files

```
go/
├── main.go        # Entry point, CLI flags, graceful shutdown
├── store.go       # SQLite schema, CRUD, query helpers
├── models.go      # Struct definitions, JSON tags
├── handlers.go    # HTTP handlers, routing, multipart parsing
├── static/
│   └── index.html # SPA (embedded into binary at build time)
├── uploads/       # Runtime photo storage (gitignored)
└── go.mod
```

## Requirements

- Go 1.21+
- No C compiler needed (`modernc.org/sqlite` is pure Go)

## Build

```bash
cd go
go mod download
go build -o cardvault .
```

Cross-compile (e.g. Linux arm64 on macOS):
```bash
GOOS=linux GOARCH=arm64 go build -o cardvault-linux-arm64 .
```

## Run

```bash
# Default settings (port 8080, cardvault.db, uploads/)
./cardvault

# Custom settings
./cardvault --port 9090 --db /data/cards.db --uploads-dir /data/uploads

# Load seed data (10 realistic SE Asia contacts)
./cardvault --seed

# Via environment variables
PORT=9090 CARDVAULT_DB=/data/cards.db ./cardvault
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

Run without building:
```bash
go run .
```

Run with race detector:
```bash
go run -race .
```

## Design Notes

- `//go:embed static/*` bakes `index.html` into the binary at compile time — no separate asset files needed at runtime
- `modernc.org/sqlite` translates the SQLite C source to Go, so CGO is not required and the binary is fully portable
- `net/http` is used directly without a router framework; routes are matched with a simple prefix switch
- Graceful shutdown: `os.Signal` listener catches `SIGINT`/`SIGTERM` and calls `http.Server.Shutdown` with a timeout
- `uploads/` is created at startup if it does not exist; it is `.gitignore`d
