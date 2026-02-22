"""
CardVault — FastAPI entry point.

Usage:
    python main.py [--port 8080] [--db cardvault.db] [--uploads-dir uploads] [--seed]
"""
import argparse
import json
import logging
import os
import sys
import time
from pathlib import Path
from typing import Optional

import aiosqlite
import uvicorn
from contextlib import asynccontextmanager
from fastapi import FastAPI, HTTPException, Query, UploadFile, File, Form, Request
from fastapi.responses import FileResponse, HTMLResponse, JSONResponse, Response

import store as db_store
from models import Card, TagCount, HealthResponse, PhoneInput, EmailInput, AddressInput

# Configure logging
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s',
    datefmt='%Y-%m-%d %H:%M:%S'
)
logger = logging.getLogger(__name__)

# ── CLI args ───────────────────────────────────────────────────────────────────

def parse_args():
    parser = argparse.ArgumentParser(description="CardVault — business card manager")
    parser.add_argument("--port",        default=os.environ.get("PORT", "8080"),                 help="HTTP port")
    parser.add_argument("--db",          default=os.environ.get("CARDVAULT_DB", "cardvault.db"), help="SQLite path")
    parser.add_argument("--uploads-dir", default=os.environ.get("CARDVAULT_UPLOADS", "uploads"), dest="uploads_dir", help="Upload directory")
    parser.add_argument("--seed",        action="store_true",                                      help="Seed DB if empty")
    return parser.parse_args()

args = parse_args()
UPLOADS_DIR = Path(args.uploads_dir)
UPLOADS_DIR.mkdir(parents=True, exist_ok=True)

# ── Lifespan ───────────────────────────────────────────────────────────────────

_db: Optional[aiosqlite.Connection] = None

@asynccontextmanager
async def lifespan(app: FastAPI):
    global _db
    _db = await aiosqlite.connect(args.db)
    _db.row_factory = aiosqlite.Row
    await db_store.init_schema(_db)
    if args.seed and await db_store.is_empty(_db):
        print("Seeding database with sample data…")
        await db_store.seed_data(_db)
        print("Seed complete.")
    print(f"CardVault listening on http://localhost:{args.port}")
    yield
    await _db.close()

# ── FastAPI app ────────────────────────────────────────────────────────────────

app = FastAPI(title="CardVault", docs_url="/docs", redoc_url="/redoc", lifespan=lifespan)

# Request logging middleware
@app.middleware("http")
async def log_requests(request: Request, call_next):
    method = request.method
    path = request.url.path
    query_string = request.url.query if request.url.query else ""
    user_agent = request.headers.get("user-agent", "-")
    referer = request.headers.get("referer", "-")
    
    response = await call_next(request)
    
    logger.info(f"{method} {path} {query_string} {response.status_code} \"{user_agent}\" \"{referer}\"")
    
    return response

def get_db() -> aiosqlite.Connection:
    if _db is None:
        raise RuntimeError("DB not initialized")
    return _db


# ── Static / SPA ───────────────────────────────────────────────────────────────

# Determine static dir — works both when run directly and via PyInstaller
_here = Path(getattr(sys, "_MEIPASS", Path(__file__).parent))
_static_dir = _here / "static"

@app.get("/", response_class=HTMLResponse)
async def serve_index():
    html_path = _static_dir / "index.html"
    if not html_path.exists():
        raise HTTPException(404, "index.html not found")
    return HTMLResponse(html_path.read_text(encoding="utf-8"))


# Serve uploaded photos
@app.get("/uploads/{filename}")
async def serve_upload(filename: str):
    # Prevent directory traversal
    safe = Path(filename).name
    path = UPLOADS_DIR / safe
    if not path.exists():
        raise HTTPException(404, "not found")
    return FileResponse(str(path))


# ── /health ────────────────────────────────────────────────────────────────────

@app.get("/health", response_model=HealthResponse)
async def health():
    try:
        async with get_db().execute("SELECT 1") as cur:
            await cur.fetchone()
        db_status = "ok"
    except Exception:
        db_status = "error"
    return HealthResponse(status="ok", db=db_status)


# ── /api/tags ──────────────────────────────────────────────────────────────────

@app.get("/api/tags", response_model=list[TagCount])
async def get_tags():
    return await db_store.list_tags(get_db())


# ── /api/cards ─────────────────────────────────────────────────────────────────

@app.get("/api/cards", response_model=list[Card])
async def get_cards(
    q:   Optional[str] = Query(None, description="Search query"),
    tag: Optional[str] = Query(None, description="Tag filter"),
):
    return await db_store.list_cards(get_db(), q=q, tag=tag)


