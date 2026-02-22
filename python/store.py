"""
Async SQLite store for CardVault.
"""
import json
import logging
from typing import Optional

import aiosqlite

from models import Card, Phone, Email, Address, TagCount, PhoneInput, EmailInput, AddressInput

logger = logging.getLogger(__name__)

SCHEMA = """
PRAGMA journal_mode=WAL;
PRAGMA foreign_keys=ON;

CREATE TABLE IF NOT EXISTS cards (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    name       TEXT NOT NULL,
    title      TEXT DEFAULT '',
    company    TEXT DEFAULT '',
    website    TEXT DEFAULT '',
    notes      TEXT DEFAULT '',
    photo_path TEXT DEFAULT '',
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS card_phones (
    id      INTEGER PRIMARY KEY AUTOINCREMENT,
    card_id INTEGER REFERENCES cards(id) ON DELETE CASCADE,
    label   TEXT DEFAULT 'mobile',
    number  TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS card_emails (
    id      INTEGER PRIMARY KEY AUTOINCREMENT,
    card_id INTEGER REFERENCES cards(id) ON DELETE CASCADE,
    label   TEXT DEFAULT 'work',
    address TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS card_addresses (
    id      INTEGER PRIMARY KEY AUTOINCREMENT,
    card_id INTEGER REFERENCES cards(id) ON DELETE CASCADE,
    label   TEXT DEFAULT 'office',
    street  TEXT DEFAULT '',
    city    TEXT DEFAULT '',
    country TEXT DEFAULT '',
    postal  TEXT DEFAULT ''
);

CREATE TABLE IF NOT EXISTS tags (
    id   INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT UNIQUE NOT NULL
);

CREATE TABLE IF NOT EXISTS card_tags (
    card_id INTEGER REFERENCES cards(id) ON DELETE CASCADE,
    tag_id  INTEGER REFERENCES tags(id) ON DELETE CASCADE,
    PRIMARY KEY (card_id, tag_id)
);
"""

# ── Schema ─────────────────────────────────────────────────────────────────────

async def init_schema(db: aiosqlite.Connection) -> None:
    await db.executescript(SCHEMA)
    await db.commit()


async def is_empty(db: aiosqlite.Connection) -> bool:
    async with db.execute("SELECT COUNT(*) FROM cards") as cur:
        row = await cur.fetchone()
    return row[0] == 0


# ── Helpers ────────────────────────────────────────────────────────────────────

async def _load_related(db: aiosqlite.Connection, card_id: int) -> tuple[list, list, list, list]:
    phones = []
    async with db.execute(
        "SELECT id, label, number FROM card_phones WHERE card_id=? ORDER BY id", (card_id,)
    ) as cur:
        async for row in cur:
            phones.append(Phone(id=row[0], label=row[1], number=row[2]))

    emails = []
    async with db.execute(
        "SELECT id, label, address FROM card_emails WHERE card_id=? ORDER BY id", (card_id,)
    ) as cur:
        async for row in cur:
            emails.append(Email(id=row[0], label=row[1], address=row[2]))

    addresses = []
    async with db.execute(
        "SELECT id, label, street, city, country, postal FROM card_addresses WHERE card_id=? ORDER BY id",
        (card_id,),
    ) as cur:
        async for row in cur:
            addresses.append(Address(id=row[0], label=row[1], street=row[2], city=row[3], country=row[4], postal=row[5]))

    tags = []
    async with db.execute(
        """SELECT t.name FROM tags t
           JOIN card_tags ct ON ct.tag_id=t.id
           WHERE ct.card_id=? ORDER BY t.name""",
        (card_id,),
    ) as cur:
        async for row in cur:
            tags.append(row[0])

    return phones, emails, addresses, tags


def _row_to_card(row) -> Card:
    photo_url = ("/" + row[6]) if row[6] else ""
    return Card(
        id=row[0],
        name=row[1],
        title=row[2],
        company=row[3],
        website=row[4],
        notes=row[5],
        photo_url=photo_url,
        created_at=row[7],
        updated_at=row[8],
    )


# ── CRUD ───────────────────────────────────────────────────────────────────────

