use askama::Template;
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Multipart, Path, State,
    },
    http::StatusCode,
    response::{Html, IntoResponse, Json, Response},
};
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::io::AsyncWriteExt;

use crate::db::ChatMessage;
use crate::templates::{CreateRoomTemplate, IndexTemplate, RoomTemplate};
use crate::websocket::{generate_random_name, generate_room_key, WsMessage};
use crate::AppState;

/// Index page handler
pub async fn index_page() -> impl IntoResponse {
    let template = IndexTemplate;
    match template.render() {
        Ok(html) => Html(html).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

/// Create room page handler
pub async fn create_room_page() -> impl IntoResponse {
    let template = CreateRoomTemplate;
    match template.render() {
        Ok(html) => Html(html).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

/// Room page handler
pub async fn room_page(Path(room_key): Path<String>) -> impl IntoResponse {
    let username = generate_random_name();
    let template = RoomTemplate { room_key, username };
    match template.render() {
        Ok(html) => Html(html).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

/// API: Create a new room
#[derive(Deserialize)]
pub struct CreateRoomRequest {
    pub room_name: Option<String>,
}

#[derive(Serialize)]
pub struct CreateRoomResponse {
    pub room_key: String,
    pub room_name: String,
    pub room_url: String,
}

pub async fn create_room(
    State(state): State<AppState>,
    Json(payload): Json<CreateRoomRequest>,
) -> Result<Json<CreateRoomResponse>, (StatusCode, String)> {
    let room_key = generate_room_key();
    let room_name = payload
        .room_name
        .unwrap_or_else(|| format!("Room {}", &room_key));

    match state.db.create_room(&room_key, &room_name).await {
        Ok(room) => Ok(Json(CreateRoomResponse {
            room_key: room.room_key.clone(),
            room_name: room.room_name,
            room_url: format!("/room/{}", room.room_key),
        })),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to create room: {}", e),
        )),
    }
}

/// API: Get messages for a room
pub async fn get_messages(
    State(state): State<AppState>,
    Path(room_key): Path<String>,
) -> Result<Json<Vec<ChatMessage>>, (StatusCode, String)> {
    match state.db.get_messages_by_room_key(&room_key).await {
        Ok(messages) => Ok(Json(messages)),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to get messages: {}", e),
        )),
    }
}

/// WebSocket handler
pub async fn websocket_handler(
    ws: WebSocketUpgrade,
    Path(room_key): Path<String>,
    State(state): State<AppState>,
) -> Response {
    ws.on_upgrade(move |socket| handle_socket(socket, room_key, state))
}

/// Heartbeat interval in seconds
const HEARTBEAT_INTERVAL: u64 = 30;

/// Handle WebSocket connection
async fn handle_socket(socket: WebSocket, room_key: String, state: AppState) {
    let (mut ws_sender, mut ws_receiver) = socket.split();

    // Get or create room channel
    let room_channel = state.room_manager.get_or_create_room(&room_key).await;
    let mut broadcast_rx = room_channel.tx.subscribe();

    // Ensure room exists in database
    if state.db.get_room_by_key(&room_key).await.unwrap().is_none() {
        let room_name = format!("Room {}", &room_key);
        let _ = state.db.create_room(&room_key, &room_name).await;
    }

    // Increment user count when connection is established
    room_channel.user_joined();
    room_channel.broadcast_user_count();

    // Channel for sending messages to the client (from broadcast and heartbeat)
    let (tx, mut rx) = mpsc::channel::<Message>(100);

    // Flag to track if client is alive (received pong)
    let alive = Arc::new(AtomicBool::new(true));
    let alive_clone = alive.clone();

    // Task: Send messages to WebSocket client
    let mut send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if ws_sender.send(msg).await.is_err() {
                break;
            }
        }
    });

    // Task: Forward broadcast messages to sender channel
    let tx_broadcast = tx.clone();
    let mut broadcast_task = tokio::spawn(async move {
        while let Ok(msg) = broadcast_rx.recv().await {
            if tx_broadcast.send(Message::Text(msg.into())).await.is_err() {
                break;
            }
        }
    });

    // Task: Send periodic heartbeat pings
    let tx_heartbeat = tx.clone();
    let alive_heartbeat = alive.clone();
    let mut heartbeat_task = tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(HEARTBEAT_INTERVAL));

        loop {
            interval.tick().await;

            // Check if client responded to last ping
            if !alive_heartbeat.load(Ordering::SeqCst) {
                // Client didn't respond, connection is dead
                break;
            }

            // Mark as not alive until we receive a pong
            alive_heartbeat.store(false, Ordering::SeqCst);

            // Send application-level ping
            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64;

            let ping_msg = WsMessage::Ping { timestamp };
            if let Ok(json) = serde_json::to_string(&ping_msg) {
                if tx_heartbeat.send(Message::Text(json.into())).await.is_err() {
                    break;
                }
            }

            // Also send WebSocket protocol-level ping
            if tx_heartbeat.send(Message::Ping(vec![].into())).await.is_err() {
                break;
            }
        }
    });

    // Task: Handle incoming messages from client
    let room_key_clone = room_key.clone();
    let state_clone = state.clone();
    let room_channel_clone = state.room_manager.get_or_create_room(&room_key).await;
    let tx_recv = tx.clone();

    let mut recv_task = tokio::spawn(async move {
        while let Some(result) = ws_receiver.next().await {
            match result {
                Ok(msg) => match msg {
                    Message::Text(text) => {
                        // Parse incoming message
                        if let Ok(ws_msg) = serde_json::from_str::<WsMessage>(&text) {
                            match ws_msg {
                                WsMessage::Chat {
                                    sender_name,
                                    content,
                                    ..
                                } => {
                                    // Check message length limit (max 2000 characters)
                                    const MAX_MESSAGE_LENGTH: usize = 2000;
                                    if content.len() > MAX_MESSAGE_LENGTH {
                                        // Send error message back to the client
                                        let error_msg = WsMessage::System {
                                            message: "Max message length limit at 2000".to_string(),
                                        };
                                        if let Ok(json) = serde_json::to_string(&error_msg) {
                                            let _ = tx_recv.send(Message::Text(json.into())).await;
                                        }
                                        continue;
                                    }

                                    // Get room from database
                                    if let Ok(Some(room)) =
                                        state_clone.db.get_room_by_key(&room_key_clone).await
                                    {
                                        // Save message to database
                                        if let Ok(saved_msg) = state_clone
                                            .db
                                            .add_message(room.id, &sender_name, &content)
                                            .await
                                        {
                                            // Broadcast to all clients
                                            let broadcast_msg = WsMessage::Chat {
                                                sender_name: saved_msg.sender_name,
                                                content: saved_msg.content,
                                                message_type: "text".to_string(),
                                                voice_url: None,
                                                created_at: saved_msg.created_at,
                                            };
                                            if let Ok(json) = serde_json::to_string(&broadcast_msg)
                                            {
                                                let _ = room_channel_clone.tx.send(json);
                                            }
                                        }
                                    }
                                }
                                WsMessage::Join { username } => {
                                    let system_msg = WsMessage::System {
                                        message: format!("{} has joined the room", username),
                                    };
                                    if let Ok(json) = serde_json::to_string(&system_msg) {
                                        let _ = room_channel_clone.tx.send(json);
                                    }
                                }
                                WsMessage::Leave { username } => {
                                    let system_msg = WsMessage::System {
                                        message: format!("{} has left the room", username),
                                    };
                                    if let Ok(json) = serde_json::to_string(&system_msg) {
                                        let _ = room_channel_clone.tx.send(json);
                                    }
                                }
                                WsMessage::Pong { timestamp: _ } => {
                                    // Client responded to heartbeat
                                    alive_clone.store(true, Ordering::SeqCst);
                                }
                                WsMessage::Ping { timestamp } => {
                                    // Client sent ping, respond with pong
                                    let pong_msg = WsMessage::Pong { timestamp };
                                    if let Ok(json) = serde_json::to_string(&pong_msg) {
                                        let _ = tx_recv.send(Message::Text(json.into())).await;
                                    }
                                    // Also mark as alive since client is active
                                    alive_clone.store(true, Ordering::SeqCst);
                                }
                                WsMessage::Typing { username, is_typing } => {
                                    // Broadcast typing indicator to all clients
                                    let typing_msg = WsMessage::Typing { username, is_typing };
                                    if let Ok(json) = serde_json::to_string(&typing_msg) {
                                        let _ = room_channel_clone.tx.send(json);
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    Message::Pong(_) => {
                        // WebSocket protocol-level pong received
                        alive_clone.store(true, Ordering::SeqCst);
                    }
                    Message::Ping(data) => {
                        // Respond to WebSocket protocol-level ping
                        let _ = tx_recv.send(Message::Pong(data)).await;
                    }
                    Message::Close(_) => {
                        break;
                    }
                    _ => {}
                },
                Err(_) => {
                    break;
                }
            }
        }
    });

    // Wait for any task to finish, then clean up all tasks
    tokio::select! {
        _ = (&mut send_task) => {
            broadcast_task.abort();
            heartbeat_task.abort();
            recv_task.abort();
        }
        _ = (&mut broadcast_task) => {
            send_task.abort();
            heartbeat_task.abort();
            recv_task.abort();
        }
        _ = (&mut heartbeat_task) => {
            send_task.abort();
            broadcast_task.abort();
            recv_task.abort();
        }
        _ = (&mut recv_task) => {
            send_task.abort();
            broadcast_task.abort();
            heartbeat_task.abort();
        }
    }

    // Decrement user count when connection ends
    room_channel.user_left();
    room_channel.broadcast_user_count();
}

/// Voice upload response
#[derive(Serialize)]
pub struct VoiceUploadResponse {
    pub success: bool,
    pub voice_url: String,
    pub message_id: i64,
}

/// Maximum voice file size (5MB)
const MAX_VOICE_FILE_SIZE: usize = 5 * 1024 * 1024;

/// API: Upload voice message
pub async fn upload_voice(
    State(state): State<AppState>,
    Path(room_key): Path<String>,
    mut multipart: Multipart,
) -> Result<Json<VoiceUploadResponse>, (StatusCode, String)> {
    let mut sender_name: Option<String> = None;
    let mut audio_data: Option<Vec<u8>> = None;
    let mut file_extension = "webm".to_string();

    // Parse multipart form data
    while let Some(field) = multipart.next_field().await.map_err(|e| {
        (StatusCode::BAD_REQUEST, format!("Failed to read multipart field: {}", e))
    })? {
        let name = field.name().unwrap_or("").to_string();
        
        match name.as_str() {
            "sender_name" => {
                sender_name = Some(field.text().await.map_err(|e| {
                    (StatusCode::BAD_REQUEST, format!("Failed to read sender_name: {}", e))
                })?);
            }
            "audio" => {
                // Get content type to determine file extension
                if let Some(content_type) = field.content_type() {
                    if content_type.contains("ogg") {
                        file_extension = "ogg".to_string();
                    } else if content_type.contains("webm") {
                        file_extension = "webm".to_string();
                    } else if content_type.contains("mp4") || content_type.contains("m4a") {
                        file_extension = "m4a".to_string();
                    }
                }
                
                let data = field.bytes().await.map_err(|e| {
                    (StatusCode::BAD_REQUEST, format!("Failed to read audio data: {}", e))
                })?;
                
                // Check file size
                if data.len() > MAX_VOICE_FILE_SIZE {
                    return Err((
                        StatusCode::PAYLOAD_TOO_LARGE,
                        "Voice file too large. Maximum size is 5MB".to_string(),
                    ));
                }
                
                audio_data = Some(data.to_vec());
            }
            _ => {}
        }
    }

    // Validate required fields
    let sender_name = sender_name.ok_or((
        StatusCode::BAD_REQUEST,
        "sender_name is required".to_string(),
    ))?;
    
    let audio_data = audio_data.ok_or((
        StatusCode::BAD_REQUEST,
        "audio file is required".to_string(),
    ))?;

    // Get room from database
    let room = state.db.get_room_by_key(&room_key).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Database error: {}", e)))?
        .ok_or((StatusCode::NOT_FOUND, "Room not found".to_string()))?;

    // Create uploads directory if it doesn't exist
    let uploads_dir = format!("uploads/voice/{}", room_key);
    tokio::fs::create_dir_all(&uploads_dir).await.map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to create uploads directory: {}", e))
    })?;

    // Generate unique filename with timestamp
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let filename = format!("{}.{}", timestamp, file_extension);
    let file_path = format!("{}/{}", uploads_dir, filename);
    let voice_url = format!("/uploads/voice/{}/{}", room_key, filename);

    // Write audio file
    let mut file = tokio::fs::File::create(&file_path).await.map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to create file: {}", e))
    })?;
    
    file.write_all(&audio_data).await.map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to write file: {}", e))
    })?;

    // Save voice message to database
    let saved_msg = state.db.add_voice_message(room.id, &sender_name, &voice_url).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to save message: {}", e)))?;

    // Broadcast voice message to all clients in the room
    let voice_msg = WsMessage::Voice {
        sender_name: saved_msg.sender_name.clone(),
        voice_url: voice_url.clone(),
        created_at: saved_msg.created_at.clone(),
    };
    
    if let Ok(json) = serde_json::to_string(&voice_msg) {
        state.room_manager.broadcast(&room_key, &json).await;
    }

    Ok(Json(VoiceUploadResponse {
        success: true,
        voice_url,
        message_id: saved_msg.id,
    }))
}

