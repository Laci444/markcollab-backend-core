use std::sync::Arc;
use axum::{
    routing::{get, post},
    Router,
};

use super::manager::RoomManager;
use super::handlers::{
    handle_create_room, handle_get_room, handle_list_rooms,
    handle_update_room, handle_delete_room, handle_cleanup_expired_rooms,
};

pub fn room_routes(room_manager: Arc<RoomManager>) -> Router {
    Router::new()
        .route("/api/rooms", post(handle_create_room).get(handle_list_rooms))
        .route("/api/rooms/:room_id",
            get(handle_get_room)
                .put(handle_update_room)
                .delete(handle_delete_room)
        )
        .route("/api/rooms/cleanup", post(handle_cleanup_expired_rooms))
        .with_state(room_manager)
}