async def list_cards(db: aiosqlite.Connection, q: Optional[str] = None, tag: Optional[str] = None) -> list[Card]:
    sql = """
SELECT DISTINCT c.id, c.name, c.title, c.company, c.website, c.notes, c.photo_path, c.created_at, c.updated_at
FROM cards c"""
    args: list = []

    if tag:
        sql += """
JOIN card_tags ct ON ct.card_id = c.id
JOIN tags t ON t.id = ct.tag_id AND t.name = ?"""
        args.append(tag)

    if q:
        like = f"%{q}%"
        sql += """
WHERE (c.name LIKE ? OR c.company LIKE ? OR EXISTS (
    SELECT 1 FROM card_emails e WHERE e.card_id=c.id AND e.address LIKE ?
))"""
        args += [like, like, like]

    sql += " ORDER BY c.updated_at DESC"

    cards = []
    async with db.execute(sql, args) as cur:
        async for row in cur:
            card = _row_to_card(row)
            phones, emails, addresses, tags = await _load_related(db, card.id)
            card.phones = phones
            card.emails = emails
            card.addresses = addresses
            card.tags = tags
            cards.append(card)
    return cards


async def get_card(db: aiosqlite.Connection, card_id: int) -> Optional[Card]:
    async with db.execute(
        "SELECT id, name, title, company, website, notes, photo_path, created_at, updated_at FROM cards WHERE id=?",
        (card_id,),
    ) as cur:
        row = await cur.fetchone()
    if not row:
        return None
    card = _row_to_card(row)
    card.phones, card.emails, card.addresses, card.tags = await _load_related(db, card.id)
    return card


async def _insert_related(
    db: aiosqlite.Connection,
    card_id: int,
    phones: list[PhoneInput],
    emails: list[EmailInput],
    addresses: list[AddressInput],
    tags: list[str],
) -> None:
    for p in phones:
        await db.execute(
            "INSERT INTO card_phones (card_id, label, number) VALUES (?,?,?)",
            (card_id, p.label, p.number),
        )
    for e in emails:
        await db.execute(
            "INSERT INTO card_emails (card_id, label, address) VALUES (?,?,?)",
            (card_id, e.label, e.address),
        )
    for a in addresses:
        await db.execute(
            "INSERT INTO card_addresses (card_id, label, street, city, country, postal) VALUES (?,?,?,?,?,?)",
            (card_id, a.label, a.street, a.city, a.country, a.postal),
        )
    for tag_name in tags:
        tag_name = tag_name.strip().lower()
        if not tag_name:
            continue
        await db.execute("INSERT OR IGNORE INTO tags (name) VALUES (?)", (tag_name,))
        async with db.execute("SELECT id FROM tags WHERE name=?", (tag_name,)) as cur:
            tag_row = await cur.fetchone()
        if tag_row:
            await db.execute(
                "INSERT OR IGNORE INTO card_tags (card_id, tag_id) VALUES (?,?)",
                (card_id, tag_row[0]),
            )


async def create_card(
    db: aiosqlite.Connection,
    name: str,
    title: str,
    company: str,
    website: str,
    notes: str,
    phones: list[PhoneInput],
    emails: list[EmailInput],
    addresses: list[AddressInput],
    tags: list[str],
) -> int:
    async with db.execute(
        "INSERT INTO cards (name, title, company, website, notes) VALUES (?,?,?,?,?)",
        (name, title, company, website, notes),
    ) as cur:
        card_id = cur.lastrowid
    await _insert_related(db, card_id, phones, emails, addresses, tags)
    await db.commit()
    return card_id


async def update_card(
    db: aiosqlite.Connection,
    card_id: int,
    name: str,
    title: str,
    company: str,
    website: str,
    notes: str,
    phones: list[PhoneInput],
    emails: list[EmailInput],
    addresses: list[AddressInput],
    tags: list[str],
) -> None:
    await db.execute(
        """UPDATE cards SET name=?, title=?, company=?, website=?, notes=?,
           updated_at=CURRENT_TIMESTAMP WHERE id=?""",
        (name, title, company, website, notes, card_id),
    )
    for tbl in ("card_phones", "card_emails", "card_addresses", "card_tags"):
        await db.execute(f"DELETE FROM {tbl} WHERE card_id=?", (card_id,))
    await _insert_related(db, card_id, phones, emails, addresses, tags)
    await db.commit()


