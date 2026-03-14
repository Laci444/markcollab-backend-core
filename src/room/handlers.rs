use super::manager::RoomManager;
use super::models::{CreateRoomRequest, Room};
use axum::extract::Path;
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde_json::json;
use std::sync::Arc;
use tracing::info;
use uuid::Uuid;

pub async fn handle_create_room(
    State(room_manager): State<Arc<RoomManager>>,
    Json(req): Json<CreateRoomRequest>,
) -> impl IntoResponse {
    let room = room_manager.create_room(req).await;
    info!(room_id = %room.id, "Room created");
    Json(json!(room))
}

pub async fn handle_get_room(
    State(room_manager): State<Arc<RoomManager>>,
    Path(room_id): Path<Uuid>,
) -> Result<impl IntoResponse, StatusCode> {
    match room_manager.get_room(room_id).await {
        Some(room) => Ok(Json(json!(room))),
        None => Err(StatusCode::NOT_FOUND),
    }
}

pub async fn handle_list_rooms(State(room_manager): State<Arc<RoomManager>>) -> impl IntoResponse {
    let rooms = room_manager.list_rooms().await;
    Json(json!(rooms))
}

pub async fn handle_delete_room(
    State(room_manager): State<Arc<RoomManager>>,
    Path(room_id): Path<Uuid>,
) -> Result<impl IntoResponse, StatusCode> {
    if room_manager.delete_room(room_id).await {
        info!(room_id = %room_id, "Room deleted");
        Ok((StatusCode::OK, Json(json!({"message": "Room deleted"}))))
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

pub async fn handle_cleanup_expired_rooms(
    State(room_manager): State<Arc<RoomManager>>,
) -> impl IntoResponse {
    let count = room_manager.cleanup_expired_rooms().await;
    info!(count = %count, "Expired rooms cleaned up");
    Json(json!({
        "message": "Cleanup completed",
        "rooms_removed": count
    }))
}
