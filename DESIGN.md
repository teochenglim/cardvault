📇 CardVault — Final Requirements
Project Structure
cardvault/
├── go/
│   ├── main.go
│   ├── store.go
│   ├── models.go
│   ├── handlers.go
│   ├── static/
│   │   └── index.html
│   ├── uploads/          # gitignored, created at runtime
│   └── go.mod
├── rust/
│   ├── src/
│   │   ├── main.rs
│   │   ├── store.rs
│   │   ├── models.rs
│   │   └── handlers.rs
│   ├── static/
│   │   └── index.html    # same HTML reused
│   ├── uploads/
│   └── Cargo.toml
├── python/
│   ├── main.py
│   ├── store.py
│   ├── models.py
│   ├── static/
│   │   └── index.html    # same HTML reused
│   ├── uploads/
│   └── requirements.txt
└── README.md

Data Model
Table: cards
sqlid           INTEGER PRIMARY KEY AUTOINCREMENT
name         TEXT NOT NULL
title        TEXT                -- job title
company      TEXT
email        TEXT                -- primary email
website      TEXT
notes        TEXT
photo_path   TEXT                -- relative path e.g. uploads/abc123.jpg
created_at   DATETIME DEFAULT CURRENT_TIMESTAMP
updated_at   DATETIME DEFAULT CURRENT_TIMESTAMP
Table: card_phones
sqlid       INTEGER PRIMARY KEY AUTOINCREMENT
card_id  INTEGER REFERENCES cards(id) ON DELETE CASCADE
label    TEXT    -- "mobile", "work", "home", "fax"
number   TEXT
Table: card_emails
sqlid       INTEGER PRIMARY KEY AUTOINCREMENT
card_id  INTEGER REFERENCES cards(id) ON DELETE CASCADE
label    TEXT    -- "work", "personal"
address  TEXT
Table: card_addresses
sqlid       INTEGER PRIMARY KEY AUTOINCREMENT
card_id  INTEGER REFERENCES cards(id) ON DELETE CASCADE
label    TEXT    -- "office", "home"
street   TEXT
city     TEXT
country  TEXT
postal   TEXT
Table: tags
sqlid    INTEGER PRIMARY KEY AUTOINCREMENT
name  TEXT UNIQUE
Table: card_tags (junction)
sqlcard_id  INTEGER REFERENCES cards(id) ON DELETE CASCADE
tag_id   INTEGER REFERENCES tags(id) ON DELETE CASCADE
PRIMARY KEY (card_id, tag_id)
```

---

## REST API
```
# Cards
GET    /api/cards              list all cards, ?q= search, ?tag= filter
POST   /api/cards              create card (multipart/form-data for photo)
GET    /api/cards/:id          get full card with all related fields
PUT    /api/cards/:id          update card (multipart/form-data)
DELETE /api/cards/:id          delete card + cascade all related rows

# Photo
POST   /api/cards/:id/photo    upload photo (multipart), returns photo_url
DELETE /api/cards/:id/photo    remove photo, delete file from disk

# Serve uploaded files
GET    /uploads/:filename       serve photo files

# Tags
GET    /api/tags               list all tags with usage count

# Health
GET    /health                 { status: ok, db: ok }

# UI
GET    /                       serve embedded index.html

