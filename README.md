# CardVault

A business card management application implemented in three languages — **Go**, **Rust**, and **Python** — sharing identical REST API, data model, and UI.

Each implementation produces a self-contained server with an embedded SQLite database and a single-page web UI.

## Project Structure

```
cardvault/
├── go/         # Go implementation — net/http stdlib, modernc SQLite (no CGO)
├── rust/       # Rust implementation — Axum, rusqlite, Tokio
├── python/     # Python implementation — FastAPI, aiosqlite, Uvicorn
└── README.md
```

## Feature Overview

- Store and manage business cards with photo, phones, emails, addresses, and tags
- Full CRUD via REST API
- Single-page UI: card grid, search, tag filtering, add/edit modal
- Photo upload (jpg/png/webp, max 5 MB)
- SQLite — embedded, no external DB process required

## Data Model

| Table | Purpose |
|---|---|
| `cards` | Core card record |
| `card_phones` | Multiple phones per card |
| `card_emails` | Multiple emails per card |
| `card_addresses` | Multiple addresses per card |
| `tags` | Tag dictionary |
| `card_tags` | Many-to-many junction |

## REST API

```
GET    /api/cards              List cards; ?q= search, ?tag= filter
POST   /api/cards              Create card (multipart/form-data)
GET    /api/cards/:id          Full card detail
PUT    /api/cards/:id          Update card (multipart/form-data)
DELETE /api/cards/:id          Delete card + cascade

POST   /api/cards/:id/photo    Upload photo, returns photo_url
DELETE /api/cards/:id/photo    Remove photo

GET    /uploads/:filename      Serve uploaded photo files

GET    /api/tags               List tags with usage count
GET    /health                 { status: ok, db: ok }
GET    /                       Serve UI
```

## curl Examples

### Create a card
```bash
curl -X POST http://localhost:8080/api/cards \
  -F 'name=Ada Lovelace' \
  -F 'title=Software Engineer' \
  -F 'company=Acme Corp' \
  -F 'website=https://ada.dev' \
  -F 'notes=Met at GovTech conference' \
  -F 'phones=[{"label":"mobile","number":"+65 9123 4567"}]' \
  -F 'emails=[{"label":"work","address":"ada@acme.com"}]' \
  -F 'addresses=[{"label":"office","street":"1 Fusionopolis Way","city":"Singapore","country":"Singapore","postal":"138632"}]' \
  -F 'tags=["fintech","government"]'
```

### List all cards
```bash
curl http://localhost:8080/api/cards
```

### Search cards
```bash
curl "http://localhost:8080/api/cards?q=ada"
```

### Filter by tag
```bash
curl "http://localhost:8080/api/cards?tag=fintech"
```

### Get a card
```bash
curl http://localhost:8080/api/cards/1
```

### Update a card
```bash
curl -X PUT http://localhost:8080/api/cards/1 \
  -F 'name=Ada Lovelace' \
  -F 'title=Principal Engineer' \
  -F 'tags=["fintech","client"]'
```

### Upload a photo
```bash
curl -X POST http://localhost:8080/api/cards/1/photo \
  -F 'photo=@headshot.jpg'
```

### Delete a photo
```bash
curl -X DELETE http://localhost:8080/api/cards/1/photo
```

### Delete a card
```bash
curl -X DELETE http://localhost:8080/api/cards/1
```

### List tags
```bash
curl http://localhost:8080/api/tags
```

### Health check
```bash
curl http://localhost:8080/health
```

## CLI Flags (all implementations)

| Flag | ENV | Default | Description |
|---|---|---|---|
| `--port` | `PORT` | `8080` | HTTP port |
| `--db` | `CARDVAULT_DB` | `cardvault.db` | SQLite file path |
| `--uploads-dir` | `CARDVAULT_UPLOADS` | `uploads/` | Photo storage dir |
| `--seed` | — | false | Load seed data on first run |

## Size Comparison

Measured on Apple Silicon (macOS). Docker images target `linux/amd64` via `alpine:3.20` / `python:3.11-slim-bookworm`.

| | Go | Rust | Python |
|---|---|---|---|
| **Native binary** | 14 MB | 4.2 MB | — |
| **Docker image** | 10.9 MB | 8.9 MB | 61.8 MB |
| **Docker base** | `alpine:3.20` | `alpine:3.20` | `python:3.11-slim` |
| **Single binary** | ✓ | ✓ | ✗ (`.venv`) |
| **CGO required** | No | Yes (bundled SQLite) | No |
| **Static files** | embedded (`go:embed`) | embedded (`rust-embed`) → extracted at startup | copied into image |

> Go native binary is unstripped (macOS dev build). The Docker Linux binary with `-ldflags="-s -w"` is ~3 MB. Rust binary has `strip = true` in `Cargo.toml`. Python has no native binary — runtime ships as a virtualenv.

## Docker

```bash
# Build
docker build -t cardvault-go     ./go
docker build -t cardvault-python  ./python
docker build -t cardvault-rust    ./rust

# Run (each on a different port)
docker run -d -p 8080:8080 --name cardvault-go     cardvault-go  /app/cardvault --seed
docker run -d -p 8081:8080 --name cardvault-python  cardvault-python python main.py --seed
docker run -d -p 8082:8080 --name cardvault-rust    cardvault-rust /app/cardvault --seed

# Stop & remove
docker stop cardvault-go     && docker rm cardvault-go
docker stop cardvault-python && docker rm cardvault-python
docker stop cardvault-rust   && docker rm cardvault-rust
```

> Data lives in anonymous Docker volumes (declared via `VOLUME` in each Dockerfile) and survives restarts. Add `-v cv-data:/app/data -v cv-uploads:/app/uploads` for named volumes that persist across `docker rm`.

## Implementations

See each subfolder for language-specific build and run instructions:

- [go/README.md](go/README.md) — single binary, no CGO, embed
- [rust/README.md](rust/README.md) — Axum + Tokio, `cargo build --release`
- [python/README.md](python/README.md) — FastAPI + PyInstaller contrast
