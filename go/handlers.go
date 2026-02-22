package main

import (
	"encoding/json"
	"fmt"
	"io"
	"log"
	"mime/multipart"
	"net/http"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"time"
)

// Handler holds shared dependencies for HTTP handlers.
type Handler struct {
	store      *Store
	uploadsDir string
}

// NewHandler creates a new Handler.
func NewHandler(store *Store, uploadsDir string) *Handler {
	return &Handler{store: store, uploadsDir: uploadsDir}
}

// ServeHTTP is the top-level router.
func (h *Handler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	// CORS
	w.Header().Set("Access-Control-Allow-Origin", "*")
	w.Header().Set("Access-Control-Allow-Methods", "GET,POST,PUT,DELETE,OPTIONS")
	w.Header().Set("Access-Control-Allow-Headers", "Content-Type")
	if r.Method == http.MethodOptions {
		w.WriteHeader(http.StatusNoContent)
		return
	}

	// Wrap response writer to capture status code
	lrw := &loggingResponseWriter{ResponseWriter: w, statusCode: http.StatusOK}

	path := r.URL.Path

	switch {
	case path == "/health":
		h.handleHealth(lrw, r)
	case path == "/api/cards" || path == "/api/cards/":
		h.handleCards(lrw, r)
	case strings.HasPrefix(path, "/api/cards/"):
		h.routeCard(lrw, r, path)
	case path == "/api/tags" || path == "/api/tags/":
		h.handleTags(lrw, r)
	case strings.HasPrefix(path, "/uploads/"):
		h.handleUploads(lrw, r)
	default:
		// Serve index.html for everything else (SPA)
		serveIndex(lrw, r)
	}

	// Log the request
	h.logRequest(r, lrw.statusCode)
}

// loggingResponseWriter wraps http.ResponseWriter to capture status code
type loggingResponseWriter struct {
	http.ResponseWriter
	statusCode int
}

func (lrw *loggingResponseWriter) WriteHeader(code int) {
	lrw.statusCode = code
	lrw.ResponseWriter.WriteHeader(code)
}

// logRequest logs the request in nginx-style format
func (h *Handler) logRequest(r *http.Request, statusCode int) {
	userAgent := r.Header.Get("User-Agent")
	if userAgent == "" {
		userAgent = "-"
	}
	referer := r.Header.Get("Referer")
	if referer == "" {
		referer = "-"
	}
	query := r.URL.RawQuery
	if query == "" {
		query = "-"
	}
	log.Printf("%s %s %s %d \"%s\" \"%s\"", r.Method, r.URL.Path, query, statusCode, userAgent, referer)
}

// routeCard dispatches /api/cards/:id and /api/cards/:id/photo
func (h *Handler) routeCard(w http.ResponseWriter, r *http.Request, path string) {
	// Strip /api/cards/
	rest := strings.TrimPrefix(path, "/api/cards/")
	parts := strings.SplitN(rest, "/", 2)
	idStr := parts[0]
	id, err := strconv.ParseInt(idStr, 10, 64)
	if err != nil {
		jsonError(w, "invalid card id", http.StatusBadRequest)
		return
	}

	if len(parts) == 2 && parts[1] == "photo" {
		h.handleCardPhoto(w, r, id)
		return
	}
	h.handleCard(w, r, id)
}

// ── /health ───────────────────────────────────────────────────────────────────

func (h *Handler) handleHealth(w http.ResponseWriter, r *http.Request) {
	dbStatus := "ok"
	if err := h.store.Ping(); err != nil {
		dbStatus = "error"
	}
	jsonOK(w, HealthResponse{Status: "ok", DB: dbStatus})
}

// ── /api/cards ────────────────────────────────────────────────────────────────

func (h *Handler) handleCards(w http.ResponseWriter, r *http.Request) {
	switch r.Method {
	case http.MethodGet:
		q := r.URL.Query().Get("q")
		tag := r.URL.Query().Get("tag")
		cards, err := h.store.ListCards(q, tag)
		if err != nil {
			jsonError(w, err.Error(), http.StatusInternalServerError)
			return
		}
		jsonOK(w, cards)

	case http.MethodPost:
		card, file, fileHeader, err := h.parseCardForm(r)
		if err != nil {
			jsonError(w, err.Error(), http.StatusBadRequest)
			return
		}
		id, err := h.store.CreateCard(card)
		if err != nil {
			jsonError(w, err.Error(), http.StatusInternalServerError)
			return
		}
		if file != nil {
			photoPath, err := h.savePhoto(id, fileHeader.Filename, file)
			if err == nil {
				h.store.UpdateCardPhoto(id, photoPath)
			}
		}
		c, _ := h.store.GetCard(id)
		w.WriteHeader(http.StatusCreated)
		jsonOK(w, c)

	default:
		jsonError(w, "method not allowed", http.StatusMethodNotAllowed)
	}
}

// ── /api/cards/:id ────────────────────────────────────────────────────────────

func (h *Handler) handleCard(w http.ResponseWriter, r *http.Request, id int64) {
	switch r.Method {
	case http.MethodGet:
		c, err := h.store.GetCard(id)
		if err != nil {
			jsonError(w, err.Error(), http.StatusInternalServerError)
			return
		}
		if c == nil {
			jsonError(w, "not found", http.StatusNotFound)
			return
		}
		jsonOK(w, c)

	case http.MethodPut:
		card, file, fileHeader, err := h.parseCardForm(r)
		if err != nil {
			jsonError(w, err.Error(), http.StatusBadRequest)
			return
		}
		if err := h.store.UpdateCard(id, card); err != nil {
			jsonError(w, err.Error(), http.StatusInternalServerError)
			return
		}
		if file != nil {
			photoPath, err := h.savePhoto(id, fileHeader.Filename, file)
			if err == nil {
				h.store.UpdateCardPhoto(id, photoPath)
			}
		}
		c, _ := h.store.GetCard(id)
		jsonOK(w, c)

	case http.MethodDelete:
		photoPath, err := h.store.DeleteCard(id)
		if err != nil {
			jsonError(w, err.Error(), http.StatusInternalServerError)
			return
		}
		if photoPath != "" {
			os.Remove(photoPath)
		}
		w.WriteHeader(http.StatusNoContent)

	default:
		jsonError(w, "method not allowed", http.StatusMethodNotAllowed)
	}
}

