mod broadcast;
mod protocol;

use std::sync::Arc;

use thiserror::Error;
use tokio::sync::RwLock;
use y_octo::{Awareness, Doc};

struct YObject {
    doc: Doc,
    awareness: Awareness,
}

impl YObject {
    pub fn new(doc: Doc, awareness: Awareness) -> Self {
        Self { doc, awareness }
    }
}

impl Default for YObject {
    fn default() -> Self {
        let doc = Doc::default();
        Self {
            doc: doc.clone(),
            awareness: Awareness::new(doc.client()),
        }
    }
}

impl From<Doc> for YObject {
    fn from(doc: Doc) -> Self {
        Self {
            doc: doc.clone(),
            awareness: Awareness::new(doc.client()),
        }
    }
}

type YObjectRef = Arc<RwLock<YObject>>;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Failed to parse message")]
    ParseError,
    #[error("")]
    Other,
}
