use std::sync::Arc;

use super::models::{CreateRoomRequest, UpdateRoomRequest, Room};
use super::storage::RoomStorage;

pub struct RoomManager {
    storage: Box<dyn RoomStorage>,
}

impl RoomManager {
    pub fn new(storage: Box<dyn RoomStorage>) -> Self {
        Self { storage }
    }

    pub async fn create_room(&self, req: CreateRoomRequest) -> Arc<Room> {
        self.storage.create_room(req).await
    }

    pub async fn get_room(&self, room_id: &str) -> Option<Arc<Room>> {
        self.storage.get_room(room_id).await
    }

    pub async fn list_rooms(&self) -> Vec<Arc<Room>> {
        self.storage.list_rooms().await
    }

    pub async fn update_room(&self, room_id: &str, req: UpdateRoomRequest) -> Option<Arc<Room>> {
        self.storage.update_room(room_id, req).await
    }

    pub async fn delete_room(&self, room_id: &str) -> bool {
        self.storage.delete_room(room_id).await
    }

    pub async fn room_exists(&self, room_id: &str) -> bool {
        self.storage.room_exists(room_id).await
    }

    pub async fn add_participant(&self, room_id: &str, user_id: String, username: String) -> bool {
        self.storage.add_participant(room_id, user_id, username).await
    }

    pub async fn remove_participant(&self, room_id: &str, user_id: &str) -> bool {
        self.storage.remove_participant(room_id, user_id).await
    }

    pub async fn cleanup_expired_rooms(&self) -> usize {
        let expired = self.storage.get_expired_rooms().await;
        self.storage.delete_rooms(expired).await
    }
}
