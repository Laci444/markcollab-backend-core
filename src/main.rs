mod broadcast_provider;
mod auth;
mod room;

use std::env;
use std::sync::Arc;
use futures::StreamExt;
use tracing::{debug, info, warn, instrument};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use warp::{filters::ws::WebSocket, Filter, Rejection};
use yrs_warp::ws::WarpSink;
use yrs_warp::ws::WarpStream;

use auth::{with_auth, with_mock_auth, handle_auth_rejection, UserInfo};
use room::{RoomManager, room_routes};
use broadcast_provider::broadcast::BroadcastGroup;
use crate::room::InMemoryRoomStorage;

#[tokio::main]
async fn main() {
    init_tracing();
    
    info!("Starting MarkCollab backend server");

    let room_storage = Box::new(InMemoryRoomStorage::new());
    let room_manager = Arc::new(RoomManager::new(room_storage));

    let api_routes = room_routes(room_manager.clone());

    // Use mock auth for development - switch to with_auth() when ready
    let auth_filter = if env::var("USE_MOCK_AUTH").is_ok() {
        info!("Using MOCK authentication (development only!)");
        with_mock_auth().boxed()
    } else {
        let jwt_secret = env::var("JWT_SECRET")
            .expect("JWT_SECRET environment variable is required when not using mock auth");
        info!("Using real JWT authentication");
        with_auth(jwt_secret).boxed()
    };

    let append_room_manager = warp::any().map(move || room_manager.clone());

    let ws_path = warp::path!("ws" / String)
        .and(warp::ws())
        .and(auth_filter)
        .and(append_room_manager)
        .and_then(
            |room_id: String,
             ws: warp::ws::Ws,
             user_info: UserInfo,
             room_manager: Arc<RoomManager>| async move {
                let bcast = room_manager
                    .get_room(&room_id)
                    .await
                    .map(|room| room.broadcast_group.clone())
                    .ok_or(warp::reject::not_found())?;

                debug!(
                    room_id = %room_id,
                    user_id = %user_info.user_id,
                    username = %user_info.username,
                    "WebSocket upgrade requested"
                );

                Ok::<_, Rejection>(ws.on_upgrade(move |socket|
                    handle_user(room_id, socket, user_info, bcast))
                )
            },
        );

    let response_headers = warp::reply::with::header("Sec-WebSocket-Protocol", "markcollab-v1");
    let tracing_filter = warp::trace::request();

    let routes = api_routes
        .or(ws_path)
        .with(response_headers)
        .with(tracing_filter)
        .recover(handle_auth_rejection);
    
    info!("Server listening on 0.0.0.0:3030");
    warp::serve(routes).run(([0, 0, 0, 0], 3030)).await;
}

#[instrument(skip(ws, bcast), fields(room_id = %room_id, user_id = %user_info.user_id, username = %user_info.username))]
async fn handle_user(room_id: String, ws: WebSocket, user_info: UserInfo, bcast: Arc<BroadcastGroup>) {
    info!("Authenticated user connected");
    let (sink, stream) = ws.split();

    let yrs_sink = WarpSink::from(sink);
    let yrs_stream = WarpStream::from(stream);

    let sub = bcast.subscribe(yrs_sink, yrs_stream);
    info!("User subscribed to room");

    match sub.completed().await {
        Ok(_) => info!("User disconnected gracefully"),
        Err(e) => warn!(error = %e, "User disconnected with error"),
    }
}

fn init_tracing() {
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| {
            tracing_subscriber::EnvFilter::new("info,markcollab_backend_core=debug,warp=info")
        });

    let formatting_layer = tracing_subscriber::fmt::layer()
        .with_target(true)
        .with_thread_ids(true)
        .with_level(true)
        .with_file(true)
        .with_line_number(true)
        .pretty();

    tracing_subscriber::registry()
        .with(env_filter)
        .with(formatting_layer)
        .init();

    info!("Tracing initialized");
}