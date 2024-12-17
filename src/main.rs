use database::Rooms;
use futures::{SinkExt, StreamExt};
use log::info;
use user::User;
use warp::{
    filters::ws::{Message, WebSocket},
    Filter,
};

mod database;
mod room;
mod user;

#[tokio::main]
async fn main() {
    pretty_env_logger::init();
    let logging = warp::log("ACCESS_LOG");
    let database = Rooms::new();

    let _ = database.create_room("example").await;
    info!("Room example created");

    let append_database = warp::any().map(move || database.clone());

    let ws_path = warp::path!("ws" / String)
        .and(warp::ws())
        .and(warp::header::<String>("user-name"))
        .and(append_database)
        .map(
            |path: String, ws: warp::ws::Ws, username: String, database: Rooms| {
                ws.on_upgrade(|socket| handle_user(path, socket, username, database))
            },
        );

    let routes = ws_path.with(logging);
    warp::serve(routes).run(([127, 0, 0, 1], 3030)).await;
}

async fn handle_user(room_id: String, ws: WebSocket, username: String, database: Rooms) {
    info!("Handling user {username}!");
    let user = User::new(&username);

    let (mut sink, mut stream) = ws.split();

    // TODO: handle the errors
    database
        .add_user(&room_id, user.clone())
        .await
        .expect("Error connecting user to room {room_id}");
    info!("User {} added to room {room_id}", user.get_nickname());

    sink.send(Message::text(format!("You are in room {room_id}")))
        .await
        .expect("Error sending message to user");

    while let Some(incoming) = stream.next().await {
        match incoming {
            Ok(msg) => info!("Got message from user {}: {:#?}", user.get_nickname(), msg),
            Err(err) => eprintln!("Socket error with user {}: {}", user.get_name(), err),
        }
    }
}
