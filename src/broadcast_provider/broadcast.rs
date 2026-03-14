use std::sync::{Arc, RwLock};

use futures_util::{SinkExt, StreamExt};
use tokio::sync::broadcast;
use tokio::sync::broadcast::error::SendError;
use tokio::task::JoinHandle;
use tokio::{select, sync::broadcast::Sender};
use tracing::{debug, error, info, instrument, trace, warn, Instrument};
use y_octo::{Doc, SyncMessage};

use super::protocol::{AsyncKafkaProtocol, MarkcollabProtocol};
use super::{YObject, YObjectRef};

pub struct BroadcastGroup {
    sender: Sender<SyncMessage>,
    yobject: YObjectRef,
}

impl BroadcastGroup {
    #[instrument(skip_all)]
    pub async fn default(buffer_capacity: usize) -> Self {
        info!(
            buffer_capacity = buffer_capacity,
            "Creating empty BroadcastGroup"
        );
        // TODO: in the future broadcast channels will be inefficient. need to switch to mspc channel per client
        let (sender, _) = broadcast::channel(buffer_capacity);
        BroadcastGroup {
            sender,
            yobject: Arc::new(RwLock::new(YObject::default())),
        }
    }

    #[instrument(skip_all)]
    pub async fn new(buffer_capacity: usize, initial_text: &str) -> Self {
        info!(
            buffer_capacity = buffer_capacity,
            "Creating BroadcastGroup with initial text"
        );
        // TODO: in the future broadcast channels will be inefficient. need to switch to mspc channel per client

        let y_doc = Doc::default();
        y_doc
            .get_or_create_text("collaboration")
            .unwrap()
            .insert(0, initial_text)
            .unwrap();

        let (sender, _) = broadcast::channel(buffer_capacity);
        BroadcastGroup {
            sender,
            yobject: Arc::new(RwLock::new(YObject::from(y_doc))),
        }
    }

    pub fn yobject(&self) -> YObjectRef {
        self.yobject.clone()
    }

    #[instrument(skip_all)]
    pub fn subscribe<Sink, Stream, E>(&self, sink: Sink, stream: Stream) -> Subscription
    where
        Sink: SinkExt<SyncMessage> + Send + Sync + Unpin + 'static,
        Stream: StreamExt<Item=Result<SyncMessage, E>> + Send + Sync + Unpin + 'static,
        <Sink as futures_util::Sink<SyncMessage>>::Error: std::error::Error + Send + Sync,
        E: std::error::Error + Send + Sync + 'static,
    {
        self.subscribe_with(sink, stream, MarkcollabProtocol)
    }

    //noinspection D
    #[instrument(skip_all)]
    pub fn subscribe_with<Sink, Stream, E, P>(
        &self,
        mut sink: Sink,
        mut stream: Stream,
        protocol: P,
    ) -> Subscription
    where
        Sink: SinkExt<SyncMessage> + Send + Sync + Unpin + 'static,
        Stream: StreamExt<Item=Result<SyncMessage, E>> + Send + Sync + Unpin + 'static,
        <Sink as futures_util::Sink<SyncMessage>>::Error: std::error::Error + Send + Sync,
        E: std::error::Error + Send + Sync + 'static,
        P: AsyncKafkaProtocol + Send + 'static,
    {
        info!("Creating subscription with Markcollab protocol");

        let sink_task = {
            let mut receiver = self.sender.subscribe();
            tokio::spawn(
                async move {
                    let mut message_count = 0u64;
                    debug!("Sink task started");

                    while let Ok(msg) = receiver.recv().await {
                        message_count += 1;
                        trace!(message_count = message_count, "Sending message to sink");

                        if let Err(e) = sink.send(msg).await {
                            warn!(error = %e, message_count = message_count, "Sink send failed");
                            return Err(format!(
                                "Sink send failed after {} messages: {}",
                                message_count, e
                            ));
                        }
                    }

                    info!(
                        message_count = message_count,
                        "Sink task completed successfully"
                    );
                    Ok(())
                }
                    .instrument(tracing::info_span!("sink_task")),
            )
        };

        let stream_task = {
            let yobject = self.yobject();
            let sender = self.sender.clone();
            tokio::spawn(
                async move {
                    debug!("Stream task started");

                    while let Some(incoming) = stream.next().await {
                        let msg = match incoming {
                            Ok(message) => {
                                trace!("Received message from stream");
                                message
                            }
                            Err(err) => {
                                error!(error = %err, "Socket error, skipping message");
                                continue;
                            }
                        };

                        let response = match protocol.handle(yobject.clone(), msg).unwrap() {
                            Some(message) => message,
                            None => continue,
                        };
                        match sender.send(response) {
                            Ok(subscriber_count) => {
                                trace!(
                                    subscriber_count = subscriber_count,
                                    "Broadcasted reply message"
                                );
                            }
                            Err(_) => {
                                debug!("No active subscribers");
                            }
                        }
                    }

                    info!("Stream task completed");
                    Ok(())
                }
                    .instrument(tracing::info_span!("stream_task")),
            )
        };

        debug!("Subscription created successfully");
        Subscription {
            sink_task,
            stream_task,
        }
    }

    #[instrument(skip(self, msg))]
    fn broadcast(&self, msg: SyncMessage) -> Result<(), SendError<SyncMessage>> {
        match self.sender.send(msg) {
            Ok(subscriber_count) => {
                debug!(
                    subscriber_count = subscriber_count,
                    "Message broadcasted successfully"
                );
                Ok(())
            }
            Err(e) => {
                warn!(error = %e, "Failed to broadcast message");
                Err(e)
            }
        }
    }

    #[instrument(skip(self))]
    pub fn subscribe_observer(&self) -> broadcast::Receiver<SyncMessage> {
        debug!("Creating an observer subscription for background processing");
        self.sender.subscribe()
    }
}

#[derive(Debug)]
pub struct Subscription {
    sink_task: JoinHandle<Result<(), String>>,
    stream_task: JoinHandle<Result<(), String>>,
}

impl Subscription {
    #[instrument(skip(self))]
    pub async fn completed(self) -> Result<(), String> {
        debug!("Waiting for subscription to complete");

        let res = select! {
            r1 = self.sink_task => {
                debug!("Sink task completed first");
                r1
            },
            r2 = self.stream_task => {
                debug!("Stream task completed first");
                r2
            },
        };

        match res {
            Ok(Ok(())) => {
                info!("Subscription completed successfully");
                Ok(())
            }
            Ok(Err(e)) => {
                warn!(error = %e, "Subscription completed with task error");
                Err(e)
            }
            Err(e) => {
                error!(error = %e, "Subscription task panicked");
                Err(e.to_string())
            }
        }
    }
}
