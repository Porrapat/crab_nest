ca# 🦀 CrabNest

Real-time chat rooms powered by **Rust** & **WebSocket**

## Features

- **Real-time Chat**: Instant messaging powered by WebSocket technology
- **Persistent History**: All messages are stored permanently in SQLite
- **Unlimited Rooms**: Create as many rooms as you want
- **Random Nicknames**: Get a fun random nickname or set your own
- **Easy Sharing**: Share room links with anyone

## Tech Stack

- **Backend**: Rust + Axum Framework
- **Real-time**: WebSocket
- **Database**: SQLite + SQLx
- **Frontend**: Bootstrap 5 + Rust Professional Theme
- **Templates**: Askama

## Project Structure

```
crab_nest/
├── Cargo.toml              # Dependencies
├── src/
│   ├── main.rs             # Application entry point
│   ├── db.rs               # Database module (SQLite + SQLx)
│   ├── handlers.rs         # HTTP and WebSocket handlers
│   ├── templates.rs        # Askama template definitions
│   └── websocket.rs        # WebSocket utilities
├── templates/
│   ├── base.html           # Base layout template
│   ├── index.html          # Homepage
│   ├── create.html         # Create room page
│   └── room.html           # Chat room page
├── migrations/
│   └── 001_init.sql        # Database schema
└── static/                 # Static assets
```

## Database Schema

```sql
-- Rooms Table
CREATE TABLE rooms (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    room_key TEXT NOT NULL UNIQUE,
    room_name TEXT NOT NULL,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Messages Table  
CREATE TABLE messages (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    room_id INTEGER NOT NULL,
    sender_name TEXT NOT NULL,
    content TEXT NOT NULL,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (room_id) REFERENCES rooms(id) ON DELETE CASCADE
);
```

## Getting Started

### Prerequisites

- Rust (1.70+)
- Cargo

### Run the Application

```bash
# Build and run
cargo run

# Or build in release mode
cargo build --release
./target/release/crab_nest
```

The server will start at `http://localhost:3000`

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `DATABASE_URL` | `sqlite:crab_nest.db?mode=rwc` | SQLite database path |

## Pages

### Homepage (`/`)
- Introduction to CrabNest
- Features overview
- Quick access to create room

### Create Room (`/create`)
- Generate a new unique room key
- Get shareable room link
- Enter room directly

### Chat Room (`/room/{room_key}`)
- Real-time messaging via WebSocket
- View chat history
- Change your nickname
- Share room link

## API Endpoints

| Method | Endpoint | Description |
|--------|----------|-------------|
| `POST` | `/api/rooms` | Create a new room |
| `GET` | `/api/rooms/{room_key}/messages` | Get messages for a room |
| `WS` | `/ws/{room_key}` | WebSocket connection for real-time chat |

### WebSocket Message Types

```json
// Chat message
{
    "type": "chat",
    "sender_name": "HappyCrab123",
    "content": "Hello!",
    "created_at": "2024-01-01T12:00:00Z"
}

// Join notification
{
    "type": "join",
    "username": "HappyCrab123"
}

// Leave notification
{
    "type": "leave", 
    "username": "HappyCrab123"
}

// System message
{
    "type": "system",
    "message": "HappyCrab123 has joined the room"
}
```

## License

MIT License

---

Built with 🦀 Rust & ❤️
