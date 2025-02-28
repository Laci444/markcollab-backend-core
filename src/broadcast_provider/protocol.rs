use std::io::Write;

use smallvec::{smallvec, SmallVec};
use y_octo::{
    read_sync_message, write_sync_message, AwarenessStates, CrdtRead, DocMessage, JwstCodecError,
    RawDecoder, StateVector, SyncMessage, Update,
};

use super::{YObject, YObjectRef};

#[derive(Debug, Clone, PartialEq)]
pub enum MessageType {
    UpdateMessage(SyncMessage),
    QueryMessage(SyncMessage),
}

pub struct MarkcollabProtocol;

impl AsyncKafkaProtocol for MarkcollabProtocol {}

pub trait AsyncKafkaProtocol {
    /// To be called whenever a new connection has been accepted. Returns a list of
    /// messages to be sent back to initiator.
    fn start(&self, yobject: YObject) -> Result<SmallVec<[SyncMessage; 1]>, JwstCodecError> {
        let (state_vector, update) = {
            let update = yobject.awareness.get_states();
            let state_vector = yobject.doc.encode_update_v1()?;
            (state_vector, update)
        };
        Ok(smallvec![
            SyncMessage::Doc(DocMessage::Step1(state_vector)),
            SyncMessage::Awareness(update.clone()),
        ])
    }

    /// Y-sync protocol message handler.
    fn handle(
        &self,
        yobject: YObjectRef,
        data: &[u8],
    ) -> Result<SmallVec<[MessageType; 1]>, JwstCodecError> {
        let mut responses = SmallVec::default();
        let scanner = SyncMessageScanner::new(data); // TODO not sure if the scanner is needed
        for message in scanner {
            let message = message?;
            if self.is_message_update(&message) {
                responses.push(MessageType::UpdateMessage(message.clone()))
            }
            if let Some(message) = self.handle_message(yobject.clone(), message)? {
                responses.push(MessageType::QueryMessage(message));
            }
        }
        Ok(responses)
    }

    fn is_message_update(&self, message: &SyncMessage) -> bool {
        !matches!(
            message,
            SyncMessage::Doc(DocMessage::Step1(_)) | SyncMessage::AwarenessQuery
        )
    }

    /// Handles incoming y-sync [Message] within the context of current awareness structure.
    /// Returns an optional reply message that should be sent back to message sender.
    fn handle_message(
        &self,
        yobject: YObjectRef,
        message: SyncMessage,
    ) -> Result<Option<SyncMessage>, JwstCodecError> {
        match message {
            SyncMessage::Doc(DocMessage::Step1(raw_state_vector)) => {
                let state_vector = StateVector::read(&mut RawDecoder::new(raw_state_vector))?;
                self.handle_sync_step1(yobject, &state_vector)
            }
            SyncMessage::Doc(DocMessage::Step2(update)) => {
                let update = Update::from_ybinary1(update)?;
                self.handle_sync_step2(yobject, update)
            }
            SyncMessage::Doc(DocMessage::Update(update)) => {
                let update = Update::from_ybinary1(update)?;
                self.handle_update(yobject, update)
            }
            SyncMessage::Auth(deny_reason) => self.handle_auth(yobject, deny_reason),
            SyncMessage::AwarenessQuery => self.handle_awareness_query(yobject),
            SyncMessage::Awareness(update) => self.handle_awareness_update(yobject, update),
        }
    }

    /// Y-sync protocol sync-step-1 - given a [StateVector] of a remote side, calculate missing
    /// updates. Returns a sync-step-2 message containing a calculated update.
    fn handle_sync_step1(
        &self,
        yobject: YObjectRef,
        sv: &StateVector,
    ) -> Result<Option<SyncMessage>, JwstCodecError> {
        let update = yobject.read().unwrap().doc.encode_state_as_update_v1(sv)?;
        Ok(Some(SyncMessage::Doc(DocMessage::Step2(update))))
    }

    /// Handle reply for a sync-step-1 send from this replica previously. By default just apply
    /// an update to current `awareness` document instance.
    fn handle_sync_step2(
        &self,
        yobject: YObjectRef,
        update: Update,
    ) -> Result<Option<SyncMessage>, JwstCodecError> {
        yobject.write().unwrap().doc.apply_update(update)?;
        Ok(None)
    }

    /// Handle continuous update send from the client. By default just apply an update to a current
    /// `awareness` document instance.
    fn handle_update(
        &self,
        yobject: YObjectRef,
        update: Update,
    ) -> Result<Option<SyncMessage>, JwstCodecError> {
        self.handle_sync_step2(yobject, update)
    }

    /// Handle authorization message. By default if reason for auth denial has been provided,
    /// send back [Error::PermissionDenied].
    fn handle_auth(
        &self,
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
    fn handle_awareness_query(
        &self,
        yobject: YObjectRef,
    ) -> Result<Option<SyncMessage>, JwstCodecError> {
        let lock = yobject.read().unwrap();
        let update = lock.awareness.get_states();
        Ok(Some(SyncMessage::Awareness(update.clone())))
    }

    /// Reply to awareness query or just incoming [AwarenessUpdate], where current `awareness`
    /// instance is being updated with incoming data.
    fn handle_awareness_update(
        &self,
        yobject: YObjectRef,
        update: AwarenessStates,
    ) -> Result<Option<SyncMessage>, JwstCodecError> {
        yobject.write().unwrap().awareness.apply_update(update);
        Ok(None)
    }

    async fn write<W: Write>(&self, buffer: &mut W, messages: SmallVec<[SyncMessage; 1]>) {
        for message in messages {
            let _ = write_sync_message(buffer, &message);
        }
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

impl Iterator for SyncMessageScanner<'_> {
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
