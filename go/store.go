package main

import (
	"database/sql"
	"fmt"
	"log"
	"strings"

	_ "modernc.org/sqlite"
)

// Store wraps the SQLite connection and provides CRUD operations.
type Store struct {
	db *sql.DB
}

// NewStore opens (or creates) the SQLite database and returns a Store.
func NewStore(path string) (*Store, error) {
	db, err := sql.Open("sqlite", path)
	if err != nil {
		return nil, fmt.Errorf("open db: %w", err)
	}
	s := &Store{db: db}
	if err := s.initSchema(); err != nil {
		return nil, fmt.Errorf("init schema: %w", err)
	}
	return s, nil
}

// Close closes the database connection.
func (s *Store) Close() error { return s.db.Close() }

// Ping checks the database connection.
func (s *Store) Ping() error { return s.db.Ping() }

const schema = `
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
    tag_id  INTEGER REFERENCES tags(id)  ON DELETE CASCADE,
    PRIMARY KEY (card_id, tag_id)
);
`

func (s *Store) initSchema() error {
	_, err := s.db.Exec(schema)
	return err
}

// IsEmpty returns true if the cards table has zero rows.
func (s *Store) IsEmpty() bool {
	var n int
	s.db.QueryRow("SELECT COUNT(*) FROM cards").Scan(&n)
	return n == 0
}

// ── Cards ─────────────────────────────────────────────────────────────────────

// ListCards returns cards matching the optional search query and/or tag filter.
func (s *Store) ListCards(q, tag string) ([]Card, error) {
	base := `
SELECT DISTINCT c.id, c.name, c.title, c.company, c.website, c.notes, c.photo_path, c.created_at, c.updated_at
FROM cards c`

	var args []any
	var where []string

	if tag != "" {
		base += `
JOIN card_tags ct ON ct.card_id = c.id
JOIN tags t ON t.id = ct.tag_id AND t.name = ?`
		args = append(args, tag)
	}
	if q != "" {
		like := "%" + q + "%"
		where = append(where,
			`(c.name LIKE ? OR c.company LIKE ? OR EXISTS (SELECT 1 FROM card_emails e WHERE e.card_id=c.id AND e.address LIKE ?))`)
		args = append(args, like, like, like)
	}
	if len(where) > 0 {
		base += " WHERE " + strings.Join(where, " AND ")
	}
	base += " ORDER BY c.updated_at DESC"

	rows, err := s.db.Query(base, args...)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var cards []Card
	for rows.Next() {
		var c Card
		var photoPath string
		if err := rows.Scan(&c.ID, &c.Name, &c.Title, &c.Company, &c.Website, &c.Notes, &photoPath, &c.CreatedAt, &c.UpdatedAt); err != nil {
			return nil, err
		}
		if photoPath != "" {
			c.PhotoURL = "/" + photoPath
		}
		cards = append(cards, c)
	}
	if err := rows.Err(); err != nil {
		return nil, err
	}

	// Load related data for each card
	for i := range cards {
		if err := s.loadRelated(&cards[i]); err != nil {
			return nil, err
		}
	}
	return cards, nil
}

// GetCard returns the full card by ID.
func (s *Store) GetCard(id int64) (*Card, error) {
	var c Card
	var photoPath string
	err := s.db.QueryRow(`
SELECT id, name, title, company, website, notes, photo_path, created_at, updated_at
FROM cards WHERE id = ?`, id).Scan(
		&c.ID, &c.Name, &c.Title, &c.Company, &c.Website, &c.Notes, &photoPath, &c.CreatedAt, &c.UpdatedAt,
	)
	if err == sql.ErrNoRows {
		return nil, nil
	}
	if err != nil {
		return nil, err
	}
	if photoPath != "" {
		c.PhotoURL = "/" + photoPath
	}
	if err := s.loadRelated(&c); err != nil {
		return nil, err
	}
	return &c, nil
}

