# CardVault — Python

Python implementation of CardVault using FastAPI for HTTP and aiosqlite for async SQLite access.

## Stack

| Component | Package |
|---|---|
| HTTP framework | `fastapi` |
| ASGI server | `uvicorn` |
| SQLite | `aiosqlite` (async) |
| File upload | `python-multipart` |
| Packaging | `pyinstaller` (optional single-file dist) |

## Files

```
python/
├── main.py            # Entry point, CLI flags, FastAPI app, startup/shutdown
├── store.py           # DB schema, connection, CRUD coroutines
├── models.py          # Pydantic models for request/response validation
├── static/
│   └── index.html     # SPA served as a static file
├── uploads/           # Runtime photo storage (gitignored)
└── requirements.txt
```

## Requirements

- Python 3.11+
- [uv](https://docs.astral.sh/uv/) (recommended) or pip

## Setup with uv (recommended)

```bash
cd python

# Install dependencies and create .venv automatically
uv sync

# Run directly through uv (no manual activation needed)
uv run python main.py --seed
```

## Setup with pip (alternative)

```bash
cd python
python -m venv .venv
source .venv/bin/activate      # macOS/Linux
pip install -r requirements.txt
```

## Run

```bash
# Via uv (recommended)
uv run python main.py

# Or activate .venv first, then run directly
source .venv/bin/activate
python main.py

# Custom settings
python main.py --port 9090 --db /data/cards.db --uploads-dir /data/uploads

# Load seed data
python main.py --seed

# Via environment variables
PORT=9090 CARDVAULT_DB=/data/cards.db python main.py
```

Then open [http://localhost:8080](http://localhost:8080) in your browser.

## CLI Flags

| Flag | ENV | Default | Description |
|---|---|---|---|
| `--port` | `PORT` | `8080` | HTTP listen port |
| `--db` | `CARDVAULT_DB` | `cardvault.db` | SQLite database file |
| `--uploads-dir` | `CARDVAULT_UPLOADS` | `uploads/` | Directory for uploaded photos |
| `--seed` | — | false | Insert seed data if DB is empty |

## Package to a Single Binary with PyInstaller

```bash
# PyInstaller is included as a dev dependency
uv run pyinstaller --onefile --add-data "static:static" main.py
```

The executable is placed at `dist/main` (or `dist/main.exe` on Windows).

> **Note:** The PyInstaller output is a self-extracting archive (~30–60 MB), not a true native binary like Go or Rust produce. It unpacks to a temp directory on first run. This is the key contrast with the Go and Rust implementations.

## Development

Run with auto-reload:
```bash
uv run uvicorn main:app --reload --port 8080
```

## Design Notes

- `FastAPI` provides automatic OpenAPI docs at `/docs` and `/redoc` — useful during development
- `aiosqlite` wraps SQLite in an async context manager compatible with FastAPI's async handlers
- `python-multipart` is required for `UploadFile` in FastAPI; it is listed explicitly in `requirements.txt`
- `uvicorn` is the ASGI server; `main.py` calls `uvicorn.run(app, ...)` directly so the file is its own entry point
- `uploads/` is created at startup if it does not exist; it is `.gitignore`d
- Pydantic models in `models.py` enforce field types and produce consistent JSON responses across all endpoints

## Contrast with Go and Rust

| | Go | Rust | Python |
|---|---|---|---|
| Binary size | ~10 MB | ~5 MB | ~50 MB (PyInstaller archive) |
| Startup time | <10 ms | <10 ms | ~1–3 s (extraction) |
| Build step | `go build` | `cargo build --release` | `pyinstaller --onefile` |
| CGO needed | No | Yes (rusqlite bundled) | No |
| Interpreter bundled | No | No | Yes |
| Dev iteration | Fast | Slower (compile) | Fastest (no compile) |
