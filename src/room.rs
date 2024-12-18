use std::{collections::HashSet, hash::Hash};

use tokio::sync::broadcast;
use tokio::sync::broadcast::error::SendError;
use tokio::sync::broadcast::Sender;

use crate::message::Message;
use crate::user::User;

#[derive(Clone)]
pub struct Room {
    name: String,
    users: HashSet<User>,
    sender: Sender<Message>,
}

#[allow(dead_code)]
impl Room {
    pub fn new(name: String) -> Self {
        Self {
            name,
            users: HashSet::default(),
            sender: broadcast::channel(usize::MAX).0,
        }
    }
    pub fn add_user(&mut self, mut user: User) {
        user.set_current_room_id(&self.name);
        self.users.insert(user);
    }
    pub fn remove_and_return_user(&mut self, user: &mut User) -> Option<User> {
        (*user).delete_current_room_id();
        self.users.take(user)
    }
    pub fn send_message(&self, message: Message) -> Result<usize, SendError<Message>> {
        self.sender.send(message)
    }
}

impl Hash for Room {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.name.hash(state);
    }
}

impl PartialEq for Room {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

impl Eq for Room {}
