use crate::broadcast_provider::broadcast::BroadcastGroup;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Serialize, Clone)]
pub struct Room {
    pub id: Uuid,
    #[serde(skip)]
    pub broadcast_group: Arc<BroadcastGroup>,
    pub retention_seconds: u64,
    pub number_of_editors: u64,
    pub empty_since: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateRoomRequest {
    pub name: Option<String>,
    pub id: Uuid,
    pub retention_seconds: Option<u64>,
    pub initial_state_url: Option<String>,
}
