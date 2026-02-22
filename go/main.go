package main

import (
	"context"
	"embed"
	"flag"
	"fmt"
	"io/fs"
	"log"
	"net/http"
	"os"
	"os/signal"
	"syscall"
	"time"
)

//go:embed static/*
var staticFiles embed.FS

// serveIndex serves the embedded index.html for any non-API route.
func serveIndex(w http.ResponseWriter, r *http.Request) {
	data, err := staticFiles.ReadFile("static/index.html")
	if err != nil {
		http.Error(w, "index.html not found", http.StatusInternalServerError)
		return
	}
	w.Header().Set("Content-Type", "text/html; charset=utf-8")
	w.Write(data)
}

// staticFS returns a sub-filesystem rooted at static/ for serving assets.
func staticFS() (fs.FS, error) {
	return fs.Sub(staticFiles, "static")
}

func main() {
	// ── CLI flags ──────────────────────────────────────────────────────────
	port       := flag.String("port",        envOr("PORT", "8080"),                    "HTTP listen port")
	dbPath     := flag.String("db",          envOr("CARDVAULT_DB", "cardvault.db"),    "SQLite database path")
	uploadsDir := flag.String("uploads-dir", envOr("CARDVAULT_UPLOADS", "uploads"),   "Directory for uploaded photos")
	seed       := flag.Bool("seed", false, "Insert seed data if the database is empty")
	flag.Parse()

	// ── Uploads directory ──────────────────────────────────────────────────
	if err := os.MkdirAll(*uploadsDir, 0755); err != nil {
		log.Fatalf("create uploads dir: %v", err)
	}

	// ── Database ───────────────────────────────────────────────────────────
	store, err := NewStore(*dbPath)
	if err != nil {
		log.Fatalf("open store: %v", err)
	}
	defer store.Close()

	if *seed && store.IsEmpty() {
		log.Println("seeding database with sample data…")
		if err := store.SeedData(); err != nil {
			log.Fatalf("seed: %v", err)
		}
		log.Println("seed complete")
	}

	// ── Router ─────────────────────────────────────────────────────────────
	handler := NewHandler(store, *uploadsDir)

	srv := &http.Server{
		Addr:         ":" + *port,
		Handler:      handler,
		ReadTimeout:  15 * time.Second,
		WriteTimeout: 30 * time.Second,
		IdleTimeout:  60 * time.Second,
	}

	// ── Graceful shutdown ──────────────────────────────────────────────────
	done := make(chan struct{})
	go func() {
		quit := make(chan os.Signal, 1)
		signal.Notify(quit, syscall.SIGINT, syscall.SIGTERM)
		sig := <-quit
		log.Printf("received signal %v, shutting down…", sig)
		ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
		defer cancel()
		if err := srv.Shutdown(ctx); err != nil {
			log.Printf("shutdown error: %v", err)
		}
		close(done)
	}()

	fmt.Printf("CardVault listening on http://localhost:%s\n", *port)
	if err := srv.ListenAndServe(); err != nil && err != http.ErrServerClosed {
		log.Fatalf("listen: %v", err)
	}
	<-done
	log.Println("server stopped")
}

// envOr returns the value of the environment variable or the fallback.
func envOr(key, fallback string) string {
	if v := os.Getenv(key); v != "" {
		return v
	}
	return fallback
}
