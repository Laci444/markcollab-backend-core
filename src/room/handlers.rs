use std::sync::Arc;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use tracing::info;

use super::manager::RoomManager;
use super::models::{CreateRoomRequest, UpdateRoomRequest, RoomResponse};

pub async fn handle_create_room(
    State(room_manager): State<Arc<RoomManager>>,
    Json(req): Json<CreateRoomRequest>,
) -> impl IntoResponse {
    let room = room_manager.create_room(req).await;
    info!(room_id = %room.id, "Room created");
    Json(RoomResponse::from_room(&room))
}

pub async fn handle_get_room(
    State(room_manager): State<Arc<RoomManager>>,
    Path(room_id): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    match room_manager.get_room(&room_id).await {
        Some(room) => {
            Ok(Json(RoomResponse::from_room(&room)))
        }
        None => Err(StatusCode::NOT_FOUND),
    }
}

pub async fn handle_list_rooms(
    State(room_manager): State<Arc<RoomManager>>,
) -> impl IntoResponse {
    let rooms = room_manager.list_rooms().await;
    let responses: Vec<RoomResponse> = rooms.iter().map(|r| RoomResponse::from_room(r)).collect();
    Json(responses)
}

pub async fn handle_update_room(
    State(room_manager): State<Arc<RoomManager>>,
    Path(room_id): Path<String>,
    Json(req): Json<UpdateRoomRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    match room_manager.update_room(&room_id, req).await {
        Some(room) => {
            info!(room_id = %room.id, "Room updated");
            Ok(Json(RoomResponse::from_room(&room)))
        }
        None => Err(StatusCode::NOT_FOUND),
    }
}

pub async fn handle_delete_room(
    State(room_manager): State<Arc<RoomManager>>,
    Path(room_id): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    if room_manager.delete_room(&room_id).await {
        info!(room_id = %room_id, "Room deleted");
        Ok((
            StatusCode::OK,
            Json(serde_json::json!({"message": "Room deleted"})),
        ))
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

pub async fn handle_cleanup_expired_rooms(
    State(room_manager): State<Arc<RoomManager>>,
) -> impl IntoResponse {
    let count = room_manager.cleanup_expired_rooms().await;
    info!(count = %count, "Expired rooms cleaned up");
    Json(serde_json::json!({
        "message": "Cleanup completed",
        "rooms_removed": count
    }))
}
