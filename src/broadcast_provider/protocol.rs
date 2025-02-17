use std::io::Write;

use super::YObjectRef;
use smallvec::{smallvec, SmallVec};
use y_octo::{
    read_sync_message, write_sync_message, AwarenessStates, CrdtRead, DocMessage, JwstCodecError,
    RawDecoder, StateVector, SyncMessage, Update,
};

#[derive(Debug, Clone, PartialEq)]
pub enum MessageType {
    UpdateMessage(SyncMessage),
    QueryMessage(SyncMessage),
}

/// To be called whenever a new connection has been accepted. Returns a list of
/// messages to be sent back to initiator.
async fn start(yobject: &YObjectRef) -> Result<SmallVec<[SyncMessage; 1]>, JwstCodecError> {
    let object_lock = yobject.read().await;
    let (state_vector, update) = {
        let update = object_lock.awareness.get_states();
        let state_vector = object_lock.doc.encode_update_v1()?;
        (state_vector, update)
    };
    Ok(smallvec![
        SyncMessage::Doc(DocMessage::Step1(state_vector)),
        SyncMessage::Awareness(update.clone()),
    ])
}

/// Y-sync protocol message handler.
pub async fn handle(
    yobject: YObjectRef,
    data: &[u8],
) -> Result<SmallVec<[MessageType; 1]>, JwstCodecError> {
    let mut responses = SmallVec::default();
    let mut scanner = SyncMessageScanner::new(data); // TODO not sure if the scanner is needed
    while let Some(message) = scanner.next() {
        let message = message?;
        if is_message_update(&message) {
            responses.push(MessageType::UpdateMessage(message.clone()))
        }
        if let Some(message) = handle_message(yobject.clone(), message).await? {
            responses.push(MessageType::QueryMessage(message));
        }
    }
    Ok(responses)
}

fn is_message_update(message: &SyncMessage) -> bool {
    match message {
        SyncMessage::Doc(DocMessage::Step1(_)) | SyncMessage::AwarenessQuery => false,
        _ => true,
    }
}

/// Handles incoming y-sync [Message] within the context of current awareness structure.
/// Returns an optional reply message that should be sent back to message sender.
async fn handle_message(
    yobject: YObjectRef,
    message: SyncMessage,
) -> Result<Option<SyncMessage>, JwstCodecError> {
    match message {
        SyncMessage::Doc(DocMessage::Step1(raw_state_vector)) => {
            let state_vector = StateVector::read(&mut RawDecoder::new(raw_state_vector))?;
            handle_sync_step1(yobject, &state_vector).await
        }
        SyncMessage::Doc(DocMessage::Step2(update)) => {
            let update = Update::from_ybinary1(update)?;
            handle_sync_step2(yobject, update).await
        }
        SyncMessage::Doc(DocMessage::Update(update)) => {
            let update = Update::from_ybinary1(update)?;
            handle_update(yobject, update).await
        }
        SyncMessage::Auth(deny_reason) => handle_auth(yobject, deny_reason).await,
        SyncMessage::AwarenessQuery => handle_awareness_query(yobject).await,
        SyncMessage::Awareness(update) => handle_awareness_update(yobject, update).await,
    }
}

/// Y-sync protocol sync-step-1 - given a [StateVector] of a remote side, calculate missing
/// updates. Returns a sync-step-2 message containing a calculated update.
async fn handle_sync_step1(
    yobject: YObjectRef,
    sv: &StateVector,
) -> Result<Option<SyncMessage>, JwstCodecError> {
    let update = yobject.read().await.doc.encode_state_as_update_v1(sv)?;
    Ok(Some(SyncMessage::Doc(DocMessage::Step2(update))))
}

/// Handle reply for a sync-step-1 send from this replica previously. By default just apply
/// an update to current `awareness` document instance.
async fn handle_sync_step2(
    yobject: YObjectRef,
    update: Update,
) -> Result<Option<SyncMessage>, JwstCodecError> {
    yobject.write().await.doc.apply_update(update)?;
    Ok(None)
}

/// Handle continuous update send from the client. By default just apply an update to a current
/// `awareness` document instance.
async fn handle_update(
    yobject: YObjectRef,
    update: Update,
) -> Result<Option<SyncMessage>, JwstCodecError> {
    handle_sync_step2(yobject, update).await
}

/// Handle authorization message. By default if reason for auth denial has been provided,
/// send back [Error::PermissionDenied].
async fn handle_auth(
    _yobject: YObjectRef,
    deny_reason: Option<String>,
) -> Result<Option<SyncMessage>, JwstCodecError> {
    if let Some(reason) = deny_reason {
        Err(JwstCodecError::InvalidWriteBuffer(reason)) // TODO: THIS IS NOT OK
    } else {
        Ok(None)
    }
}

/// Returns an [AwarenessStates] which is a serializable representation of a current `awareness`
/// instance.
async fn handle_awareness_query(
    yobject: YObjectRef,
) -> Result<Option<SyncMessage>, JwstCodecError> {
    let lock = yobject.read().await;
    let update = lock.awareness.get_states();
    Ok(Some(SyncMessage::Awareness(update.clone())))
}

/// Reply to awareness query or just incoming [AwarenessUpdate], where current `awareness`
/// instance is being updated with incoming data.
async fn handle_awareness_update(
    yobject: YObjectRef,
    update: AwarenessStates,
) -> Result<Option<SyncMessage>, JwstCodecError> {
    yobject.write().await.awareness.apply_update(update);
    Ok(None)
}

pub async fn write<W: Write>(buffer: &mut W, messages: SmallVec<[SyncMessage; 1]>) {
    for message in messages {
        write_sync_message(buffer, &message);
    }
}

struct SyncMessageScanner<'a> {
    buffer: &'a [u8],
}

impl<'a> SyncMessageScanner<'a> {
    pub fn new(buffer: &'a [u8]) -> Self {
        Self { buffer }
    }
}

impl<'a> Iterator for SyncMessageScanner<'a> {
    type Item = Result<SyncMessage, JwstCodecError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.buffer.is_empty() {
            return None;
        }

        match read_sync_message(self.buffer) {
            Ok((tail, message)) => {
                self.buffer = tail;
                Some(Ok(message))
            }
            Err(_) => Some(Err(JwstCodecError::IncompleteDocument(String::from(
                "invalid buffer",
            )))), // TODO RENDES ERROROK KELLENEK
        }
    }
}
