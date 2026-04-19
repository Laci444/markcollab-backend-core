use super::models::{CreateRoomRequest, Room};
use super::storage::RoomStorage;
use crate::broadcast_provider::broadcast::BroadcastGroup;
use async_trait::async_trait;
use dashmap::DashMap;
use std::str::FromStr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

pub struct InMemoryRoomStorage {
    rooms: DashMap<Uuid, Room>,
}

fn current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}
impl InMemoryRoomStorage {
    pub async fn new() -> Self {
        let manager = Self {
            rooms: DashMap::new(),
        };
        let room_id = Uuid::from_str("99999999-9999-9999-9999-999999999999").unwrap();
        manager.rooms.insert(
            room_id,
            Room {
                id: room_id,
                broadcast_group: Arc::new(BroadcastGroup::default(100).await),
                retention_seconds: 60 * 5,
                number_of_editors: 0,
                empty_since: None,
            },
        );
        manager
    }
}

#[async_trait]
impl RoomStorage for InMemoryRoomStorage {
    async fn create_room(&self, req: CreateRoomRequest, broadcast_group: BroadcastGroup) -> Room {
        let room = Room {
            id: req.id,
            broadcast_group: Arc::new(broadcast_group),
            retention_seconds: req.retention_seconds.unwrap_or(60 * 5),
            number_of_editors: 0,
            empty_since: None,
        };

        self.rooms.insert(req.id, room.clone());
        room
    }

    async fn get_room(&self, room_id: Uuid) -> Option<Room> {
        self.rooms.get(&room_id).map(|room| room.clone())
    }

    async fn list_rooms(&self) -> Vec<Room> {
        self.rooms.iter().map(|room| room.clone()).collect()
    }

    async fn delete_room(&self, room_id: Uuid) -> bool {
        self.rooms.remove(&room_id).is_some()
    }

    async fn room_exists(&self, room_id: Uuid) -> bool {
        self.rooms.contains_key(&room_id)
    }

    async fn increase_editor_count(&self, room_id: Uuid) {
        self.rooms.get_mut(&room_id).unwrap().number_of_editors += 1;
    }

    async fn decrease_editor_count(&self, room_id: Uuid) {
        let room = &mut self.rooms.get_mut(&room_id).unwrap();
        room.number_of_editors = room.number_of_editors.saturating_sub(1);
        if room.number_of_editors == 0 {
            room.empty_since = Option::from(current_timestamp());
        }
    }

    async fn get_editor_count(&self, room_id: Uuid) -> Option<u64> {
        self.rooms.get(&room_id).map(|room| room.number_of_editors)
    }

    async fn get_expired_rooms(&self) -> Vec<Uuid> {
        let now = current_timestamp();

        self.rooms
            .iter()
            .filter(|room| {
                // Only consider rooms that are empty
                if let Some(empty_since) = room.empty_since {
                    now - empty_since > room.retention_seconds
                } else {
                    false
                }
            })
            .map(|room| room.id.clone())
            .collect()
    }

    async fn delete_rooms(&self, room_ids: Vec<Uuid>) -> usize {
        let mut count = 0;

        for room_id in room_ids {
            if self.rooms.remove(&room_id).is_some() {
                count += 1;
                tracing::info!(room_id = %room_id, "Room expired and removed");
            }
        }

        count
    }
}
