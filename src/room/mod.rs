mod handlers;
mod in_memory_storage;
mod manager;
mod models;
mod storage;

use crate::room::handlers::{
    handle_cleanup_expired_rooms, handle_create_room, handle_delete_room, handle_get_room,
    handle_list_rooms,
};
use axum::{
    routing::{get, post},
    Router,
};
pub use in_memory_storage::InMemoryRoomStorage;
pub use manager::RoomManager;
use std::sync::Arc;

pub fn room_routes(room_manager: Arc<RoomManager>) -> Router {
    Router::new().nest(
        "/v1",
        Router::new()
            .route("/rooms", post(handle_create_room).get(handle_list_rooms))
            .route(
                "/rooms/{room_id}",
                get(handle_get_room).delete(handle_delete_room),
            )
            .route("/rooms/cleanup", post(handle_cleanup_expired_rooms))
            .with_state(room_manager),
    )
}
