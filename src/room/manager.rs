use super::models::{CreateRoomRequest, Room};
use super::storage::RoomStorage;
use crate::broadcast_provider::adapters::{AxumSink, AxumStream, ProtocolSink, ProtocolStream};
use crate::broadcast_provider::broadcast::BroadcastGroup;
use crate::kafka_recorder::KafkaRecorder;
use axum::extract::ws::WebSocket;
use futures_util::StreamExt;
use rdkafka::producer::FutureProducer;
use std::sync::Arc;
use tracing::{info, warn};
use uuid::Uuid;

pub struct RoomManager {
    storage: Box<dyn RoomStorage>,
    kafka_producer: FutureProducer,
    topic_name: String,
}

impl RoomManager {
    pub fn new(
        storage: Box<dyn RoomStorage>,
        kafka_producer: FutureProducer,
        topic_name: String,
    ) -> Self {
        Self {
            storage,
            kafka_producer,
            topic_name,
        }
    }

    pub async fn create_room(&self, req: CreateRoomRequest) -> Room {
        let broadcast_group = match req.initial_state_url.clone() {
            Some(url) => {
                let text = reqwest::get(url).await.unwrap().text().await.unwrap();
                BroadcastGroup::new(100, &text).await
            }
            None => BroadcastGroup::default(100).await,
        };

        let room = self.storage.create_room(req, broadcast_group).await;

        let kafka_rx = room.broadcast_group.subscribe_observer();

        let kafka_recorder = KafkaRecorder::new(
            room.id,
            self.kafka_producer.clone(),
            kafka_rx,
            self.topic_name.clone(),
        );

        tokio::spawn(kafka_recorder.run());

        room
    }

    pub async fn get_room(&self, room_id: Uuid) -> Option<Room> {
        self.storage.get_room(room_id).await
    }

    pub async fn list_rooms(&self) -> Vec<Room> {
        self.storage.list_rooms().await
    }

    pub async fn delete_room(&self, room_id: Uuid) -> bool {
        self.storage.delete_room(room_id).await
    }

    pub async fn room_exists(&self, room_id: Uuid) -> bool {
        self.storage.room_exists(room_id).await
    }

    pub async fn increase_editor_count(&self, room_id: Uuid) {
        self.storage.increase_editor_count(room_id).await
    }

    pub async fn decrease_editor_count(&self, room_id: Uuid) {
        self.storage.decrease_editor_count(room_id).await;
    }

    pub async fn get_editor_count(&self, room_id: Uuid) -> Option<u64> {
        self.storage.get_editor_count(room_id).await
    }

    pub async fn cleanup_expired_rooms(&self) -> usize {
        let expired = self.storage.get_expired_rooms().await;
        self.storage.delete_rooms(expired).await
    }

    pub async fn handle_user(&self, room_id: Uuid, ws: WebSocket, user_name: String) {
        info!(
            room_id = %room_id,
            username = %user_name,
            "Authenticated user connected"
        );

        let (sink, stream) = ws.split();

        let axum_sink = AxumSink { inner: sink };
        let axum_stream = AxumStream { inner: stream };

        let protocol_sink = ProtocolSink::new(axum_sink);
        let protocol_stream = ProtocolStream::new(axum_stream);

        let sub = self
            .get_room(room_id)
            .await
            .unwrap()
            .broadcast_group
            .subscribe(protocol_sink, protocol_stream);
        self.increase_editor_count(room_id).await;
        info!("User subscribed to room with protocol adapters");

        match sub.completed().await {
            Ok(_) => info!("User disconnected gracefully"),
            Err(e) => warn!(error = %e, "User disconnected with error"),
        }
        self.decrease_editor_count(room_id).await;
    }
}
