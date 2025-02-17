mod broadcast_provider;

use std::collections::HashMap;
use std::sync::Arc;

use futures::StreamExt;
use log::debug;
use log::info;
use log::warn;
use tokio::sync::{Mutex, RwLock};
use warp::{filters::ws::WebSocket, Filter};
use yrs::{sync::Awareness, Doc, Text, Transact};
use yrs_warp::ws::WarpSink;
use yrs_warp::ws::WarpStream;
use yrs_warp::{broadcast::BroadcastGroup, AwarenessRef};

//use broadcast_provider::BroadcastGroup;

#[tokio::main]
async fn main() {
    pretty_env_logger::init();
    let logging = warp::log("ACCESS_LOG");

    let awareness: AwarenessRef = {
        let doc = Doc::new();
        {
            let txt = doc.get_or_insert_text("example-room");
            let mut txn = doc.transact_mut();
            txt.push(
                &mut txn,
                r#"function hello() {
  console.log('hello world');
}"#,
            );
        }
        Arc::new(RwLock::new(Awareness::new(doc)))
    };

    let bcast = Arc::new(BroadcastGroup::new(awareness, 32).await);
    let append_broadcast_group = warp::any().map(move || bcast.clone());

    let ws_path = warp::path!("ws" / String)
        .and(warp::ws())
        .and(warp::filters::query::query::<HashMap<String, String>>())
        .and(append_broadcast_group)
        .map(
            |path: String,
             ws: warp::ws::Ws,
             request_query: HashMap<String, String>,
             bcast: Arc<BroadcastGroup>| {
                let username = request_query.get("user-name").unwrap().to_owned();
                ws.on_upgrade(|socket| handle_user(path, socket, username, bcast))
            },
        );

    let response_headers = warp::reply::with::header("Sec-WebSocket-Protocol", "markcollab-v1");
    let routes = ws_path.with(response_headers).with(logging);
    warp::serve(routes).run(([127, 0, 0, 1], 3030)).await;
}

async fn handle_user(room_id: String, ws: WebSocket, username: String, bcast: Arc<BroadcastGroup>) {
    debug!("New user connected: {username}!");
    let (sink, stream) = ws.split();

    let yrs_sink = Arc::new(Mutex::new(WarpSink::from(sink)));
    let yrs_stream = WarpStream::from(stream);

    let sub = bcast.subscribe(yrs_sink, yrs_stream);

    info!("User {} subscribed to room {room_id}", &username);

    match sub.completed().await {
        Ok(_) => info!("User {} disconnected", &username),
        Err(e) => warn!("User {} error: {}", &username, e),
    }
}
