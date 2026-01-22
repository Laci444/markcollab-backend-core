use std::sync::Arc;
use warp;
use tracing::info;

use super::manager::RoomManager;
use super::models::{CreateRoomRequest, UpdateRoomRequest, RoomResponse};

pub async fn handle_create_room(
    req: CreateRoomRequest,
    room_manager: Arc<RoomManager>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let room = room_manager.create_room(req).await;
    info!(room_id = %room.id, "Room created");
    Ok(warp::reply::json(&RoomResponse::from_room(&room)))
}

pub async fn handle_get_room(
    room_id: String,
    room_manager: Arc<RoomManager>,
) -> Result<impl warp::Reply, warp::Rejection> {
    match room_manager.get_room(&room_id).await {
        Some(room) => {
            //room_manager.update_last_accessed(&room_id).await;
            Ok(warp::reply::json(&RoomResponse::from_room(&room)))
        }
        None => Err(warp::reject::not_found()),
    }
}

pub async fn handle_list_rooms(
    room_manager: Arc<RoomManager>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let rooms = room_manager.list_rooms().await;
    let responses: Vec<RoomResponse> = rooms.iter().map(|r| RoomResponse::from_room(r)).collect();
    Ok(warp::reply::json(&responses))
}

pub async fn handle_update_room(
    room_id: String,
    req: UpdateRoomRequest,
    room_manager: Arc<RoomManager>,
) -> Result<impl warp::Reply, warp::Rejection> {
    match room_manager.update_room(&room_id, req).await {
        Some(room) => {
            info!(room_id = %room.id, "Room updated");
            Ok(warp::reply::json(&RoomResponse::from_room(&room)))
        }
        None => Err(warp::reject::not_found()),
    }
}

pub async fn handle_delete_room(
    room_id: String,
    room_manager: Arc<RoomManager>,
) -> Result<impl warp::Reply, warp::Rejection> {
    if room_manager.delete_room(&room_id).await {
        info!(room_id = %room_id, "Room deleted");
        Ok(warp::reply::with_status(
            warp::reply::json(&serde_json::json!({"message": "Room deleted"})),
            warp::http::StatusCode::OK,
        ))
    } else {
        Err(warp::reject::not_found())
    }
}

pub async fn handle_cleanup_expired_rooms(
    room_manager: Arc<RoomManager>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let count = room_manager.cleanup_expired_rooms().await;
    info!(count = %count, "Expired rooms cleaned up");
    Ok(warp::reply::json(&serde_json::json!({
        "message": "Cleanup completed",
        "rooms_removed": count
    })))
}
