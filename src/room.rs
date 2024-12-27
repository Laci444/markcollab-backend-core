use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::hash::Hash;

use log::debug;
use tokio::sync::broadcast::error::SendError;
use tokio::sync::broadcast::Sender;
use tokio::sync::broadcast::{self, Receiver};

use crate::user::User;

pub struct Room {
    name: String,
    users: HashMap<String, User>,
    sender: Sender<String>,
}

#[allow(dead_code)]
impl Room {
    pub fn new(name: String) -> Self {
        Self {
            name,
            users: HashMap::default(),
            sender: broadcast::channel(100).0,
        }
    }
    pub fn subscribe(&self) -> Receiver<String> {
        self.sender.subscribe()
    }
    pub fn get_user(&self, username: &str) -> Result<&User, &str> {
        match self.users.get(username) {
            Some(user) => Ok(user),
            None => Err("User not found"),
        }
    }
    pub fn add_user(&mut self, mut user: User) -> Result<&mut User, &str> {
        debug!(
            "{}",
            match self.users.entry(user.get_name().to_owned()) {
                Entry::Occupied(_) => "In database",
                Entry::Vacant(_) => "Not in database",
            }
        );
        match self.users.entry(user.get_name().to_owned()) {
            Entry::Occupied(_) => Err("User already in database"),
            Entry::Vacant(entry) => {
                user.set_current_room_id(&self.name);
                Ok(entry.insert(user))
            }
        }
    }
    pub fn remove_and_return_user(&mut self, username: &str) -> Option<User> {
        self.users
            .get_mut(username)
            .unwrap()
            .delete_current_room_id();
        self.users.remove(username)
    }
    pub fn send_message(&self, message: String) -> Result<usize, SendError<String>> {
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
