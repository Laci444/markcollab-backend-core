use std::sync::Arc;
use tokio::sync::RwLock;
use yrs::sync::awareness::Awareness;

pub mod broadcast_new;
pub mod protocol_new;

pub type AwarenessRef = Arc<RwLock<Awareness>>;
