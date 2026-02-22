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

## Implementations

See each subfolder for language-specific build and run instructions:

- [go/README.md](go/README.md) — single binary, no CGO, embed
- [rust/README.md](rust/README.md) — Axum + Tokio, `cargo build --release`
- [python/README.md](python/README.md) — FastAPI + PyInstaller contrast
