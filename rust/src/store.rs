use std::sync::{Arc, Mutex};

use anyhow::Result;
use rusqlite::{Connection, OptionalExtension, params};
use tracing::info;

use crate::models::{Address, Card, CardInput, Email, Phone, TagCount};

pub fn init_db(conn: &Arc<Mutex<Connection>>) -> Result<()> {
    let conn = conn.lock().unwrap();
    conn.execute_batch("PRAGMA journal_mode=WAL;")?;
    conn.execute_batch("PRAGMA foreign_keys=ON;")?;
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS cards (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            name        TEXT NOT NULL,
            title       TEXT NOT NULL DEFAULT '',
            company     TEXT NOT NULL DEFAULT '',
            website     TEXT NOT NULL DEFAULT '',
            notes       TEXT NOT NULL DEFAULT '',
            photo_path  TEXT NOT NULL DEFAULT '',
            created_at  DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at  DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
        );

        CREATE TABLE IF NOT EXISTS card_phones (
            id      INTEGER PRIMARY KEY AUTOINCREMENT,
            card_id INTEGER NOT NULL REFERENCES cards(id) ON DELETE CASCADE,
            label   TEXT NOT NULL DEFAULT '',
            number  TEXT NOT NULL DEFAULT ''
        );

        CREATE TABLE IF NOT EXISTS card_emails (
            id      INTEGER PRIMARY KEY AUTOINCREMENT,
            card_id INTEGER NOT NULL REFERENCES cards(id) ON DELETE CASCADE,
            label   TEXT NOT NULL DEFAULT '',
            address TEXT NOT NULL DEFAULT ''
        );

        CREATE TABLE IF NOT EXISTS card_addresses (
            id      INTEGER PRIMARY KEY AUTOINCREMENT,
            card_id INTEGER NOT NULL REFERENCES cards(id) ON DELETE CASCADE,
            label   TEXT NOT NULL DEFAULT '',
            street  TEXT NOT NULL DEFAULT '',
            city    TEXT NOT NULL DEFAULT '',
            country TEXT NOT NULL DEFAULT '',
            postal  TEXT NOT NULL DEFAULT ''
        );

        CREATE TABLE IF NOT EXISTS tags (
            id   INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL UNIQUE
        );

        CREATE TABLE IF NOT EXISTS card_tags (
            card_id INTEGER NOT NULL REFERENCES cards(id) ON DELETE CASCADE,
            tag_id  INTEGER NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
            PRIMARY KEY (card_id, tag_id)
        );
        "#,
    )?;
    Ok(())
}

pub fn is_empty(conn: &Arc<Mutex<Connection>>) -> bool {
    let conn = conn.lock().unwrap();
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM cards", [], |row| row.get(0))
        .unwrap_or(0);
    count == 0
}

