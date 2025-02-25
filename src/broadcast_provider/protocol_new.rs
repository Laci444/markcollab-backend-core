use async_trait::async_trait;
use smallvec::SmallVec;
use yrs::{
    encoding::read::Cursor,
    sync::{protocol::AsyncProtocol, Awareness, AwarenessUpdate, Error, Message, MessageReader},
    updates::decoder::DecoderV1,
    Update,
};

pub struct AsyncKafkaProtocol;

#[async_trait]
impl AsyncProtocol for AsyncKafkaProtocol {
    /// Y-sync protocol message handler.
    async fn handle(
        &self,
        awareness: &Awareness,
        data: &[u8],
    ) -> Result<SmallVec<[Message; 1]>, Error> {
        let mut decoder = DecoderV1::new(Cursor::new(data));
        let mut reader = MessageReader::new(&mut decoder);
        let mut responses = SmallVec::new();
        while let Some(result) = reader.next() {
            let message = result?;
            if let Some(response) = self.handle_message(awareness, message).await? {
                responses.push(response);
            }
        }
        Ok(responses)
    }

    /// Handle reply for a sync-step-1 send from this replica previously. By default just apply
    /// an update to current `awareness` document instance.
    async fn handle_sync_step2(
        &self,
        awareness: &Awareness,
        update: Update,
    ) -> Result<Option<Message>, Error> {
        use yrs::AsyncTransact;
        let mut txn = awareness.doc().transact_mut().await;
        txn.apply_update(update)?;
        Ok(None)
    }

    /// Handle authorization message. By default if reason for auth denial has been provided,
    /// send back [Error::PermissionDenied].
    async fn handle_auth(
        &self,
        _awareness: &Awareness,
        deny_reason: Option<String>,
    ) -> Result<Option<Message>, Error> {
        if let Some(reason) = deny_reason {
            Err(Error::PermissionDenied { reason })
        } else {
            Ok(None)
        }
    }

    /// Reply to awareness query or just incoming [AwarenessUpdate], where current `awareness`
    /// instance is being updated with incoming data.
    async fn handle_awareness_update(
        &self,
        awareness: &Awareness,
        update: AwarenessUpdate,
    ) -> Result<Option<Message>, Error> {
        awareness.apply_update(update)?;
        Ok(None)
    }

    /// Y-sync protocol enables to extend its own settings with custom handles. These can be
    /// implemented here. By default it returns an [Error::Unsupported].
    async fn missing_handle(
        &self,
        _awareness: &Awareness,
        tag: u8,
        _data: Vec<u8>,
    ) -> Result<Option<Message>, Error> {
        Err(Error::Unsupported(tag))
    }
}
