mod handlers;
mod models;
mod store;

use std::sync::{Arc, Mutex};

use anyhow::Result;
use axum::{
    routing::{get, post},
    Router,
};
use clap::Parser;
use handlers::AppState;
use rust_embed::RustEmbed;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::ServeDir;
use tracing::info;
use tracing_subscriber::EnvFilter;

#[derive(RustEmbed)]
#[folder = "src/static/"]
pub struct Asset;

#[derive(Parser, Debug)]
#[command(name = "cardvault", about = "CardVault business card manager")]
struct Cli {
    /// Port to listen on
    #[arg(long, env = "PORT", default_value = "8080")]
    port: u16,

    /// SQLite database path
    #[arg(long, env = "CARDVAULT_DB", default_value = "cardvault.db")]
    db: String,

    /// Directory for uploaded photos
    #[arg(long, env = "CARDVAULT_UPLOADS", default_value = "uploads")]
    uploads_dir: String,

    /// Seed the database with sample data if empty
    #[arg(long, default_value_t = false)]
    seed: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("cardvault=info".parse()?))
        .init();

    let cli = Cli::parse();

    // Open SQLite connection
    let connection = rusqlite::Connection::open(&cli.db)?;
    let conn = Arc::new(Mutex::new(connection));

    // Initialize schema
    store::init_db(&conn)?;

    // Seed if requested and DB is empty
    if cli.seed && store::is_empty(&conn) {
        info!("Seeding database with sample contacts...");
        store::seed_data(&conn)?;
        info!("Seeded 10 contacts.");
    }

    // Ensure uploads directory exists
    tokio::fs::create_dir_all(&cli.uploads_dir).await?;

    // Extract embedded static files to ./static/ next to the binary
    let static_dir = std::path::PathBuf::from("static");
    tokio::fs::create_dir_all(&static_dir).await?;
    for path in Asset::iter() {
        if let Some(content) = Asset::get(&path) {
            tokio::fs::write(static_dir.join(path.as_ref()), content.data).await?;
        }
    }

    let state = Arc::new(AppState {
        conn,
        uploads_dir: cli.uploads_dir.clone(),
    });

    // CORS: allow all
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        // Index
        .route("/", get(handlers::serve_index))
        // Static assets (CSS, JS) — served from extracted temp dir
        .nest_service("/static", ServeDir::new(&static_dir))
        // Uploads
        .route("/uploads/{filename}", get(handlers::serve_uploads))
        // Health
        .route("/health", get(handlers::health))
        // Cards
        .route("/api/cards", get(handlers::list_cards).post(handlers::create_card))
        .route("/api/cards/:id", get(handlers::get_card).put(handlers::update_card).delete(handlers::delete_card))
        // Photos
        .route("/api/cards/:id/photo", post(handlers::upload_photo).delete(handlers::delete_photo))
        // Tags
        .route("/api/tags", get(handlers::list_tags))
        // Middleware
        .layer(cors)
        .with_state(state);

    let addr = format!("0.0.0.0:{}", cli.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    info!("CardVault listening on http://localhost:{}", cli.port);

    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            tokio::signal::ctrl_c()
                .await
                .expect("failed to install CTRL+C signal handler");
            info!("Shutting down CardVault...");
        })
        .await?;

    Ok(())
}
