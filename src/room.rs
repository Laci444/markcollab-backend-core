use std::{collections::HashSet, hash::Hash};

use crate::user::User;

#[derive(Clone)]
pub struct Room {
    name: String,
    users: HashSet<User>,
}

impl Room {
    pub fn new(name: String) -> Self {
        Self {
            name,
            users: HashSet::default(),
        }
    }
    pub fn add_user(&mut self, user: User) {
        self.users.insert(user);
    }
    pub fn remove_and_return_user(&mut self, user: &User) -> Option<User> {
        self.users.take(user)
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
