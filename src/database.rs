use log::error;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::{
    broadcast::{error::SendError, Receiver},
    RwLock,
};

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
    pub async fn add_new_user(
        &self,
        default_room_id: &str,
        username: &str,
    ) -> Result<Receiver<String>, &str> {
        let mut lock = self.rooms.write().await;
        let topic = lock.get(default_room_id).unwrap().subscribe();
        let new_user = User::new(username);
        match lock.get_mut(default_room_id) {
            Some(room) => {
                room.add_user(new_user).expect("User already in database");
                Ok(topic)
            }
            None => Err("Room not found"),
        }
    }
    pub async fn add_user_to_room(&self, room_id: &str, user: User) -> Result<(), &str> {
        match self.rooms.write().await.get_mut(room_id) {
            Some(room) => {
                room.add_user(user).expect("User already in room");
                Ok(())
            }
            None => Err("Room not found"),
        }
    }
    pub async fn remove_and_return_user(
        &self,
        room_id: &str,
        username: &str,
    ) -> Result<Option<User>, &str> {
        match self.rooms.write().await.get_mut(room_id) {
            Some(room) => Ok(room.remove_and_return_user(username)),
            None => Err("Room not found"),
        }
    }
    pub async fn move_user(&self, username: &str, from: &str, to: &str) -> Result<(), &str> {
        match self.remove_and_return_user(from, username).await? {
            Some(user) => self.add_user_to_room(to, user).await,
            None => Err("User not found"),
        }
    }
    pub async fn write_to_room(
        &self,
        room_id: &str,
        message: String,
    ) -> Result<usize, SendError<String>> {
        self.rooms
            .write()
            .await
            .get(room_id)
            .unwrap()
            .send_message(message)
    }
    pub async fn purge_user(&self, _username: &str) {
        error!("purge_user is not implemented")
    }
}
