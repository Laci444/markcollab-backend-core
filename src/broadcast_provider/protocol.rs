use std::io::Write;

use smallvec::{smallvec, SmallVec};
use tracing::{debug, error, info, instrument, trace, warn};
use y_octo::{
    write_sync_message, AwarenessStates, CrdtRead, DocMessage, JwstCodecError,
    RawDecoder, StateVector, SyncMessage, Update,
};

use super::{YObject, YObjectRef};

pub struct MarkcollabProtocol;

impl AsyncKafkaProtocol for MarkcollabProtocol {}

pub trait AsyncKafkaProtocol {
    #[instrument(skip(self, yobject))]
    fn start(&self, yobject: YObject) -> Result<SmallVec<[SyncMessage; 1]>, JwstCodecError> {
        debug!("Starting protocol handshake");

        let (state_vector, update) = {
            let update = yobject.awareness.get_states();
            let state_vector = match yobject.doc.encode_update_v1() {
                Ok(sv) => sv,
                Err(e) => {
                    error!(error = %e, "Failed to encode state vector");
                    return Err(e);
                }
            };
            (state_vector, update)
        };

        debug!(
            state_vector_size = state_vector.len(),
            awareness_states_count = update.len(),
            "Protocol start messages prepared"
        );

        Ok(smallvec![
            SyncMessage::Doc(DocMessage::Step1(state_vector)),
            SyncMessage::Awareness(update.clone()),
        ])
    }

    #[instrument(skip(self, yobject), fields(message_type = ?message))]
    fn handle(
        &self,
        yobject: YObjectRef,
        message: SyncMessage,
    ) -> Result<Option<SyncMessage>, JwstCodecError> {
        match message {
            SyncMessage::Doc(DocMessage::Step1(raw_state_vector)) => {
                debug!(state_vector_size = raw_state_vector.len(), "Handling sync step 1");
                let state_vector = StateVector::read(&mut RawDecoder::new(raw_state_vector))?;
                self.handle_sync_step1(yobject, &state_vector)
            }
            SyncMessage::Doc(DocMessage::Step2(update)) => {
                debug!(update_size = update.len(), "Handling sync step 2");
                let update = Update::from_ybinary1(update)?;
                self.handle_sync_step2(yobject, update)
            }
            SyncMessage::Doc(DocMessage::Update(update)) => {
                debug!(update_size = update.len(), "Handling document update");
                let update = Update::from_ybinary1(update)?;
                self.handle_update(yobject, update)
            }
            SyncMessage::Auth(deny_reason) => {
                debug!(has_deny_reason = deny_reason.is_some(), "Handling auth message");
                self.handle_auth(yobject, deny_reason)
            }
            SyncMessage::AwarenessQuery => {
                debug!("Handling awareness query");
                self.handle_awareness_query(yobject)
            }
            SyncMessage::Awareness(update) => {
                debug!(awareness_states_count = update.len(), "Handling awareness update");
                self.handle_awareness_update(yobject, update)
            }
        }
    }

    #[instrument(skip(self, yobject, sv))]
    fn handle_sync_step1(
        &self,
        yobject: YObjectRef,
        sv: &StateVector,
    ) -> Result<Option<SyncMessage>, JwstCodecError> {
        let update = match yobject.read().unwrap().doc.encode_state_as_update_v1(sv) {
            Ok(update) => {
                debug!(update_size = update.len(), "Generated sync step 1 response");
                update
            }
            Err(e) => {
                error!(error = %e, "Failed to encode state as update");
                return Err(e);
            }
        };

        Ok(Some(SyncMessage::Doc(DocMessage::Step2(update))))
    }

    #[instrument(skip(self, yobject, update))]
    fn handle_sync_step2(
        &self,
        yobject: YObjectRef,
        update: Update,
    ) -> Result<Option<SyncMessage>, JwstCodecError> {
        match yobject.write().unwrap().doc.apply_update(update) {
            Ok(_) => {
                debug!("Successfully applied sync step 2 update");
                Ok(None)
            }
            Err(e) => {
                error!(error = %e, "Failed to apply sync step 2 update");
                Err(e)
            }
        }
    }

    #[instrument(skip(self, yobject, update))]
    fn handle_update(
        &self,
        yobject: YObjectRef,
        update: Update,
    ) -> Result<Option<SyncMessage>, JwstCodecError> {
        debug!("Handling document update");
        self.handle_sync_step2(yobject, update)
    }

    #[instrument(skip(self, _yobject))]
    fn handle_auth(
        &self,
        _yobject: YObjectRef,
        deny_reason: Option<String>,
    ) -> Result<Option<SyncMessage>, JwstCodecError> {
        if let Some(reason) = deny_reason {
            warn!(deny_reason = %reason, "Authentication denied");
            Err(JwstCodecError::InvalidWriteBuffer(reason)) // TODO: THIS IS NOT OK
        } else {
            info!("Authentication successful");
            Ok(None)
        }
    }

    #[instrument(skip(self, yobject))]
    fn handle_awareness_query(
        &self,
        yobject: YObjectRef,
    ) -> Result<Option<SyncMessage>, JwstCodecError> {
        let lock = yobject.read().unwrap();
        let update = lock.awareness.get_states();

        debug!(awareness_states_count = update.len(), "Responding to awareness query");
        Ok(Some(SyncMessage::Awareness(update.clone())))
    }

    #[instrument(skip(self, yobject, update), fields(awareness_states_count = update.len()))]
    fn handle_awareness_update(
        &self,
        yobject: YObjectRef,
        update: AwarenessStates,
    ) -> Result<Option<SyncMessage>, JwstCodecError> {
        yobject.write().unwrap().awareness.apply_update(update);
        debug!("Applied awareness update");
        Ok(None)
    }

    #[instrument(skip(self, buffer, messages), fields(message_count = messages.len()))]
    async fn write<W: Write>(&self, buffer: &mut W, messages: SmallVec<[SyncMessage; 1]>) {
        for (idx, message) in messages.iter().enumerate() {
            match write_sync_message(buffer, message) {
                Ok(_) => {
                    trace!(message_idx = idx, "Successfully wrote sync message");
                }
                Err(e) => {
                    error!(error = %e, message_idx = idx, "Failed to write sync message");
                }
            }
        }
        debug!("Completed writing all messages to buffer");
    }
}