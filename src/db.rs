use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};

/// Database wrapper
#[derive(Clone)]
pub struct Database {
    pool: SqlitePool,
}

/// Room model
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Room {
    pub id: i64,
    pub room_key: String,
    pub room_name: String,
    pub created_at: String,
    pub updated_at: String,
}

/// Message model
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ChatMessage {
    pub id: i64,
    pub room_id: i64,
    pub sender_name: String,
    pub content: String,
    pub created_at: String,
}

/// Run database migrations
pub async fn run_migrations(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS rooms (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            room_key TEXT NOT NULL UNIQUE,
            room_name TEXT NOT NULL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
        )
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS messages (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            room_id INTEGER NOT NULL,
            sender_name TEXT NOT NULL,
            content TEXT NOT NULL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (room_id) REFERENCES rooms(id) ON DELETE CASCADE
        )
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_rooms_key ON rooms(room_key)")
        .execute(pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_messages_room ON messages(room_id)")
        .execute(pool)
        .await?;

    println!("✅ Database migrations completed");
    Ok(())
}

impl Database {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Create a new room
    pub async fn create_room(&self, room_key: &str, room_name: &str) -> Result<Room, sqlx::Error> {
        let result = sqlx::query(
            "INSERT INTO rooms (room_key, room_name) VALUES (?, ?)"
        )
        .bind(room_key)
        .bind(room_name)
        .execute(&self.pool)
        .await?;

        let room = sqlx::query_as::<_, Room>(
            "SELECT * FROM rooms WHERE id = ?"
        )
        .bind(result.last_insert_rowid())
        .fetch_one(&self.pool)
        .await?;

        Ok(room)
    }

    /// Get room by key
    pub async fn get_room_by_key(&self, room_key: &str) -> Result<Option<Room>, sqlx::Error> {
        let room = sqlx::query_as::<_, Room>(
            "SELECT * FROM rooms WHERE room_key = ?"
        )
        .bind(room_key)
        .fetch_optional(&self.pool)
        .await?;

        Ok(room)
    }

    /// Get all rooms
    pub async fn get_all_rooms(&self) -> Result<Vec<Room>, sqlx::Error> {
        let rooms = sqlx::query_as::<_, Room>(
            "SELECT * FROM rooms ORDER BY created_at DESC"
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rooms)
    }

    /// Add message to room
    pub async fn add_message(
        &self,
        room_id: i64,
        sender_name: &str,
        content: &str,
    ) -> Result<ChatMessage, sqlx::Error> {
        let result = sqlx::query(
            "INSERT INTO messages (room_id, sender_name, content) VALUES (?, ?, ?)"
        )
        .bind(room_id)
        .bind(sender_name)
        .bind(content)
        .execute(&self.pool)
        .await?;

        let message = sqlx::query_as::<_, ChatMessage>(
            "SELECT id, room_id, sender_name, content, (replace(created_at, ' ', 'T') || 'Z') as created_at FROM messages WHERE id = ?"
        )
        .bind(result.last_insert_rowid())
        .fetch_one(&self.pool)
        .await?;

        Ok(message)
    }

    /// Get messages for a room
    pub async fn get_messages(&self, room_id: i64) -> Result<Vec<ChatMessage>, sqlx::Error> {
        let messages = sqlx::query_as::<_, ChatMessage>(
            "SELECT * FROM messages WHERE room_id = ? ORDER BY created_at ASC"
        )
        .bind(room_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(messages)
    }

    /// Get messages by room key
    pub async fn get_messages_by_room_key(&self, room_key: &str) -> Result<Vec<ChatMessage>, sqlx::Error> {
        let messages = sqlx::query_as::<_, ChatMessage>(
            r#"
            SELECT m.id, m.room_id, m.sender_name, m.content, 
                   (replace(m.created_at, ' ', 'T') || 'Z') as created_at 
            FROM messages m
            JOIN rooms r ON m.room_id = r.id
            WHERE r.room_key = ?
            ORDER BY m.created_at ASC
            "#
        )
        .bind(room_key)
        .fetch_all(&self.pool)
        .await?;

        Ok(messages)
    }
}