// loadRelated populates Phones, Emails, Addresses, and Tags for a Card.
func (s *Store) loadRelated(c *Card) error {
	// Phones
	rows, err := s.db.Query(`SELECT id, label, number FROM card_phones WHERE card_id = ? ORDER BY id`, c.ID)
	if err != nil {
		return err
	}
	defer rows.Close()
	for rows.Next() {
		var p Phone
		if err := rows.Scan(&p.ID, &p.Label, &p.Number); err != nil {
			return err
		}
		c.Phones = append(c.Phones, p)
	}

	// Emails
	rows2, err := s.db.Query(`SELECT id, label, address FROM card_emails WHERE card_id = ? ORDER BY id`, c.ID)
	if err != nil {
		return err
	}
	defer rows2.Close()
	for rows2.Next() {
		var e Email
		if err := rows2.Scan(&e.ID, &e.Label, &e.Address); err != nil {
			return err
		}
		c.Emails = append(c.Emails, e)
	}

	// Addresses
	rows3, err := s.db.Query(`SELECT id, label, street, city, country, postal FROM card_addresses WHERE card_id = ? ORDER BY id`, c.ID)
	if err != nil {
		return err
	}
	defer rows3.Close()
	for rows3.Next() {
		var a Address
		if err := rows3.Scan(&a.ID, &a.Label, &a.Street, &a.City, &a.Country, &a.Postal); err != nil {
			return err
		}
		c.Addresses = append(c.Addresses, a)
	}

	// Tags
	rows4, err := s.db.Query(`
SELECT t.name FROM tags t
JOIN card_tags ct ON ct.tag_id = t.id
WHERE ct.card_id = ? ORDER BY t.name`, c.ID)
	if err != nil {
		return err
	}
	defer rows4.Close()
	for rows4.Next() {
		var name string
		if err := rows4.Scan(&name); err != nil {
			return err
		}
		c.Tags = append(c.Tags, name)
	}

	if c.Phones == nil {
		c.Phones = []Phone{}
	}
	if c.Emails == nil {
		c.Emails = []Email{}
	}
	if c.Addresses == nil {
		c.Addresses = []Address{}
	}
	if c.Tags == nil {
		c.Tags = []string{}
	}
	return nil
}

// CreateCard inserts a new card and all related rows in a transaction.
func (s *Store) CreateCard(c Card) (int64, error) {
	tx, err := s.db.Begin()
	if err != nil {
		return 0, err
	}
	defer tx.Rollback()

	res, err := tx.Exec(`
INSERT INTO cards (name, title, company, website, notes, photo_path)
VALUES (?, ?, ?, ?, ?, ?)`,
		c.Name, c.Title, c.Company, c.Website, c.Notes, "")
	if err != nil {
		return 0, err
	}
	id, _ := res.LastInsertId()
	if err := insertRelated(tx, id, c); err != nil {
		return 0, err
	}
	return id, tx.Commit()
}

// UpdateCard replaces all fields and related rows for an existing card.
func (s *Store) UpdateCard(id int64, c Card) error {
	tx, err := s.db.Begin()
	if err != nil {
		return err
	}
	defer tx.Rollback()

	_, err = tx.Exec(`
UPDATE cards SET name=?, title=?, company=?, website=?, notes=?, updated_at=CURRENT_TIMESTAMP
WHERE id=?`,
		c.Name, c.Title, c.Company, c.Website, c.Notes, id)
	if err != nil {
		return err
	}

	// Delete old related rows
	for _, tbl := range []string{"card_phones", "card_emails", "card_addresses", "card_tags"} {
		if _, err := tx.Exec("DELETE FROM "+tbl+" WHERE card_id=?", id); err != nil {
			return err
		}
	}

	if err := insertRelated(tx, id, c); err != nil {
		return err
	}
	return tx.Commit()
}

// DeleteCard removes the card and all related rows (cascaded).
func (s *Store) DeleteCard(id int64) (string, error) {
	log.Printf("SQL: SELECT photo_path FROM cards WHERE id = %d", id)
	var photoPath string
	s.db.QueryRow("SELECT photo_path FROM cards WHERE id=?", id).Scan(&photoPath)
	log.Printf("SQL: Result - photo_path: %s", photoPath)

	log.Printf("SQL: DELETE FROM cards WHERE id = %d", id)
	res, err := s.db.Exec("DELETE FROM cards WHERE id=?", id)
	if err != nil {
		return photoPath, err
	}
	rowsAffected, _ := res.RowsAffected()
	log.Printf("SQL: Result - rows_affected: %d", rowsAffected)

	return photoPath, err
}

// UpdateCardPhoto sets the photo_path for a card.
func (s *Store) UpdateCardPhoto(id int64, path string) error {
	_, err := s.db.Exec("UPDATE cards SET photo_path=?, updated_at=CURRENT_TIMESTAMP WHERE id=?", path, id)
	return err
}

// DeleteCardPhoto clears the photo_path and returns the old path.
func (s *Store) DeleteCardPhoto(id int64) (string, error) {
	var old string
	s.db.QueryRow("SELECT photo_path FROM cards WHERE id=?", id).Scan(&old)
	_, err := s.db.Exec("UPDATE cards SET photo_path='', updated_at=CURRENT_TIMESTAMP WHERE id=?", id)
	return old, err
}