Request / Response Shape
Card (full response)
json{
  "id": 1,
  "name": "Ada Lovelace",
  "title": "Software Engineer",
  "company": "Acme Corp",
  "website": "https://ada.dev",
  "notes": "Met at GovTech conference",
  "photo_url": "/uploads/abc123.jpg",
  "phones": [
    { "id": 1, "label": "mobile", "number": "+65 9123 4567" },
    { "id": 2, "label": "work",   "number": "+65 6123 4567" }
  ],
  "emails": [
    { "id": 1, "label": "work",     "address": "ada@acme.com" },
    { "id": 2, "label": "personal", "address": "ada@gmail.com" }
  ],
  "addresses": [
    {
      "id": 1,
      "label": "office",
      "street": "1 Fusionopolis Way",
      "city": "Singapore",
      "country": "Singapore",
      "postal": "138632"
    }
  ],
  "tags": ["fintech", "government"],
  "created_at": "2026-01-01T00:00:00Z",
  "updated_at": "2026-01-01T00:00:00Z"
}
```

### Create / Update (multipart/form-data)
```
name        string (required)
title       string
company     string
website     string
notes       string
photo       file (jpg/png, max 5MB)
phones      JSON string  -- "[{\"label\":\"mobile\",\"number\":\"+65 91234567\"}]"
emails      JSON string  -- "[{\"label\":\"work\",\"address\":\"a@b.com\"}]"
addresses   JSON string  -- "[{\"label\":\"office\",\"street\":\"...\"}]"
tags        JSON string  -- "[\"fintech\",\"client\"]"
```

---

## UI Requirements

### Layout
- Top navbar: app name "CardVault", search bar, "Add Card" button
- Main area: responsive card grid (3 cols desktop, 2 tablet, 1 mobile)
- Tag filter chips row below navbar

### Business Card Component
```
┌─────────────────────────────┐
│  [Photo/Avatar]  Name        │
│                  Job Title   │
│                  Company     │
├─────────────────────────────┤
│  📧 work email               │
│  📱 mobile number            │
│  🌐 website                  │
├─────────────────────────────┤
│  [tag] [tag]          ✏️ 🗑️ │
└─────────────────────────────┘
```

- Avatar: show uploaded photo, fallback to initials in colored circle
- Color of initials circle derived from name (consistent per card)
- Show only first phone + first email on card, rest in detail modal

### Detail / Edit Modal
- Triggered by clicking a card or the edit button
- Tabbed or sectioned: **Basic Info | Contact | Address | Tags & Notes**
- Dynamic rows for phones, emails, addresses — "Add another" button per section
- Each row has a label dropdown + value input + remove button
- Photo upload: drag-and-drop area or click to upload, preview before save
- Tags: pill input (type tag, press Enter or comma to add)
- Save / Cancel / Delete buttons

### Search & Filter
- Search bar filters in real time (debounced 300ms) across name, company, email
- Tag chips toggle filter, multiple tags = AND filter
- Empty state: illustration + "No cards found. Add one!" prompt

### UX Details
- Delete shows confirmation dialog before calling API
- Form validates required fields client-side before submit
- Toast notifications for success/error (create, update, delete, upload)
- Loading skeleton while fetching cards
- Optimistic UI on delete (remove card from grid immediately)

### Style
- Clean, minimal design — white cards, subtle shadow
- Font: system font stack (no Google Fonts dependency for air-gap friendliness)
- Color accent: one primary color (e.g. indigo `#4f46e5`)
- Dark mode toggle (bonus, stored in localStorage)
- No CSS framework — vanilla CSS only so the binary stays small and the code is readable

---

## Non-Functional Requirements

### All Three Languages
- Single binary output
- SQLite via embedded driver (no external DB process)
- Static files and uploads served from binary or relative path
- CLI: `--port`, `--db`, `--uploads-dir`, `--seed`
- ENV fallback: `PORT`, `CARDVAULT_DB`, `CARDVAULT_UPLOADS`
- Graceful shutdown
- CORS allow all (demo mode)
- Max photo upload: 5MB, accept jpg/png/webp only

### Go
```
modernc.org/sqlite   -- pure Go, no CGO
//go:embed static/*  -- bakes HTML into binary
net/http stdlib      -- no framework
```

### Rust
```
axum                 -- HTTP framework
rusqlite + bundled   -- SQLite, statically linked
rust-embed           -- embed static files
tokio                -- async runtime
serde / serde_json   -- serialization
```

### Python
```
fastapi              -- HTTP framework
uvicorn              -- ASGI server
aiosqlite            -- async SQLite
python-multipart     -- file upload parsing
pyinstaller          -- package to single binary (show the contrast!)

Seed Data
10 cards with realistic Southeast Asia mix — varied companies, roles, multiple phones/emails per card, at least 3 with addresses, tags covering: client, partner, fintech, government, colleague, vendor, investor

Acceptance Criteria

All CRUD via REST, tested with curl examples in README
Photo upload persists across restarts
Cascade delete removes all related rows and photo file
Search works across name, company, email in real time
Tag filter works multi-select
Modal shows all phones/emails/addresses dynamically
Go binary: go build → single file, runs anywhere
Rust binary: cargo build --release → single file
Python: pyinstaller → dist folder (discuss the contrast vs Go/Rust)
README has curl examples for every endpoint