use askama::Template;

/// Index page template
#[derive(Template)]
#[template(path = "index.html")]
pub struct IndexTemplate;

/// Create room page template
#[derive(Template)]
#[template(path = "create.html")]
pub struct CreateRoomTemplate;

/// Room page template
#[derive(Template)]
#[template(path = "room.html")]
pub struct RoomTemplate {
    pub room_key: String,
    pub username: String,
}
