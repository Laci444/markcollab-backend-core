use std::{collections::HashMap, sync::Arc};
use tokio::sync::RwLock;

use crate::{room::Room, user::User};

#[derive(Clone)]
pub struct Rooms {
    rooms: Arc<RwLock<HashMap<String, Room>>>,
}

#[allow(dead_code)]
impl Rooms {
    pub fn new() -> Self {
        Rooms {
            rooms: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    /*
    async fn list_rooms(&self) -> Result<Vec<Room>, &str> {
        Ok(self.rooms.read().await.deref().values().collect())
    }
    */
    pub async fn create_room(&self, room_id: &str) -> Result<(), &str> {
        match self
            .rooms
            .write()
            .await
            .insert(room_id.to_string(), Room::new(room_id.to_string()))
        {
            // TODO: bad behaviour
            Some(_) => Err("Room already existed. Replaced"),
            None => Ok(()),
        }
    }
    pub async fn delete_room(&self, room_id: &str) -> Result<(), &str> {
        match self.rooms.write().await.remove(room_id) {
            Some(_) => Ok(()),
            None => Err("Room not found"),
        }
    }
    pub async fn add_user(&self, room_id: &str, user: User) -> Result<(), &str> {
        match self.rooms.write().await.get_mut(room_id) {
            Some(room) => {
                room.add_user(user);
                Ok(())
            }
            None => Err("Room not found"),
        }
    }
    pub async fn remove_and_return_user(
        &self,
        room_id: &str,
        user: &mut User,
    ) -> Result<Option<User>, &str> {
        match self.rooms.write().await.get_mut(room_id) {
            Some(room) => Ok(room.remove_and_return_user(user)),
            None => Err("Room not found"),
        }
    }
    pub async fn move_user(&self, user: &mut User, from: &str, to: &str) -> Result<(), &str> {
        match self.remove_and_return_user(from, user).await? {
            Some(user) => self.add_user(to, user).await,
            None => Err("User not found"),
        }
    }
}