// ── /api/cards/:id/photo ──────────────────────────────────────────────────────

func (h *Handler) handleCardPhoto(w http.ResponseWriter, r *http.Request, id int64) {
	switch r.Method {
	case http.MethodPost:
		if err := r.ParseMultipartForm(6 << 20); err != nil {
			jsonError(w, "bad multipart", http.StatusBadRequest)
			return
		}
		file, fh, err := r.FormFile("photo")
		if err != nil {
			jsonError(w, "photo field missing", http.StatusBadRequest)
			return
		}
		defer file.Close()
		photoPath, err := h.savePhoto(id, fh.Filename, file)
		if err != nil {
			jsonError(w, err.Error(), http.StatusBadRequest)
			return
		}
		if err := h.store.UpdateCardPhoto(id, photoPath); err != nil {
			jsonError(w, err.Error(), http.StatusInternalServerError)
			return
		}
		jsonOK(w, map[string]string{"photo_url": "/" + photoPath})

	case http.MethodDelete:
		old, err := h.store.DeleteCardPhoto(id)
		if err != nil {
			jsonError(w, err.Error(), http.StatusInternalServerError)
			return
		}
		if old != "" {
			os.Remove(old)
		}
		w.WriteHeader(http.StatusNoContent)

	default:
		jsonError(w, "method not allowed", http.StatusMethodNotAllowed)
	}
}

// ── /api/tags ─────────────────────────────────────────────────────────────────

func (h *Handler) handleTags(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodGet {
		jsonError(w, "method not allowed", http.StatusMethodNotAllowed)
		return
	}
	tags, err := h.store.ListTags()
	if err != nil {
		jsonError(w, err.Error(), http.StatusInternalServerError)
		return
	}
	jsonOK(w, tags)
}

// ── /uploads/:filename ────────────────────────────────────────────────────────

func (h *Handler) handleUploads(w http.ResponseWriter, r *http.Request) {
	filename := strings.TrimPrefix(r.URL.Path, "/uploads/")
	// Prevent directory traversal
	filename = filepath.Base(filename)
	fullPath := filepath.Join(h.uploadsDir, filename)
	http.ServeFile(w, r, fullPath)
}

// ── Helpers ───────────────────────────────────────────────────────────────────

// parseCardForm parses a multipart form and returns a Card and optional file.
func (h *Handler) parseCardForm(r *http.Request) (Card, io.ReadCloser, *multipart.FileHeader, error) {
	if err := r.ParseMultipartForm(6 << 20); err != nil {
		return Card{}, nil, nil, fmt.Errorf("parse multipart: %w", err)
	}

	var card Card
	card.Name    = strings.TrimSpace(r.FormValue("name"))
	card.Title   = strings.TrimSpace(r.FormValue("title"))
	card.Company = strings.TrimSpace(r.FormValue("company"))
	card.Website = strings.TrimSpace(r.FormValue("website"))
	card.Notes   = strings.TrimSpace(r.FormValue("notes"))

	if card.Name == "" {
		return card, nil, nil, fmt.Errorf("name is required")
	}

	// Parse JSON sub-fields
	if v := r.FormValue("phones"); v != "" {
		json.Unmarshal([]byte(v), &card.Phones)
	}
	if v := r.FormValue("emails"); v != "" {
		json.Unmarshal([]byte(v), &card.Emails)
	}
	if v := r.FormValue("addresses"); v != "" {
		json.Unmarshal([]byte(v), &card.Addresses)
	}
	if v := r.FormValue("tags"); v != "" {
		json.Unmarshal([]byte(v), &card.Tags)
	}

	// Optional photo
	file, fh, err := r.FormFile("photo")
	if err != nil {
		return card, nil, nil, nil // no photo is fine
	}
	return card, file, fh, nil
}

// savePhoto saves the uploaded photo to disk and returns the relative path.
func (h *Handler) savePhoto(cardID int64, originalName string, r io.ReadCloser) (string, error) {
	defer r.Close()

	// Validate extension
	ext := strings.ToLower(filepath.Ext(originalName))
	allowed := map[string]bool{".jpg": true, ".jpeg": true, ".png": true, ".webp": true}
	if !allowed[ext] {
		return "", fmt.Errorf("only jpg/png/webp allowed")
	}

	// Unique filename
	fname := fmt.Sprintf("card_%d_%d%s", cardID, time.Now().UnixNano(), ext)
	dest := filepath.Join(h.uploadsDir, fname)

	f, err := os.Create(dest)
	if err != nil {
		return "", fmt.Errorf("create file: %w", err)
	}
	defer f.Close()

	n, err := io.Copy(f, r)
	if err != nil {
		return "", fmt.Errorf("write file: %w", err)
	}
	if n > 5<<20 {
		os.Remove(dest)
		return "", fmt.Errorf("file too large (max 5 MB)")
	}

	return "uploads/" + fname, nil
}

// jsonOK writes a JSON 200 response.
func jsonOK(w http.ResponseWriter, v any) {
	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(v)
}

// jsonError writes a JSON error response.
func jsonError(w http.ResponseWriter, msg string, code int) {
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(code)
	json.NewEncoder(w).Encode(map[string]string{"error": msg})
}
