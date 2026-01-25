mod broadcast_provider;
mod auth;
mod room;

use axum::{
    extract::ws::WebSocket,
    extract::{Path, State, WebSocketUpgrade},
    response::IntoResponse,
    routing::get,
    Router,
};
use futures_util::StreamExt;
use std::env;
use std::sync::Arc;
use tower::ServiceBuilder;
use tower_http::trace::TraceLayer;
use tracing::{debug, info, instrument, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::room::InMemoryRoomStorage;
use auth::{AuthState, UserInfo};
use broadcast_provider::adapters::{AxumSink, AxumStream, ProtocolSink, ProtocolStream};
use broadcast_provider::broadcast::BroadcastGroup;
use room::{room_routes, RoomManager};

#[tokio::main]
async fn main() {
    init_tracing();

    info!("Starting MarkCollab backend server");

    let room_storage = Box::new(InMemoryRoomStorage::new());
    let room_manager = Arc::new(RoomManager::new(room_storage));

    let api_routes = room_routes(room_manager.clone());

    // Use mock auth for development - switch to JWT when ready
    let auth_state = if env::var("USE_MOCK_AUTH").is_ok() {
        info!("Using MOCK authentication (development only!)");
        AuthState::mock()
    } else {
        let jwt_secret = env::var("JWT_SECRET")
            .expect("JWT_SECRET environment variable is required when not using mock auth");
        info!("Using real JWT authentication");
        AuthState::jwt(jwt_secret)
    };

    // WebSocket route with custom state
    let ws_route = Router::new()
        .route("/ws/:room_id", get(ws_handler))
        .layer(axum::middleware::from_fn_with_state(
            auth_state.clone(),
            auth_middleware,
        ))
        .with_state(room_manager.clone());

    // Combine all routes
    let app = Router::new()
        .merge(api_routes)
        .merge(ws_route)
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
        );

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3030")
        .await
        .expect("Failed to bind to port 3030");

    info!("Server listening on 0.0.0.0:3030");

    axum::serve(listener, app)
        .await
        .expect("Server failed");
}

// Middleware to extract and inject UserInfo
async fn auth_middleware(
    State(auth_state): State<AuthState>,
    mut request: axum::http::Request<axum::body::Body>,
    next: axum::middleware::Next,
) -> Result<axum::response::Response, axum::http::StatusCode> {
    let (mut parts, body) = request.into_parts();

    let user_info = auth_state.extract_user_info(&parts).await
        .map_err(|_| axum::http::StatusCode::UNAUTHORIZED)?;

    parts.extensions.insert(user_info);
    request = axum::http::Request::from_parts(parts, body);

    Ok(next.run(request).await)
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    Path(room_id): Path<String>,
    State(room_manager): State<Arc<RoomManager>>,
    axum::Extension(user_info): axum::Extension<UserInfo>,
) -> impl IntoResponse {
    let bcast = match room_manager.get_room(&room_id).await {
        Some(room) => room.broadcast_group.clone(),
        None => {
            return (axum::http::StatusCode::NOT_FOUND, "Room not found").into_response();
        }
    };

    debug!(
        room_id = %room_id,
        user_id = %user_info.user_id,
        username = %user_info.username,
        "WebSocket upgrade requested"
    );

    ws.protocols(["markcollab-v1"])
        .on_upgrade(move |socket| handle_user(room_id, socket, user_info, bcast))
}

#[instrument(skip(ws, bcast), fields(room_id = %room_id, user_id = %user_info.user_id, username = %user_info.username
))]
async fn handle_user(room_id: String, ws: WebSocket, user_info: UserInfo, bcast: Arc<BroadcastGroup>) {
    info!(
        room_id = %room_id,
        user_id = %user_info.user_id,
        username = %user_info.username,
        "Authenticated user connected"
    );

    let (sink, stream) = ws.split();

    let axum_sink = AxumSink { inner: sink };
    let axum_stream = AxumStream { inner: stream };

    let protocol_sink = ProtocolSink::new(axum_sink);
    let protocol_stream = ProtocolStream::new(axum_stream);

    let sub = bcast.subscribe(protocol_sink, protocol_stream);
    info!("User subscribed to room with protocol adapters");

    match sub.completed().await {
        Ok(_) => info!("User disconnected gracefully"),
        Err(e) => warn!(error = %e, "User disconnected with error"),
    }
}

fn init_tracing() {
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| {
            tracing_subscriber::EnvFilter::new("info,markcollab_backend_core=debug,axum=info,tower_http=info")
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