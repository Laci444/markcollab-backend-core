mod auth;
mod broadcast_provider;
mod error;
mod kafka_recorder;
mod room;

use crate::auth::{CurrentUser, User};
use crate::room::InMemoryRoomStorage;
use axum::extract::Query;
use axum::{
    extract::{ws::WebSocket, Path, State, WebSocketUpgrade},
    response::IntoResponse,
    routing::any,
    Router,
};
use broadcast_provider::{
    adapters::{AxumSink, AxumStream, ProtocolSink, ProtocolStream},
    broadcast::BroadcastGroup,
};
use futures_util::StreamExt;
use rdkafka::ClientConfig;
use room::{room_routes, RoomManager};
use std::sync::Arc;
use tower::ServiceBuilder;
use tower_http::trace::TraceLayer;
use tracing::{debug, error, info, instrument, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use uuid::Uuid;

#[tokio::main]
async fn main() {
    init_tracing();

    info!("Starting MarkCollab backend server");

    let kafka_producer = ClientConfig::new()
        .set("bootstrap.servers", "192.168.0.10:9092")
        .set("message.timeout.ms", "5000")
        .create()
        .expect("Producer creation error");

    let room_storage = Box::new(InMemoryRoomStorage::new().await);
    let room_manager = Arc::new(RoomManager::new(room_storage, kafka_producer));

    let api_routes = room_routes(room_manager.clone());

    // WebSocket route with custom state
    let ws_route = Router::new()
        .route("/ws/{room_id}", any(ws_handler))
        .with_state(room_manager.clone());

    // Combine all routes
    let app = Router::new()
        .merge(api_routes)
        .merge(ws_route)
        .layer(ServiceBuilder::new().layer(TraceLayer::new_for_http()));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3030")
        .await
        .expect("Failed to bind to port 3030");

    info!("Server listening on 0.0.0.0:3030");

    axum::serve(listener, app).await.expect("Server failed");
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    Path(room_id): Path<Uuid>,
    State(room_manager): State<Arc<RoomManager>>,
    Query(user): Query<User>,
) -> impl IntoResponse {
    let user_name = user.user_name;
    if !room_manager.room_exists(room_id).await {
        error!(
        room_id = %room_id,
        //user_id = %user.id,
        username = %user_name,
        "Room not found!"
        );
        return (http::StatusCode::NOT_FOUND, "Room not found").into_response();
    }

    debug!(
        room_id = %room_id,
        //user_id = %user.id,
        username = %user_name,
        "WebSocket upgrade requested"
    );

    let room_manager_clone = room_manager.clone();
    ws.protocols(["markcollab-v1"])
        .on_upgrade(move |socket| async move {
            room_manager_clone
                .handle_user(room_id, socket, user_name)
                .await
        })
}

fn init_tracing() {
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        tracing_subscriber::EnvFilter::new(
            "info,markcollab_backend_core=debug,axum=info,tower_http=info",
        )
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
