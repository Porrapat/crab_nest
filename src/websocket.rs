use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};

/// WebSocket message types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WsMessage {
    #[serde(rename = "chat")]
    Chat {
        sender_name: String,
        content: String,
        #[serde(default = "default_message_type")]
        message_type: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        voice_url: Option<String>,
        created_at: String,
    },
    #[serde(rename = "voice")]
    Voice {
        sender_name: String,
        voice_url: String,
        created_at: String,
    },
    #[serde(rename = "join")]
    Join { username: String },
    #[serde(rename = "leave")]
    Leave { username: String },
    #[serde(rename = "system")]
    System { message: String },
    #[serde(rename = "ping")]
    Ping { timestamp: u64 },
    #[serde(rename = "pong")]
    Pong { timestamp: u64 },
    #[serde(rename = "user_count")]
    UserCount { count: usize },
    #[serde(rename = "typing")]
    Typing { username: String, is_typing: bool },
    #[serde(rename = "message_deleted")]
    MessageDeleted { message_id: i64 },
}

fn default_message_type() -> String {
    "text".to_string()
}

/// Room channel for broadcasting messages
pub struct RoomChannel {
    pub tx: broadcast::Sender<String>,
    pub user_count: AtomicUsize,
}

impl RoomChannel {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(100);
        Self { 
            tx,
            user_count: AtomicUsize::new(0),
        }
    }

    /// Increment user count and return new count
    pub fn user_joined(&self) -> usize {
        self.user_count.fetch_add(1, Ordering::SeqCst) + 1
    }

    /// Decrement user count and return new count
    pub fn user_left(&self) -> usize {
        let prev = self.user_count.fetch_sub(1, Ordering::SeqCst);
        if prev == 0 {
            // Prevent underflow
            self.user_count.store(0, Ordering::SeqCst);
            0
        } else {
            prev - 1
        }
    }

    /// Get current user count
    pub fn get_user_count(&self) -> usize {
        self.user_count.load(Ordering::SeqCst)
    }

    /// Broadcast user count to all clients
    pub fn broadcast_user_count(&self) {
        let count = self.get_user_count();
        let msg = WsMessage::UserCount { count };
        if let Ok(json) = serde_json::to_string(&msg) {
            let _ = self.tx.send(json);
        }
    }
}

/// Manager for all room channels
pub struct RoomManager {
    rooms: RwLock<HashMap<String, Arc<RoomChannel>>>,
}

impl RoomManager {
    pub fn new() -> Self {
        Self {
            rooms: RwLock::new(HashMap::new()),
        }
    }

    /// Get or create a room channel
    pub async fn get_or_create_room(&self, room_key: &str) -> Arc<RoomChannel> {
        // Try to get existing room first
        {
            let rooms = self.rooms.read().await;
            if let Some(room) = rooms.get(room_key) {
                return room.clone();
            }
        }

        // Create new room if doesn't exist
        let mut rooms = self.rooms.write().await;
        rooms
            .entry(room_key.to_string())
            .or_insert_with(|| Arc::new(RoomChannel::new()))
            .clone()
    }

    /// Get room channel if exists
    pub async fn get_room(&self, room_key: &str) -> Option<Arc<RoomChannel>> {
        let rooms = self.rooms.read().await;
        rooms.get(room_key).cloned()
    }

    /// Broadcast message to room
    pub async fn broadcast(&self, room_key: &str, message: &str) {
        if let Some(room) = self.get_room(room_key).await {
            let _ = room.tx.send(message.to_string());
        }
    }
}

/// Random name generator for users
pub fn generate_random_name() -> String {
    use rand::Rng;
    
    let adjectives = [
        "Happy", "Sleepy", "Brave", "Clever", "Swift",
        "Mighty", "Gentle", "Wild", "Calm", "Eager",
        "Bold", "Bright", "Cool", "Cozy", "Fuzzy",
        "Lucky", "Rusty", "Dusty", "Misty", "Stormy",
    ];
    
    let animals = [
        "Crab", "Fox", "Bear", "Wolf", "Eagle",
        "Tiger", "Lion", "Panda", "Koala", "Otter",
        "Owl", "Hawk", "Deer", "Duck", "Swan",
        "Whale", "Shark", "Seal", "Penguin", "Ferret",
    ];
    
    let mut rng = rand::thread_rng();
    let adj = adjectives[rng.gen_range(0..adjectives.len())];
    let animal = animals[rng.gen_range(0..animals.len())];
    let num: u16 = rng.gen_range(1..1000);
    
    format!("{}{}{}", adj, animal, num)
}

/// Generate unique room key
pub fn generate_room_key() -> String {
    use rand::Rng;
    
    let words = [
        "alpha", "bravo", "charlie", "delta", "echo",
        "foxtrot", "golf", "hotel", "india", "juliet",
        "kilo", "lima", "mike", "november", "oscar",
        "papa", "quebec", "romeo", "sierra", "tango",
        "crab", "rust", "nest", "wave", "tide",
        "coral", "reef", "shell", "pearl", "sand",
    ];
    
    let mut rng = rand::thread_rng();
    let word = words[rng.gen_range(0..words.len())];
    let num: u16 = rng.gen_range(1..10000);
    
    format!("{}-{}", word, num)
}
