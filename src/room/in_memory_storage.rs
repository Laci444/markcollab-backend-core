use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;
use async_trait::async_trait;

use super::models::{Room, CreateRoomRequest, UpdateRoomRequest, Participant};
use super::storage::RoomStorage;
use crate::broadcast_provider::broadcast::BroadcastGroup;

pub struct InMemoryRoomStorage {
    rooms: Arc<RwLock<HashMap<String, Arc<Room>>>>,
}

impl InMemoryRoomStorage {
    pub fn new() -> Self {
        Self {
            rooms: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    fn current_timestamp() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }
}

#[async_trait]
impl RoomStorage for InMemoryRoomStorage {
    async fn create_room(&self, req: CreateRoomRequest) -> Arc<Room> {
        let room_id = Uuid::new_v4().to_string();
        let now = Self::current_timestamp();

        let room = Room {
            id: room_id.clone(),
            name: req.name.unwrap_or_else(|| format!("Room {}", &room_id[..8])),
            broadcast_group: Arc::new(BroadcastGroup::new(100).await),
            created_at: now,
            last_accessed: now,
            retention_hours: req.retention_hours.unwrap_or(24),
            participants: HashMap::new(),
            empty_since: Some(now),
        };

        let room_arc = Arc::new(room);
        let mut rooms = self.rooms.write().await;
        rooms.insert(room_id, room_arc.clone());
        room_arc
    }

    async fn get_room(&self, room_id: &str) -> Option<Arc<Room>> {
        let rooms = self.rooms.read().await;
        rooms.get(room_id).cloned()
    }

    async fn list_rooms(&self) -> Vec<Arc<Room>> {
        let rooms = self.rooms.read().await;
        rooms.values().cloned().collect()
    }

    async fn update_room(&self, room_id: &str, req: UpdateRoomRequest) -> Option<Arc<Room>> {
        let mut rooms = self.rooms.write().await;
        if let Some(old_room) = rooms.get(room_id) {
            let now = Self::current_timestamp();

            let updated_room = Room {
                id: old_room.id.clone(),
                name: req.name.unwrap_or_else(|| old_room.name.clone()),
                broadcast_group: old_room.broadcast_group.clone(),
                created_at: old_room.created_at,
                last_accessed: now,
                retention_hours: req.retention_hours.unwrap_or(old_room.retention_hours),
                participants: old_room.participants.clone(),
                empty_since: old_room.empty_since,
            };

            let room_arc = Arc::new(updated_room);
            rooms.insert(room_id.to_string(), room_arc.clone());
            Some(room_arc)
        } else {
            None
        }
    }

    async fn delete_room(&self, room_id: &str) -> bool {
        let mut rooms = self.rooms.write().await;
        rooms.remove(room_id).is_some()
    }

    async fn room_exists(&self, room_id: &str) -> bool {
        let rooms = self.rooms.read().await;
        rooms.contains_key(room_id)
    }

    async fn add_participant(&self, room_id: &str, user_id: String, username: String) -> bool {
        let mut rooms = self.rooms.write().await;
        if let Some(old_room) = rooms.get(room_id) {
            let now = Self::current_timestamp();

            let mut participants = old_room.participants.clone();
            participants.insert(user_id.clone(), Participant {
                user_id,
                username,
                joined_at: now,
            });

            let updated_room = Room {
                id: old_room.id.clone(),
                name: old_room.name.clone(),
                broadcast_group: old_room.broadcast_group.clone(),
                created_at: old_room.created_at,
                last_accessed: now,
                retention_hours: old_room.retention_hours,
                participants,
                empty_since: None,  // Room is no longer empty
            };

            rooms.insert(room_id.to_string(), Arc::new(updated_room));
            tracing::debug!(room_id = %room_id, "Participant added to room");
            true
        } else {
            false
        }
    }

    async fn remove_participant(&self, room_id: &str, user_id: &str) -> bool {
        let mut rooms = self.rooms.write().await;
        if let Some(old_room) = rooms.get(room_id) {
            let now = Self::current_timestamp();

            let mut participants = old_room.participants.clone();
            participants.remove(user_id);

            let empty_since = if participants.is_empty() {
                Some(now)
            } else {
                None
            };

            let updated_room = Room {
                id: old_room.id.clone(),
                name: old_room.name.clone(),
                broadcast_group: old_room.broadcast_group.clone(),
                created_at: old_room.created_at,
                last_accessed: now,
                retention_hours: old_room.retention_hours,
                participants,
                empty_since,
            };

            rooms.insert(room_id.to_string(), Arc::new(updated_room));

            if empty_since.is_some() {
                tracing::info!(room_id = %room_id, "Room became empty");
            } else {
                tracing::debug!(room_id = %room_id, "Participant removed from room");
            }

            true
        } else {
            false
        }
    }

    async fn get_expired_rooms(&self) -> Vec<String> {
        let now = Self::current_timestamp();
        let rooms = self.rooms.read().await;

        rooms
            .iter()
            .filter(|(_, room)| {
                // Only consider rooms that are empty
                if let Some(empty_since) = room.empty_since {
                    let retention_seconds = room.retention_hours * 3600;
                    now - empty_since > retention_seconds
                } else {
                    false
                }
            })
            .map(|(id, _)| id.clone())
            .collect()
    }

    async fn delete_rooms(&self, room_ids: Vec<String>) -> usize {
        let mut rooms = self.rooms.write().await;
        let mut count = 0;

        for room_id in room_ids {
            if rooms.remove(&room_id).is_some() {
                count += 1;
                tracing::info!(room_id = %room_id, "Room expired and removed");
            }
        }

        count
    }
}
