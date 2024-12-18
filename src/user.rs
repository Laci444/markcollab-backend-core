use std::hash::Hash;

#[derive(Clone)]
pub struct User {
    name: String,
    nickname: String,
}

#[allow(dead_code)]
impl User {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            nickname: name.to_string(),
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
