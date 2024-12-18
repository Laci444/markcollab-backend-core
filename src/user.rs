use std::hash::Hash;

use tokio::sync::broadcast::Receiver;

use crate::message::Message;

#[derive(Clone)]
pub struct User {
    name: String,
    nickname: String,
    receiver: Option<Receiver<Message>>,
    current_room_id: Option<String>,
}

#[allow(dead_code)]
impl User {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            nickname: name.to_string(),
            receiver: None,
            current_room_id: None,
        }
    }
    pub fn set_nickname(&mut self, nickname: &str) {
        self.nickname = format!("^{nickname}");
    }
    pub fn get_nickname(&self) -> &str {
        &self.nickname
    }
    pub fn get_name(&self) -> &str {
        &self.name
    }
    pub fn get_current_room_id(&self) -> Option<&str> {
        self.current_room_id.as_deref()
    }
    pub fn set_current_room_id(&mut self, room_id: &str) {
        self.current_room_id = Some(room_id.to_string());
    }
    pub fn delete_current_room_id(&mut self) {
        self.current_room_id = None;
    }
}

impl Hash for User {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.name.hash(state);
    }
}

impl PartialEq for User {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

impl Eq for User {}