// insertRelated inserts phones, emails, addresses, and tags inside a transaction.
func insertRelated(tx *sql.Tx, cardID int64, c Card) error {
	for _, p := range c.Phones {
		if _, err := tx.Exec("INSERT INTO card_phones (card_id, label, number) VALUES (?,?,?)", cardID, p.Label, p.Number); err != nil {
			return err
		}
	}
	for _, e := range c.Emails {
		if _, err := tx.Exec("INSERT INTO card_emails (card_id, label, address) VALUES (?,?,?)", cardID, e.Label, e.Address); err != nil {
			return err
		}
	}
	for _, a := range c.Addresses {
		if _, err := tx.Exec("INSERT INTO card_addresses (card_id, label, street, city, country, postal) VALUES (?,?,?,?,?,?)",
			cardID, a.Label, a.Street, a.City, a.Country, a.Postal); err != nil {
			return err
		}
	}
	for _, tagName := range c.Tags {
		if tagName == "" {
			continue
		}
		if _, err := tx.Exec("INSERT OR IGNORE INTO tags (name) VALUES (?)", tagName); err != nil {
			return err
		}
		var tagID int64
		if err := tx.QueryRow("SELECT id FROM tags WHERE name=?", tagName).Scan(&tagID); err != nil {
			return err
		}
		if _, err := tx.Exec("INSERT OR IGNORE INTO card_tags (card_id, tag_id) VALUES (?,?)", cardID, tagID); err != nil {
			return err
		}
	}
	return nil
}

// ListTags returns all tags with usage count, ordered by name.
func (s *Store) ListTags() ([]Tag, error) {
	rows, err := s.db.Query(`
SELECT t.name, COUNT(ct.card_id) as cnt
FROM tags t
LEFT JOIN card_tags ct ON ct.tag_id = t.id
GROUP BY t.id, t.name
ORDER BY t.name`)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	var tags []Tag
	for rows.Next() {
		var t Tag
		if err := rows.Scan(&t.Name, &t.Count); err != nil {
			return nil, err
		}
		tags = append(tags, t)
	}
	if tags == nil {
		tags = []Tag{}
	}
	return tags, rows.Err()
}

// ── Seed data ─────────────────────────────────────────────────────────────────

