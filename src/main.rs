use axum::{routing::{get, post}, Router};
use sqlx::sqlite::SqlitePoolOptions;
use std::sync::Arc;
use tower_http::services::ServeDir;

mod db;
mod handlers;
mod templates;
mod websocket;

use db::Database;
use websocket::RoomManager;

/// Application State
#[derive(Clone)]
pub struct AppState {
    pub db: Database,
    pub room_manager: Arc<RoomManager>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize database
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite:crab_nest.db?mode=rwc".to_string());
    
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;

    // Run migrations
    db::run_migrations(&pool).await?;

    let db = Database::new(pool);
    let room_manager = Arc::new(RoomManager::new());

    let state = AppState { db, room_manager };

    // Build router
    let app = Router::new()
        // Pages
        .route("/", get(handlers::index_page))
        .route("/create", get(handlers::create_room_page))
        .route("/room/:room_key", get(handlers::room_page))
        // API
        .route("/api/rooms", post(handlers::create_room))
        .route("/api/rooms/:room_key/messages", get(handlers::get_messages))
        // WebSocket
        .route("/ws/:room_key", get(handlers::websocket_handler))
        // Static files
        .nest_service("/static", ServeDir::new("static"))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    println!("🦀 CrabNest is running on http://localhost:3000");
    
    axum::serve(listener, app).await?;
    Ok(())
}
