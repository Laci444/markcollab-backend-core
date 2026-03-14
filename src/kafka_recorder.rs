use rdkafka::{
    producer::{FutureProducer, FutureRecord},
    util::Timeout,
};
use tokio::sync::broadcast;
use tracing::{error, info, instrument, trace};
use uuid::Uuid;
use y_octo::{
    write_sync_message,
    DocMessage::{Step2, Update},
    SyncMessage,
    SyncMessage::Doc,
};

pub struct KafkaRecorder {
    document_id: String,
    producer: FutureProducer,
    receiver: broadcast::Receiver<SyncMessage>,
}

impl KafkaRecorder {
    pub fn new(
        document_id: Uuid,
        producer: FutureProducer,
        receiver: broadcast::Receiver<SyncMessage>,
    ) -> Self {
        Self {
            document_id: document_id.to_string(),
            producer,
            receiver,
        }
    }

    #[instrument(skip_all, fields(doc_id = %self.document_id))]
    pub async fn run(mut self) {
        info!("Kafka Recorder started. Listening for CRDT updates...");

        while let Ok(msg) = self.receiver.recv().await {
            let inner_payload = match &msg {
                Doc(Update(payload)) => payload,
                Doc(Step2(payload)) => payload,
                _ => continue,
            };

            let payload_slice = inner_payload.as_slice();
            if payload_slice.is_empty() || payload_slice == [0, 0] || payload_slice == [0] {
                trace!("Dropped empty CRDT delta (no actual document changes)");
                continue;
            }

            trace!("Valid update received, serializing to raw binary...");

            let record = FutureRecord::to("document-events")
                .key(&self.document_id)
                .payload(payload_slice);

            match self.producer.send(record, Timeout::Never).await {
                Ok(_) => trace!("Successfully persisted to Kafka"),
                Err((err, _)) => error!(error = %err, "Failed to persist to Kafka!"),
            }
        }

        info!("Kafka Recorder stopped (Room channel closed)");
    }
}