/// Delete message request
#[derive(Deserialize)]
pub struct DeleteMessageRequest {
    pub sender_name: String,
}

/// Delete message response
#[derive(Serialize)]
pub struct DeleteMessageResponse {
    pub success: bool,
    pub message: String,
}

/// API: Delete a message within 1 minute window
pub async fn delete_message(
    State(state): State<AppState>,
    Path((room_key, message_id)): Path<(String, i64)>,
    Json(payload): Json<DeleteMessageRequest>,
) -> Result<Json<DeleteMessageResponse>, (StatusCode, Json<DeleteMessageResponse>)> {
    // Verify room exists
    let _room = state.db.get_room_by_key(&room_key).await
        .map_err(|e| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(DeleteMessageResponse {
                success: false,
                message: format!("Database error: {}", e),
            })
        ))?
        .ok_or((
            StatusCode::NOT_FOUND,
            Json(DeleteMessageResponse {
                success: false,
                message: "Room not found".to_string(),
            })
        ))?;

    // Attempt to delete the message
    match state.db.delete_message(message_id, &payload.sender_name).await {
        Ok(true) => {
            // Broadcast message deletion to all clients in the room
            let delete_msg = WsMessage::MessageDeleted { message_id };
            if let Ok(json) = serde_json::to_string(&delete_msg) {
                state.room_manager.broadcast(&room_key, &json).await;
            }

            Ok(Json(DeleteMessageResponse {
                success: true,
                message: "Message deleted successfully".to_string(),
            }))
        }
        Ok(false) => {
            Err((
                StatusCode::NOT_FOUND,
                Json(DeleteMessageResponse {
                    success: false,
                    message: "Message not found".to_string(),
                })
            ))
        }
        Err(e) => {
            let status = if e == "message_delete_locked" {
                StatusCode::FORBIDDEN
            } else if e == "unauthorized" {
                StatusCode::UNAUTHORIZED
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            };

            Err((
                status,
                Json(DeleteMessageResponse {
                    success: false,
                    message: e,
                })
            ))
        }
    }
}
