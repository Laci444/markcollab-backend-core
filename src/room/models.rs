use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use crate::broadcast_provider::broadcast::BroadcastGroup;

#[derive(Clone)]
pub struct Participant {
    pub user_id: String,
    pub username: String,
    pub joined_at: u64,
}

pub struct Room {
    pub id: String,
    pub name: String,
    pub broadcast_group: Arc<BroadcastGroup>,
    pub created_at: u64,
    pub last_accessed: u64,
    pub retention_hours: u64,
    pub participants: HashMap<String, Participant>,
    pub empty_since: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateRoomRequest {
    pub name: Option<String>,
    pub retention_hours: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateRoomRequest {
    pub name: Option<String>,
    pub retention_hours: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RoomResponse {
    pub id: String,
    pub name: String,
    pub url: String,
    pub created_at: u64,
    pub last_accessed: u64,
    pub retention_hours: u64,
    pub participant_count: usize,
    pub empty_since: Option<u64>,
}

impl RoomResponse {
    pub fn from_room(room: &Room) -> Self {
        RoomResponse {
            url: format!("/ws/{}", room.id),
            id: room.id.clone(),
            name: room.name.clone(),
            created_at: room.created_at,
            last_accessed: room.last_accessed,
            retention_hours: room.retention_hours,
            participant_count: room.participants.len(),
            empty_since: room.empty_since,
        }
    }
}
