use std::sync::Arc;
use warp::Filter;

use super::manager::RoomManager;
use super::handlers::{
    handle_create_room, handle_get_room, handle_list_rooms,
    handle_update_room, handle_delete_room, handle_cleanup_expired_rooms,
};

fn with_room_manager(
    room_manager: Arc<RoomManager>,
) -> impl Filter<Extract = (Arc<RoomManager>,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || room_manager.clone())
}

pub fn room_routes(
    room_manager: Arc<RoomManager>,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    let create_room = warp::path!("api" / "rooms")
        .and(warp::post())
        .and(warp::body::json())
        .and(with_room_manager(room_manager.clone()))
        .and_then(handle_create_room);

    let get_room = warp::path!("api" / "rooms" / String)
        .and(warp::get())
        .and(with_room_manager(room_manager.clone()))
        .and_then(handle_get_room);

    let list_rooms = warp::path!("api" / "rooms")
        .and(warp::get())
        .and(with_room_manager(room_manager.clone()))
        .and_then(handle_list_rooms);

    let update_room = warp::path!("api" / "rooms" / String)
        .and(warp::put())
        .and(warp::body::json())
        .and(with_room_manager(room_manager.clone()))
        .and_then(handle_update_room);

    let delete_room = warp::path!("api" / "rooms" / String)
        .and(warp::delete())
        .and(with_room_manager(room_manager.clone()))
        .and_then(handle_delete_room);

    let cleanup_rooms = warp::path!("api" / "rooms" / "cleanup")
        .and(warp::post())
        .and(with_room_manager(room_manager.clone()))
        .and_then(handle_cleanup_expired_rooms);

    create_room
        .or(get_room)
        .or(list_rooms)
        .or(update_room)
        .or(delete_room)
        .or(cleanup_rooms)
}