async def delete_card(db: aiosqlite.Connection, card_id: int) -> str:
    logger.info(f"SQL: SELECT photo_path FROM cards WHERE id = {card_id}")
    async with db.execute("SELECT photo_path FROM cards WHERE id=?", (card_id,)) as cur:
        row = await cur.fetchone()
    photo_path = row[0] if row else ""
    logger.info(f"SQL: Result - photo_path: {photo_path}")
    
    logger.info(f"SQL: DELETE FROM cards WHERE id = {card_id}")
    await db.execute("DELETE FROM cards WHERE id=?", (card_id,))
    await db.commit()
    logger.info(f"SQL: Result - rows_affected: 1")
    
    return photo_path or ""


async def update_card_photo(db: aiosqlite.Connection, card_id: int, path: str) -> None:
    await db.execute(
        "UPDATE cards SET photo_path=?, updated_at=CURRENT_TIMESTAMP WHERE id=?",
        (path, card_id),
    )
    await db.commit()


async def delete_card_photo(db: aiosqlite.Connection, card_id: int) -> str:
    async with db.execute("SELECT photo_path FROM cards WHERE id=?", (card_id,)) as cur:
        row = await cur.fetchone()
    old = row[0] if row else ""
    await db.execute(
        "UPDATE cards SET photo_path='', updated_at=CURRENT_TIMESTAMP WHERE id=?",
        (card_id,),
    )
    await db.commit()
    return old or ""


async def list_tags(db: aiosqlite.Connection) -> list[TagCount]:
    sql = """
SELECT t.name, COUNT(ct.card_id) as cnt
FROM tags t
LEFT JOIN card_tags ct ON ct.tag_id=t.id
GROUP BY t.id, t.name
ORDER BY t.name"""
    tags = []
    async with db.execute(sql) as cur:
        async for row in cur:
            tags.append(TagCount(name=row[0], count=row[1]))
    return tags


# ── Seed data ──────────────────────────────────────────────────────────────────

