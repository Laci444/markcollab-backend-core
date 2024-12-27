use database::Rooms;
use futures::{SinkExt, StreamExt};
use log::debug;
use log::error;
use log::info;
use message::ParsedMessage;
use warp::{
    filters::ws::{Message, WebSocket},
    Filter,
};

mod database;
mod message;
mod room;
mod user;

#[tokio::main]
async fn main() {
    pretty_env_logger::init();
    let logging = warp::log("ACCESS_LOG");
    let database = Rooms::new();

    let _ = database.create_room("example").await;
    let _ = database.create_room("topic").await;
    debug!("Room example created");
    debug!("Room topic created");

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

    let response_headers = warp::reply::with::header("Sec-WebSocket-Protocol", "markcollab-v1");
    let routes = ws_path.with(response_headers).with(logging);
    warp::serve(routes).run(([127, 0, 0, 1], 3030)).await;
}

async fn handle_user(room_id: String, ws: WebSocket, username: String, database: Rooms) {
    debug!("New user connected: {username}!");
    let (mut sink, mut stream) = ws.split();

    // TODO: handle the errors
    let mut topic = database
        .add_new_user(&room_id, &username)
        .await
        .expect("Error connecting user to room {room_id}");
    info!("User {} connected to room {room_id}", &username);

    sink.send(Message::text(format!("You are in room {room_id}")))
        .await
        .expect("Error sending message to user");

    // send out messages received from subscribed topic
    tokio::spawn(async move {
        while let Ok(msg) = topic.recv().await {
            match sink
                .send(Message::text(serde_json::to_string(&msg).unwrap()))
                .await
            {
                Ok(_) => (),
                Err(_) => break,
            }
        }
    });

    // listen for messages from client
    while let Some(incoming) = stream.next().await {
        let msg = match incoming {
            Ok(msg) => msg,
            Err(err) => {
                error!("Socket error with user {}: {}", &username, err);
                break;
            }
        };
        let _ = handle_message(&database, &username, msg, &room_id).await;
    }

    handle_disconnect(&username, database).await;
}

async fn handle_message(database: &Rooms, username: &str, msg: Message, room_id: &str) {
    if !msg.is_text() {
        debug!(
            "Got non-text message in room {room_id} from user {}: {:#?} ",
            &username, msg
        );
        return;
    }
    debug!(
        "Got message in room {room_id} from user {}: {:#?} ",
        &username, msg
    );

    let message: ParsedMessage = match serde_json::from_str(msg.to_str().unwrap()) {
        Ok(pmsg) => pmsg,
        Err(_) => {
            debug!("Received message is not a valid ParsedMessage type");
            return;
        }
    };


    database
        .write_to_room(room_id, msg.to_str().unwrap().to_owned())
        .await
        .unwrap();
}

async fn handle_disconnect(username: &str, database: Rooms) {
    info!("User {username} has disconnected");
    database.purge_user(username).await;
}