// SeedData inserts 10 realistic SE Asia contacts.
func (s *Store) SeedData() error {
	type seedCard struct {
		card      Card
		phones    []Phone
		emails    []Email
		addresses []Address
		tags      []string
	}

	seeds := []seedCard{
		{
			card: Card{Name: "Tan Wei Ming", Title: "Chief Executive Officer", Company: "DBS Group Holdings", Website: "https://www.dbs.com", Notes: "Met at Singapore Fintech Festival 2025"},
			phones: []Phone{
				{Label: "mobile", Number: "+65 9123 4567"},
				{Label: "work", Number: "+65 6878 8888"},
			},
			emails: []Email{
				{Label: "work", Address: "weiming.tan@dbs.com"},
				{Label: "personal", Address: "wm.tan@gmail.com"},
			},
			addresses: []Address{
				{Label: "office", Street: "12 Marina Boulevard, DBS Asia Hub 2", City: "Singapore", Country: "Singapore", Postal: "018982"},
			},
			tags: []string{"fintech", "client", "investor"},
		},
		{
			card: Card{Name: "Priya Krishnamurthy", Title: "VP Engineering", Company: "Grab Holdings", Website: "https://www.grab.com", Notes: "Collaborated on payments infrastructure"},
			phones: []Phone{
				{Label: "mobile", Number: "+65 8234 5678"},
			},
			emails: []Email{
				{Label: "work", Address: "priya.k@grab.com"},
			},
			addresses: []Address{
				{Label: "office", Street: "3 Media Close, One-North", City: "Singapore", Country: "Singapore", Postal: "138498"},
			},
			tags: []string{"fintech", "partner", "colleague"},
		},
		{
			card: Card{Name: "Ahmad Fauzi bin Rashid", Title: "Director of Investments", Company: "Temasek Holdings", Website: "https://www.temasek.com.sg", Notes: "Investor relations contact"},
			phones: []Phone{
				{Label: "mobile", Number: "+65 9345 6789"},
				{Label: "work", Number: "+65 6308 2222"},
			},
			emails: []Email{
				{Label: "work", Address: "ahmad.fauzi@temasek.com.sg"},
			},
			addresses: []Address{
				{Label: "office", Street: "60B Orchard Road, Tower 2", City: "Singapore", Country: "Singapore", Postal: "238891"},
			},
			tags: []string{"government", "investor"},
		},
		{
			card: Card{Name: "Li Mei Chen", Title: "Chief Technology Officer", Company: "Sea Limited", Website: "https://www.sea.com", Notes: "Introduced by James Wong"},
			phones: []Phone{
				{Label: "mobile", Number: "+65 8456 7890"},
			},
			emails: []Email{
				{Label: "work", Address: "meichen@sea.com"},
				{Label: "personal", Address: "limeichen@hotmail.com"},
			},
			tags: []string{"fintech", "colleague"},
		},
		{
			card: Card{Name: "Rajesh Kumar s/o Subramaniam", Title: "Principal Consultant", Company: "McKinsey & Company", Website: "https://www.mckinsey.com", Notes: "Strategy consulting, KL office lead"},
			phones: []Phone{
				{Label: "mobile", Number: "+60 12-345 6789"},
				{Label: "work", Number: "+60 3-2302 1000"},
			},
			emails: []Email{
				{Label: "work", Address: "rajesh.kumar@mckinsey.com"},
			},
			addresses: []Address{
				{Label: "office", Street: "Level 34, Menara Citibank, 165 Jalan Ampang", City: "Kuala Lumpur", Country: "Malaysia", Postal: "50450"},
			},
			tags: []string{"partner", "vendor"},
		},
		{
			card: Card{Name: "Siti Nurbaya Haji Mohamad", Title: "Senior Director", Company: "GovTech Singapore", Website: "https://www.tech.gov.sg", Notes: "Digital government partnerships"},
			phones: []Phone{
				{Label: "mobile", Number: "+65 9567 8901"},
			},
			emails: []Email{
				{Label: "work", Address: "siti_nurbaya@tech.gov.sg"},
			},
			tags: []string{"government", "client"},
		},
		{
			card: Card{Name: "Kevin Tan Kiat Seng", Title: "Founder & CEO", Company: "PaySG Technologies", Website: "https://www.paysg.io", Notes: "Seed stage, looking for Series A"},
			phones: []Phone{
				{Label: "mobile", Number: "+65 9678 9012"},
			},
			emails: []Email{
				{Label: "work", Address: "kevin@paysg.io"},
				{Label: "personal", Address: "kevintks@gmail.com"},
			},
			addresses: []Address{
				{Label: "office", Street: "71 Ayer Rajah Crescent, JTC LaunchPad", City: "Singapore", Country: "Singapore", Postal: "139952"},
			},
			tags: []string{"fintech", "investor", "client"},
		},
		{
			card: Card{Name: "Siti Rahimah Binti Abdullah", Title: "Regional Director", Company: "Prudential plc", Website: "https://www.prudential.co.id", Notes: "Insurance & wealth management, Indonesia"},
			phones: []Phone{
				{Label: "mobile", Number: "+62 812-3456-7890"},
				{Label: "work", Number: "+62 21-5799-8400"},
			},
			emails: []Email{
				{Label: "work", Address: "siti.rahimah@prudential.co.id"},
			},
			addresses: []Address{
				{Label: "office", Street: "Prudential Tower, 7 Jalan Jenderal Sudirman", City: "Jakarta", Country: "Indonesia", Postal: "10220"},
			},
			tags: []string{"fintech", "partner"},
		},
		{
			card: Card{Name: "James Wong Wei Jian", Title: "Head of Engineering", Company: "Shopee / Sea Group", Website: "https://shopee.sg", Notes: "Ex-PayPal, strong mobile payments background"},
			phones: []Phone{
				{Label: "mobile", Number: "+65 9789 0123"},
			},
			emails: []Email{
				{Label: "work", Address: "jameswong@shopee.com"},
				{Label: "personal", Address: "james.wongwj@gmail.com"},
			},
			tags: []string{"colleague"},
		},
		{
			card: Card{Name: "Anika Sharma", Title: "Senior Product Manager", Company: "Agoda Company", Website: "https://www.agoda.com", Notes: "Travel tech; met at ProductCon Bangkok"},
			phones: []Phone{
				{Label: "mobile", Number: "+66 89-123-4567"},
			},
			emails: []Email{
				{Label: "work", Address: "anika.sharma@agoda.com"},
			},
			addresses: []Address{
				{Label: "office", Street: "30th Floor, The Offices at CentralWorld, Ratchadamri Road", City: "Bangkok", Country: "Thailand", Postal: "10330"},
			},
			tags: []string{"colleague", "vendor"},
		},
	}

	for _, s2 := range seeds {
		c := s2.card
		c.Phones = s2.phones
		c.Emails = s2.emails
		c.Addresses = s2.addresses
		c.Tags = s2.tags
		if _, err := s.CreateCard(c); err != nil {
			return fmt.Errorf("seed %q: %w", c.Name, err)
		}
	}
	return nil
}
