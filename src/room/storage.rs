use super::models::{CreateRoomRequest, Room};
use crate::broadcast_provider::broadcast::BroadcastGroup;
use async_trait::async_trait;
use uuid::Uuid;

/// Trait for room storage implementations
/// This allows for different storage backends (in-memory, database, etc.)
#[async_trait]
pub trait RoomStorage: Send + Sync {
    /// Create a new room
    async fn create_room(&self, req: CreateRoomRequest, broadcast_group: BroadcastGroup) -> Room;

    /// Get a room by ID
    async fn get_room(&self, room_id: Uuid) -> Option<Room>;

    /// List all rooms
    async fn list_rooms(&self) -> Vec<Room>;

    /// Delete a room
    async fn delete_room(&self, room_id: Uuid) -> bool;

    /// Check if a room exists
    async fn room_exists(&self, room_id: Uuid) -> bool;

    /// Increase editor count by 1
    async fn increase_editor_count(&self, room_id: Uuid);

    /// Decrease editor count by 1
    async fn decrease_editor_count(&self, room_id: Uuid);

    /// Get editor count
    async fn get_editor_count(&self, room_id: Uuid) -> Option<u64>;

    /// Get rooms that are empty and have been empty for longer than their retention period
    async fn get_expired_rooms(&self) -> Vec<Uuid>;

    /// Delete multiple rooms by IDs
    async fn delete_rooms(&self, room_ids: Vec<Uuid>) -> usize;
}
