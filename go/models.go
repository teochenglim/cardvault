package main

// Phone is a single phone entry for a card.
type Phone struct {
	ID     int64  `json:"id"`
	CardID int64  `json:"card_id,omitempty"`
	Label  string `json:"label"`
	Number string `json:"number"`
}

// Email is a single email entry for a card.
type Email struct {
	ID      int64  `json:"id"`
	CardID  int64  `json:"card_id,omitempty"`
	Label   string `json:"label"`
	Address string `json:"address"`
}

// Address is a single address entry for a card.
type Address struct {
	ID      int64  `json:"id"`
	CardID  int64  `json:"card_id,omitempty"`
	Label   string `json:"label"`
	Street  string `json:"street"`
	City    string `json:"city"`
	Country string `json:"country"`
	Postal  string `json:"postal"`
}

// Card is the full card record returned by the API.
type Card struct {
	ID        int64     `json:"id"`
	Name      string    `json:"name"`
	Title     string    `json:"title"`
	Company   string    `json:"company"`
	Website   string    `json:"website"`
	Notes     string    `json:"notes"`
	PhotoURL  string    `json:"photo_url"`
	Phones    []Phone   `json:"phones"`
	Emails    []Email   `json:"emails"`
	Addresses []Address `json:"addresses"`
	Tags      []string  `json:"tags"`
	CreatedAt string    `json:"created_at"`
	UpdatedAt string    `json:"updated_at"`
}

// Tag is a tag with its usage count.
type Tag struct {
	Name  string `json:"name"`
	Count int    `json:"count"`
}

// HealthResponse is returned by GET /health.
type HealthResponse struct {
	Status string `json:"status"`
	DB     string `json:"db"`
}
