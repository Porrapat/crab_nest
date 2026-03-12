# 🦀 CrabNest

Real-time chat rooms powered by **Rust** & **WebSocket**

## Features

- **Real-time Chat**: Instant messaging powered by WebSocket technology
- **Persistent History**: All messages are stored permanently in SQLite
- **Push To Talk**: You can press button to start voice talk and it keeps the files.
- **Unlimited Rooms**: Create as many rooms as you want
- **Random Nicknames**: Get a fun random nickname or set your own
- **Easy Sharing**: Share room links with anyone

## Getting Started

### Prerequisites

- Rust (1.80+)
- Cargo

### Run the Application

```bash
# Build and run
cargo run

# Or build in release mode
cargo build --release
./target/release/crab_nest
```

The server will start at `http://localhost:3000` or port you specified on `.env` You have to deploy on the hosting.

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

## Security Notice

All messages in **CrabNest** are currently **not encrypted.**
Administrators may have access to all conversation data.

For your safety, **please avoid sharing any personal information, passwords, or sensitive data** in this chat.

## License

MIT License

---

Built with 🦀 Rust & ❤️
