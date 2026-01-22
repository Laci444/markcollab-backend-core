use std::sync::Arc;
use async_trait::async_trait;

use super::models::{Room, CreateRoomRequest, UpdateRoomRequest};

/// Trait for room storage implementations
/// This allows for different storage backends (in-memory, database, etc.)
#[async_trait]
pub trait RoomStorage: Send + Sync {
    /// Create a new room
    async fn create_room(&self, req: CreateRoomRequest) -> Arc<Room>;

    /// Get a room by ID
    async fn get_room(&self, room_id: &str) -> Option<Arc<Room>>;

    /// List all rooms
    async fn list_rooms(&self) -> Vec<Arc<Room>>;

    /// Update a room
    async fn update_room(&self, room_id: &str, req: UpdateRoomRequest) -> Option<Arc<Room>>;

    /// Delete a room
    async fn delete_room(&self, room_id: &str) -> bool;

    /// Check if a room exists
    async fn room_exists(&self, room_id: &str) -> bool;

    /// Add a participant to a room
    async fn add_participant(&self, room_id: &str, user_id: String, username: String) -> bool;

    /// Remove a participant from a room
    async fn remove_participant(&self, room_id: &str, user_id: &str) -> bool;

    /// Get rooms that are empty and have been empty for longer than their retention period
    async fn get_expired_rooms(&self) -> Vec<String>;

    /// Delete multiple rooms by IDs
    async fn delete_rooms(&self, room_ids: Vec<String>) -> usize;
}