SEED_CARDS = [
    {
        "name": "Tan Wei Ming",
        "title": "Chief Executive Officer",
        "company": "DBS Group Holdings",
        "website": "https://www.dbs.com",
        "notes": "Met at Singapore Fintech Festival 2025",
        "phones": [{"label": "mobile", "number": "+65 9123 4567"}, {"label": "work", "number": "+65 6878 8888"}],
        "emails": [{"label": "work", "address": "weiming.tan@dbs.com"}, {"label": "personal", "address": "wm.tan@gmail.com"}],
        "addresses": [{"label": "office", "street": "12 Marina Boulevard, DBS Asia Hub 2", "city": "Singapore", "country": "Singapore", "postal": "018982"}],
        "tags": ["fintech", "client", "investor"],
    },
    {
        "name": "Priya Krishnamurthy",
        "title": "VP Engineering",
        "company": "Grab Holdings",
        "website": "https://www.grab.com",
        "notes": "Collaborated on payments infrastructure",
        "phones": [{"label": "mobile", "number": "+65 8234 5678"}],
        "emails": [{"label": "work", "address": "priya.k@grab.com"}],
        "addresses": [{"label": "office", "street": "3 Media Close, One-North", "city": "Singapore", "country": "Singapore", "postal": "138498"}],
        "tags": ["fintech", "partner", "colleague"],
    },
    {
        "name": "Ahmad Fauzi bin Rashid",
        "title": "Director of Investments",
        "company": "Temasek Holdings",
        "website": "https://www.temasek.com.sg",
        "notes": "Investor relations contact",
        "phones": [{"label": "mobile", "number": "+65 9345 6789"}, {"label": "work", "number": "+65 6308 2222"}],
        "emails": [{"label": "work", "address": "ahmad.fauzi@temasek.com.sg"}],
        "addresses": [{"label": "office", "street": "60B Orchard Road, Tower 2", "city": "Singapore", "country": "Singapore", "postal": "238891"}],
        "tags": ["government", "investor"],
    },
    {
        "name": "Li Mei Chen",
        "title": "Chief Technology Officer",
        "company": "Sea Limited",
        "website": "https://www.sea.com",
        "notes": "Introduced by James Wong",
        "phones": [{"label": "mobile", "number": "+65 8456 7890"}],
        "emails": [{"label": "work", "address": "meichen@sea.com"}, {"label": "personal", "address": "limeichen@hotmail.com"}],
        "addresses": [],
        "tags": ["fintech", "colleague"],
    },
    {
        "name": "Rajesh Kumar s/o Subramaniam",
        "title": "Principal Consultant",
        "company": "McKinsey & Company",
        "website": "https://www.mckinsey.com",
        "notes": "Strategy consulting, KL office lead",
        "phones": [{"label": "mobile", "number": "+60 12-345 6789"}, {"label": "work", "number": "+60 3-2302 1000"}],
        "emails": [{"label": "work", "address": "rajesh.kumar@mckinsey.com"}],
        "addresses": [{"label": "office", "street": "Level 34, Menara Citibank, 165 Jalan Ampang", "city": "Kuala Lumpur", "country": "Malaysia", "postal": "50450"}],
        "tags": ["partner", "vendor"],
    },
    {
        "name": "Siti Nurbaya Haji Mohamad",
        "title": "Senior Director",
        "company": "GovTech Singapore",
        "website": "https://www.tech.gov.sg",
        "notes": "Digital government partnerships",
        "phones": [{"label": "mobile", "number": "+65 9567 8901"}],
        "emails": [{"label": "work", "address": "siti_nurbaya@tech.gov.sg"}],
        "addresses": [],
        "tags": ["government", "client"],
    },
    {
        "name": "Kevin Tan Kiat Seng",
        "title": "Founder & CEO",
        "company": "PaySG Technologies",
        "website": "https://www.paysg.io",
        "notes": "Seed stage, looking for Series A",
        "phones": [{"label": "mobile", "number": "+65 9678 9012"}],
        "emails": [{"label": "work", "address": "kevin@paysg.io"}, {"label": "personal", "address": "kevintks@gmail.com"}],
        "addresses": [{"label": "office", "street": "71 Ayer Rajah Crescent, JTC LaunchPad", "city": "Singapore", "country": "Singapore", "postal": "139952"}],
        "tags": ["fintech", "investor", "client"],
    },
    {
        "name": "Siti Rahimah Binti Abdullah",
        "title": "Regional Director",
        "company": "Prudential plc",
        "website": "https://www.prudential.co.id",
        "notes": "Insurance & wealth management, Indonesia",
        "phones": [{"label": "mobile", "number": "+62 812-3456-7890"}, {"label": "work", "number": "+62 21-5799-8400"}],
        "emails": [{"label": "work", "address": "siti.rahimah@prudential.co.id"}],
        "addresses": [{"label": "office", "street": "Prudential Tower, 7 Jalan Jenderal Sudirman", "city": "Jakarta", "country": "Indonesia", "postal": "10220"}],
        "tags": ["fintech", "partner"],
    },
    {
        "name": "James Wong Wei Jian",
        "title": "Head of Engineering",
        "company": "Shopee / Sea Group",
        "website": "https://shopee.sg",
        "notes": "Ex-PayPal, strong mobile payments background",
        "phones": [{"label": "mobile", "number": "+65 9789 0123"}],
        "emails": [{"label": "work", "address": "jameswong@shopee.com"}, {"label": "personal", "address": "james.wongwj@gmail.com"}],
        "addresses": [],
        "tags": ["colleague"],
    },
    {
        "name": "Anika Sharma",
        "title": "Senior Product Manager",
        "company": "Agoda Company",
        "website": "https://www.agoda.com",
        "notes": "Travel tech; met at ProductCon Bangkok",
        "phones": [{"label": "mobile", "number": "+66 89-123-4567"}],
        "emails": [{"label": "work", "address": "anika.sharma@agoda.com"}],
        "addresses": [{"label": "office", "street": "30th Floor, The Offices at CentralWorld, Ratchadamri Road", "city": "Bangkok", "country": "Thailand", "postal": "10330"}],
        "tags": ["colleague", "vendor"],
    },
]


async def seed_data(db: aiosqlite.Connection) -> None:
    for s in SEED_CARDS:
        phones = [PhoneInput(**p) for p in s["phones"]]
        emails = [EmailInput(**e) for e in s["emails"]]
        addresses = [AddressInput(**a) for a in s["addresses"]]
        await create_card(
            db,
            name=s["name"],
            title=s["title"],
            company=s["company"],
            website=s["website"],
            notes=s["notes"],
            phones=phones,
            emails=emails,
            addresses=addresses,
            tags=s["tags"],
        )