@app.post("/api/cards", response_model=Card, status_code=201)
async def post_card(
    name:      str        = Form(...),
    title:     str        = Form(""),
    company:   str        = Form(""),
    website:   str        = Form(""),
    notes:     str        = Form(""),
    phones:    str        = Form("[]"),
    emails:    str        = Form("[]"),
    addresses: str        = Form("[]"),
    tags:      str        = Form("[]"),
    photo:     Optional[UploadFile] = File(None),
):
    if not name.strip():
        raise HTTPException(400, "name is required")

    phones_list    = [PhoneInput(**p)   for p in json.loads(phones)]
    emails_list    = [EmailInput(**e)   for e in json.loads(emails)]
    addresses_list = [AddressInput(**a) for a in json.loads(addresses)]
    tags_list      = json.loads(tags)

    card_id = await db_store.create_card(
        get_db(), name=name.strip(), title=title, company=company,
        website=website, notes=notes,
        phones=phones_list, emails=emails_list,
        addresses=addresses_list, tags=tags_list,
    )

    if photo:
        photo_path = await _save_photo(card_id, photo)
        if photo_path:
            await db_store.update_card_photo(get_db(), card_id, photo_path)

    card = await db_store.get_card(get_db(), card_id)
    return card


@app.get("/api/cards/{card_id}", response_model=Card)
async def get_card(card_id: int):
    card = await db_store.get_card(get_db(), card_id)
    if not card:
        raise HTTPException(404, "card not found")
    return card


@app.put("/api/cards/{card_id}", response_model=Card)
async def put_card(
    card_id:   int,
    name:      str        = Form(...),
    title:     str        = Form(""),
    company:   str        = Form(""),
    website:   str        = Form(""),
    notes:     str        = Form(""),
    phones:    str        = Form("[]"),
    emails:    str        = Form("[]"),
    addresses: str        = Form("[]"),
    tags:      str        = Form("[]"),
    photo:     Optional[UploadFile] = File(None),
):
    existing = await db_store.get_card(get_db(), card_id)
    if not existing:
        raise HTTPException(404, "card not found")

    if not name.strip():
        raise HTTPException(400, "name is required")

    phones_list    = [PhoneInput(**p)   for p in json.loads(phones)]
    emails_list    = [EmailInput(**e)   for e in json.loads(emails)]
    addresses_list = [AddressInput(**a) for a in json.loads(addresses)]
    tags_list      = json.loads(tags)

    await db_store.update_card(
        get_db(), card_id=card_id, name=name.strip(), title=title, company=company,
        website=website, notes=notes,
        phones=phones_list, emails=emails_list,
        addresses=addresses_list, tags=tags_list,
    )

    if photo:
        photo_path = await _save_photo(card_id, photo)
        if photo_path:
            await db_store.update_card_photo(get_db(), card_id, photo_path)

    return await db_store.get_card(get_db(), card_id)


@app.delete("/api/cards/{card_id}", status_code=204)
async def delete_card(card_id: int):
    existing = await db_store.get_card(get_db(), card_id)
    if not existing:
        raise HTTPException(404, "card not found")
    photo_path = await db_store.delete_card(get_db(), card_id)
    if photo_path:
        _remove_file(photo_path)
    return Response(status_code=204)


# ── /api/cards/:id/photo ───────────────────────────────────────────────────────

@app.post("/api/cards/{card_id}/photo")
async def upload_photo(card_id: int, photo: UploadFile = File(...)):
    existing = await db_store.get_card(get_db(), card_id)
    if not existing:
        raise HTTPException(404, "card not found")

    photo_path = await _save_photo(card_id, photo)
    if not photo_path:
        raise HTTPException(400, "invalid photo")
    await db_store.update_card_photo(get_db(), card_id, photo_path)
    return {"photo_url": "/" + photo_path}


@app.delete("/api/cards/{card_id}/photo", status_code=204)
async def remove_photo(card_id: int):
    existing = await db_store.get_card(get_db(), card_id)
    if not existing:
        raise HTTPException(404, "card not found")
    old = await db_store.delete_card_photo(get_db(), card_id)
    if old:
        _remove_file(old)
    return Response(status_code=204)


# ── Photo helpers ──────────────────────────────────────────────────────────────

ALLOWED_EXTS = {".jpg", ".jpeg", ".png", ".webp"}
MAX_PHOTO_BYTES = 5 * 1024 * 1024  # 5 MB


async def _save_photo(card_id: int, upload: UploadFile) -> Optional[str]:
    ext = Path(upload.filename or "").suffix.lower()
    if ext not in ALLOWED_EXTS:
        raise HTTPException(400, "Only jpg/png/webp allowed")

    data = await upload.read()
    if len(data) > MAX_PHOTO_BYTES:
        raise HTTPException(400, "Photo must be under 5 MB")

    fname = f"card_{card_id}_{int(time.time() * 1000)}{ext}"
    dest = UPLOADS_DIR / fname
    dest.write_bytes(data)
    return f"uploads/{fname}"


def _remove_file(path: str) -> None:
    try:
        os.remove(path)
    except OSError:
        pass


# ── Main ───────────────────────────────────────────────────────────────────────

if __name__ == "__main__":
    uvicorn.run(
        "main:app",
        host="0.0.0.0",
        port=int(args.port),
        log_level="info",
    )
