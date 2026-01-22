use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use super::models::{Room, CreateRoomRequest, UpdateRoomRequest};
use crate::broadcast_provider::broadcast::BroadcastGroup;

pub struct RoomManager {
    rooms: Arc<RwLock<HashMap<String, Arc<Room>>>>,
}

impl RoomManager {
    pub fn new() -> Self {
        Self {
            rooms: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn create_room(&self, req: CreateRoomRequest) -> Arc<Room> {
        let room_id = Uuid::new_v4().to_string();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let room = Room {
            id: room_id.clone(),
            name: req.name.unwrap_or_else(|| format!("Room {}", &room_id[..8])),
            broadcast_group: Arc::new(BroadcastGroup::new(100).await),
            created_at: now,
            last_accessed: now,
            retention_hours: req.retention_hours.unwrap_or(24),
        };

        let room_arc = Arc::new(room);
        let mut rooms = self.rooms.write().await;
        rooms.insert(room_id, room_arc.clone());
        room_arc
    }

    pub async fn get_room(&self, room_id: &str) -> Option<Arc<Room>> {
        let rooms = self.rooms.read().await;
        rooms.get(room_id).cloned()
    }

    pub async fn get_broadcast_group(&self, room_id: &str) -> Option<Arc<BroadcastGroup>> {
        let rooms = self.rooms.read().await;
        rooms.get(room_id).map(|room| room.broadcast_group.clone())
    }

    pub async fn list_rooms(&self) -> Vec<Arc<Room>> {
        let rooms = self.rooms.read().await;
        rooms.values().cloned().collect()
    }

    pub async fn update_room(&self, room_id: &str, req: UpdateRoomRequest) -> Option<Arc<Room>> {
        let mut rooms = self.rooms.write().await;
        if let Some(old_room) = rooms.get(room_id) {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs();

            let updated_room = Room {
                id: old_room.id.clone(),
                name: req.name.unwrap_or_else(|| old_room.name.clone()),
                broadcast_group: old_room.broadcast_group.clone(),
                created_at: old_room.created_at,
                last_accessed: now,
                retention_hours: req.retention_hours.unwrap_or(old_room.retention_hours),
            };

            let room_arc = Arc::new(updated_room);
            rooms.insert(room_id.to_string(), room_arc.clone());
            Some(room_arc)
        } else {
            None
        }
    }

    pub async fn delete_room(&self, room_id: &str) -> bool {
        let mut rooms = self.rooms.write().await;
        rooms.remove(room_id).is_some()
    }

    pub async fn room_exists(&self, room_id: &str) -> bool {
        let rooms = self.rooms.read().await;
        rooms.contains_key(room_id)
    }

    pub async fn update_last_accessed(&self, room_id: &str) {
        let mut rooms = self.rooms.write().await;
        if let Some(old_room) = rooms.get(room_id) {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs();

            let updated_room = Room {
                id: old_room.id.clone(),
                name: old_room.name.clone(),
                broadcast_group: old_room.broadcast_group.clone(),
                created_at: old_room.created_at,
                last_accessed: now,
                retention_hours: old_room.retention_hours,
            };

            rooms.insert(room_id.to_string(), Arc::new(updated_room));
        }
    }

    pub async fn cleanup_expired_rooms(&self) -> usize {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let mut rooms = self.rooms.write().await;
        let expired: Vec<String> = rooms
            .iter()
            .filter(|(_, room)| {
                let retention_seconds = room.retention_hours * 3600;
                now - room.last_accessed > retention_seconds
            })
            .map(|(id, _)| id.clone())
            .collect();

        let count = expired.len();
        for room_id in expired {
            rooms.remove(&room_id);
            tracing::info!(room_id = %room_id, "Room expired and removed");
        }

        count
    }
}