fn fetch_card_by_id(conn: &Connection, id: i64) -> Result<Option<Card>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, title, company, website, notes, photo_path, created_at, updated_at
         FROM cards WHERE id = ?1",
    )?;

    let card_opt = stmt
        .query_row(params![id], |row| {
            Ok(Card {
                id: row.get(0)?,
                name: row.get(1)?,
                title: row.get(2)?,
                company: row.get(3)?,
                website: row.get(4)?,
                notes: row.get(5)?,
                photo_url: {
                    let path: String = row.get(6)?;
                    if path.is_empty() {
                        String::new()
                    } else {
                        format!("/{path}")
                    }
                },
                phones: vec![],
                emails: vec![],
                addresses: vec![],
                tags: vec![],
                created_at: row.get(7)?,
                updated_at: row.get(8)?,
            })
        })
        .ok();

    let Some(mut card) = card_opt else {
        return Ok(None);
    };

    // phones
    let mut stmt = conn.prepare(
        "SELECT id, label, number FROM card_phones WHERE card_id = ?1 ORDER BY id",
    )?;
    card.phones = stmt
        .query_map(params![id], |row| {
            Ok(Phone {
                id: row.get(0)?,
                label: row.get(1)?,
                number: row.get(2)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    // emails
    let mut stmt = conn.prepare(
        "SELECT id, label, address FROM card_emails WHERE card_id = ?1 ORDER BY id",
    )?;
    card.emails = stmt
        .query_map(params![id], |row| {
            Ok(Email {
                id: row.get(0)?,
                label: row.get(1)?,
                address: row.get(2)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    // addresses
    let mut stmt = conn.prepare(
        "SELECT id, label, street, city, country, postal FROM card_addresses WHERE card_id = ?1 ORDER BY id",
    )?;
    card.addresses = stmt
        .query_map(params![id], |row| {
            Ok(Address {
                id: row.get(0)?,
                label: row.get(1)?,
                street: row.get(2)?,
                city: row.get(3)?,
                country: row.get(4)?,
                postal: row.get(5)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    // tags
    let mut stmt = conn.prepare(
        "SELECT t.name FROM tags t
         JOIN card_tags ct ON ct.tag_id = t.id
         WHERE ct.card_id = ?1
         ORDER BY t.name",
    )?;
    card.tags = stmt
        .query_map(params![id], |row| row.get(0))?
        .collect::<std::result::Result<Vec<String>, _>>()?;

    Ok(Some(card))
}

pub fn list_cards(
    conn: &Arc<Mutex<Connection>>,
    q: Option<&str>,
    tag: Option<&str>,
) -> Result<Vec<Card>> {
    let conn = conn.lock().unwrap();

    let ids: Vec<i64> = match (q, tag) {
        (Some(search), Some(tag_filter)) => {
            let pattern = format!("%{search}%");
            let mut stmt = conn.prepare(
                r#"SELECT DISTINCT c.id FROM cards c
                   LEFT JOIN card_emails ce ON ce.card_id = c.id
                   JOIN card_tags ct ON ct.card_id = c.id
                   JOIN tags t ON t.id = ct.tag_id
                   WHERE t.name = ?1
                     AND (c.name LIKE ?2 OR c.company LIKE ?2 OR ce.address LIKE ?2)
                   ORDER BY c.updated_at DESC"#,
            )?;
            let ids = stmt
                .query_map(params![tag_filter, pattern], |row| row.get(0))?
                .collect::<std::result::Result<Vec<i64>, _>>()?;
            ids
        }
        (Some(search), None) => {
            let pattern = format!("%{search}%");
            let mut stmt = conn.prepare(
                r#"SELECT DISTINCT c.id FROM cards c
                   LEFT JOIN card_emails ce ON ce.card_id = c.id
                   WHERE c.name LIKE ?1 OR c.company LIKE ?1 OR ce.address LIKE ?1
                   ORDER BY c.updated_at DESC"#,
            )?;
            let ids = stmt
                .query_map(params![pattern], |row| row.get(0))?
                .collect::<std::result::Result<Vec<i64>, _>>()?;
            ids
        }
        (None, Some(tag_filter)) => {
            let mut stmt = conn.prepare(
                r#"SELECT DISTINCT c.id FROM cards c
                   JOIN card_tags ct ON ct.card_id = c.id
                   JOIN tags t ON t.id = ct.tag_id
                   WHERE t.name = ?1
                   ORDER BY c.updated_at DESC"#,
            )?;
            let ids = stmt
                .query_map(params![tag_filter], |row| row.get(0))?
                .collect::<std::result::Result<Vec<i64>, _>>()?;
            ids
        }
        (None, None) => {
            let mut stmt =
                conn.prepare("SELECT id FROM cards ORDER BY updated_at DESC")?;
            let ids = stmt
                .query_map([], |row| row.get(0))?
                .collect::<std::result::Result<Vec<i64>, _>>()?;
            ids
        }
    };

    let mut cards = Vec::with_capacity(ids.len());
    for id in ids {
        if let Some(card) = fetch_card_by_id(&conn, id)? {
            cards.push(card);
        }
    }
    Ok(cards)
}

pub fn get_card(conn: &Arc<Mutex<Connection>>, id: i64) -> Result<Option<Card>> {
    let conn = conn.lock().unwrap();
    fetch_card_by_id(&conn, id)
}

fn upsert_tags_and_link(
    conn: &Connection,
    card_id: i64,
    tags: &[String],
) -> Result<()> {
    conn.execute("DELETE FROM card_tags WHERE card_id = ?1", params![card_id])?;
    for tag in tags {
        if tag.trim().is_empty() {
            continue;
        }
        conn.execute(
            "INSERT OR IGNORE INTO tags (name) VALUES (?1)",
            params![tag],
        )?;
        let tag_id: i64 =
            conn.query_row("SELECT id FROM tags WHERE name = ?1", params![tag], |r| {
                r.get(0)
            })?;
        conn.execute(
            "INSERT OR IGNORE INTO card_tags (card_id, tag_id) VALUES (?1, ?2)",
            params![card_id, tag_id],
        )?;
    }
    Ok(())
}

pub fn create_card(conn: &Arc<Mutex<Connection>>, input: &CardInput) -> Result<i64> {
    let conn = conn.lock().unwrap();
    conn.execute(
        "INSERT INTO cards (name, title, company, website, notes) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![input.name, input.title, input.company, input.website, input.notes],
    )?;
    let id = conn.last_insert_rowid();

    for p in &input.phones {
        conn.execute(
            "INSERT INTO card_phones (card_id, label, number) VALUES (?1, ?2, ?3)",
            params![id, p.label, p.number],
        )?;
    }
    for e in &input.emails {
        conn.execute(
            "INSERT INTO card_emails (card_id, label, address) VALUES (?1, ?2, ?3)",
            params![id, e.label, e.address],
        )?;
    }
    for a in &input.addresses {
        conn.execute(
            "INSERT INTO card_addresses (card_id, label, street, city, country, postal) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![id, a.label, a.street, a.city, a.country, a.postal],
        )?;
    }
    upsert_tags_and_link(&conn, id, &input.tags)?;
    Ok(id)
}

pub fn update_card(
    conn: &Arc<Mutex<Connection>>,
    id: i64,
    input: &CardInput,
) -> Result<()> {
    let conn = conn.lock().unwrap();
    let updated = conn.execute(
        "UPDATE cards SET name=?1, title=?2, company=?3, website=?4, notes=?5, updated_at=CURRENT_TIMESTAMP WHERE id=?6",
        params![input.name, input.title, input.company, input.website, input.notes, id],
    )?;
    if updated == 0 {
        anyhow::bail!("card not found");
    }

    conn.execute("DELETE FROM card_phones WHERE card_id = ?1", params![id])?;
    for p in &input.phones {
        conn.execute(
            "INSERT INTO card_phones (card_id, label, number) VALUES (?1, ?2, ?3)",
            params![id, p.label, p.number],
        )?;
    }

    conn.execute("DELETE FROM card_emails WHERE card_id = ?1", params![id])?;
    for e in &input.emails {
        conn.execute(
            "INSERT INTO card_emails (card_id, label, address) VALUES (?1, ?2, ?3)",
            params![id, e.label, e.address],
        )?;
    }

    conn.execute("DELETE FROM card_addresses WHERE card_id = ?1", params![id])?;
    for a in &input.addresses {
        conn.execute(
            "INSERT INTO card_addresses (card_id, label, street, city, country, postal) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![id, a.label, a.street, a.city, a.country, a.postal],
        )?;
    }

    upsert_tags_and_link(&conn, id, &input.tags)?;
    Ok(())
}

pub fn delete_card(conn: &Arc<Mutex<Connection>>, id: i64) -> Result<Option<String>> {
    info!("SQL: SELECT photo_path FROM cards WHERE id = {}", id);
    let conn = conn.lock().unwrap();

    // Get photo_path before deleting
    let photo_path: Option<String> = conn
        .query_row(
            "SELECT photo_path FROM cards WHERE id = ?1",
            params![id],
            |r| r.get(0),
        )
        .optional()?;
    info!("SQL: Result - photo_path: {:?}", photo_path);

    // Delete the card and check if it existed
    info!("SQL: DELETE FROM cards WHERE id = {}", id);
    let rows_affected = conn.execute("DELETE FROM cards WHERE id = ?1", params![id])?;
    info!("SQL: Result - rows_affected: {}", rows_affected);

    if rows_affected == 0 {
        return Ok(None); // Card didn't exist
    }

    Ok(photo_path)
}

pub fn update_card_photo(
    conn: &Arc<Mutex<Connection>>,
    id: i64,
    path: &str,
) -> Result<()> {
    let conn = conn.lock().unwrap();
    let updated = conn.execute(
        "UPDATE cards SET photo_path=?1, updated_at=CURRENT_TIMESTAMP WHERE id=?2",
        params![path, id],
    )?;
    if updated == 0 {
        anyhow::bail!("card not found");
    }
    Ok(())
}

pub fn delete_card_photo(conn: &Arc<Mutex<Connection>>, id: i64) -> Result<Option<String>> {
    let conn = conn.lock().unwrap();
    let old_path: Option<String> = conn
        .query_row(
            "SELECT photo_path FROM cards WHERE id = ?1",
            params![id],
            |r| r.get(0),
        )
        .optional()?;
    if old_path.is_none() {
        return Ok(None);
    }
    conn.execute(
        "UPDATE cards SET photo_path='', updated_at=CURRENT_TIMESTAMP WHERE id=?1",
        params![id],
    )?;
    Ok(old_path)
}

pub fn list_tags(conn: &Arc<Mutex<Connection>>) -> Result<Vec<TagCount>> {
    let conn = conn.lock().unwrap();
    let mut stmt = conn.prepare(
        "SELECT t.name, COUNT(ct.card_id) as cnt
         FROM tags t
         LEFT JOIN card_tags ct ON ct.tag_id = t.id
         GROUP BY t.id, t.name
         ORDER BY t.name",
    )?;
    let tags: Vec<TagCount> = stmt
        .query_map([], |row| {
            Ok(TagCount {
                name: row.get(0)?,
                count: row.get(1)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(tags)
}

pub fn seed_data(conn: &Arc<Mutex<Connection>>) -> Result<()> {
    struct SeedCard {
        name: &'static str,
        title: &'static str,
        company: &'static str,
        website: &'static str,
        notes: &'static str,
        phones: Vec<(&'static str, &'static str)>,
        emails: Vec<(&'static str, &'static str)>,
        addresses: Vec<(&'static str, &'static str, &'static str, &'static str, &'static str)>,
        tags: Vec<&'static str>,
    }

    let seeds = vec![
        SeedCard {
            name: "Tan Wei Ming",
            title: "CEO",
            company: "DBS Group Holdings",
            website: "https://www.dbs.com",
            notes: "Met at Singapore Fintech Festival 2025",
            phones: vec![
                ("mobile", "+65 9123 4567"),
                ("work", "+65 6878 8888"),
            ],
            emails: vec![
                ("work", "weiming.tan@dbs.com"),
                ("personal", "wm.tan@gmail.com"),
            ],
            addresses: vec![(
                "office",
                "12 Marina Boulevard DBS Asia Hub 2",
                "Singapore",
                "Singapore",
                "018982",
            )],
            tags: vec!["fintech", "client", "investor"],
        },
        SeedCard {
            name: "Priya Krishnamurthy",
            title: "VP Engineering",
            company: "Grab Holdings",
            website: "https://www.grab.com",
            notes: "",
            phones: vec![("mobile", "+65 8234 5678")],
            emails: vec![("work", "priya.k@grab.com")],
            addresses: vec![(
                "office",
                "3 Media Close One-North",
                "Singapore",
                "Singapore",
                "138498",
            )],
            tags: vec!["fintech", "partner", "colleague"],
        },
        SeedCard {
            name: "Ahmad Fauzi bin Rashid",
            title: "Director of Investments",
            company: "Temasek Holdings",
            website: "https://www.temasek.com.sg",
            notes: "",
            phones: vec![
                ("mobile", "+65 9345 6789"),
                ("work", "+65 6308 2222"),
            ],
            emails: vec![("work", "ahmad.fauzi@temasek.com.sg")],
            addresses: vec![(
                "office",
                "60B Orchard Road Tower 2",
                "Singapore",
                "Singapore",
                "238891",
            )],
            tags: vec!["government", "investor"],
        },
        SeedCard {
            name: "Li Mei Chen",
            title: "CTO",
            company: "Sea Limited",
            website: "https://www.sea.com",
            notes: "",
            phones: vec![("mobile", "+65 8456 7890")],
            emails: vec![
                ("work", "meichen@sea.com"),
                ("personal", "limeichen@hotmail.com"),
            ],
            addresses: vec![],
            tags: vec!["fintech", "colleague"],
        },
        SeedCard {
            name: "Rajesh Kumar s/o Subramaniam",
            title: "Principal Consultant",
            company: "McKinsey & Company",
            website: "https://www.mckinsey.com",
            notes: "",
            phones: vec![
                ("mobile", "+60 12-345 6789"),
                ("work", "+60 3-2302 1000"),
            ],
            emails: vec![("work", "rajesh.kumar@mckinsey.com")],
            addresses: vec![(
                "office",
                "Level 34 Menara Citibank 165 Jalan Ampang",
                "Kuala Lumpur",
                "Malaysia",
                "50450",
            )],
            tags: vec!["partner", "vendor"],
        },
        SeedCard {
            name: "Siti Nurbaya Haji Mohamad",
            title: "Senior Director",
            company: "GovTech Singapore",
            website: "https://www.tech.gov.sg",
            notes: "",
            phones: vec![("mobile", "+65 9567 8901")],
            emails: vec![("work", "siti_nurbaya@tech.gov.sg")],
            addresses: vec![],
            tags: vec!["government", "client"],
        },
        SeedCard {
            name: "Kevin Tan Kiat Seng",
            title: "Founder & CEO",
            company: "PaySG Technologies",
            website: "https://paysg.io",
            notes: "",
            phones: vec![("mobile", "+65 9678 9012")],
            emails: vec![
                ("work", "kevin@paysg.io"),
                ("personal", "kevintks@gmail.com"),
            ],
            addresses: vec![(
                "office",
                "71 Ayer Rajah Crescent JTC LaunchPad",
                "Singapore",
                "Singapore",
                "139952",
            )],
            tags: vec!["fintech", "investor", "client"],
        },
        SeedCard {
            name: "Siti Rahimah Binti Abdullah",
            title: "Regional Director",
            company: "Prudential plc",
            website: "https://www.prudential.co.id",
            notes: "",
            phones: vec![
                ("mobile", "+62 812-3456-7890"),
                ("work", "+62 21-5799-8400"),
            ],
            emails: vec![("work", "siti.rahimah@prudential.co.id")],
            addresses: vec![(
                "office",
                "Prudential Tower 7 Jalan Jenderal Sudirman",
                "Jakarta",
                "Indonesia",
                "10220",
            )],
            tags: vec!["fintech", "partner"],
        },
        SeedCard {
            name: "James Wong Wei Jian",
            title: "Head of Engineering",
            company: "Shopee / Sea Group",
            website: "https://shopee.sg",
            notes: "",
            phones: vec![("mobile", "+65 9789 0123")],
            emails: vec![
                ("work", "jameswong@shopee.com"),
                ("personal", "james.wongwj@gmail.com"),
            ],
            addresses: vec![],
            tags: vec!["colleague"],
        },
        SeedCard {
            name: "Anika Sharma",
            title: "Senior Product Manager",
            company: "Agoda Company",
            website: "https://www.agoda.com",
            notes: "",
            phones: vec![("mobile", "+66 89-123-4567")],
            emails: vec![("work", "anika.sharma@agoda.com")],
            addresses: vec![(
                "office",
                "30th Floor The Offices at CentralWorld Ratchadamri Road",
                "Bangkok",
                "Thailand",
                "10330",
            )],
            tags: vec!["colleague", "vendor"],
        },
    ];

    let conn_arc = conn.clone();
    let conn_guard = conn_arc.lock().unwrap();

    for seed in &seeds {
        conn_guard.execute(
            "INSERT INTO cards (name, title, company, website, notes) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![seed.name, seed.title, seed.company, seed.website, seed.notes],
        )?;
        let card_id = conn_guard.last_insert_rowid();

        for (label, number) in &seed.phones {
            conn_guard.execute(
                "INSERT INTO card_phones (card_id, label, number) VALUES (?1, ?2, ?3)",
                params![card_id, label, number],
            )?;
        }
        for (label, address) in &seed.emails {
            conn_guard.execute(
                "INSERT INTO card_emails (card_id, label, address) VALUES (?1, ?2, ?3)",
                params![card_id, label, address],
            )?;
        }
        for (label, street, city, country, postal) in &seed.addresses {
            conn_guard.execute(
                "INSERT INTO card_addresses (card_id, label, street, city, country, postal) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![card_id, label, street, city, country, postal],
            )?;
        }
        for tag in &seed.tags {
            conn_guard.execute(
                "INSERT OR IGNORE INTO tags (name) VALUES (?1)",
                params![tag],
            )?;
            let tag_id: i64 = conn_guard.query_row(
                "SELECT id FROM tags WHERE name = ?1",
                params![tag],
                |r| r.get(0),
            )?;
            conn_guard.execute(
                "INSERT OR IGNORE INTO card_tags (card_id, tag_id) VALUES (?1, ?2)",
                params![card_id, tag_id],
            )?;
        }
    }

    Ok(())
}
