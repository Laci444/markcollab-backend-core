use std::sync::{Arc, RwLock};

use futures_util::{SinkExt, StreamExt};
use tokio::sync::broadcast::error::SendError;
use tokio::task::JoinHandle;
use tokio::{select, sync::broadcast::Sender};
use tokio::sync::broadcast;
use tokio::sync::broadcast::Receiver;
use tracing::{debug, error, info, instrument, trace, warn, Instrument};
use y_octo::write_sync_message;

use super::protocol::{AsyncKafkaProtocol, MarkcollabProtocol};
use super::{YObject, YObjectRef};

pub struct BroadcastGroup {
    sender: Sender<Vec<u8>>,
    yobject: YObjectRef,
}

impl BroadcastGroup {
    #[instrument(skip_all)]
    pub async fn new(buffer_capacity: usize) -> Self {
        info!(buffer_capacity = buffer_capacity, "Creating new BroadcastGroup");
        let (sender, _) = broadcast::channel(buffer_capacity);
        BroadcastGroup {
            sender,
            yobject: Arc::new(RwLock::new(YObject::default())),
        }
    }

    pub fn yobject(&self) -> YObjectRef {
        self.yobject.clone()
    }

    #[instrument(skip_all)]
    pub fn subscribe<Sink, Stream, E>(&self, sink: Sink, stream: Stream) -> Subscription
    where
        Sink: SinkExt<Vec<u8>> + Send + Sync + Unpin + 'static,
        Stream: StreamExt<Item = Result<Vec<u8>, E>> + Send + Sync + Unpin + 'static,
        <Sink as futures_util::Sink<Vec<u8>>>::Error: std::error::Error + Send + Sync,
        E: std::error::Error + Send + Sync + 'static,
    {
        info!("Subscribing new connection with default protocol");
        self.subscribe_with(sink, stream, MarkcollabProtocol)
    }

    #[instrument(skip_all)]
    pub fn subscribe_with<Sink, Stream, E, P>(
        &self,
        mut sink: Sink,
        mut stream: Stream,
        protocol: P,
    ) -> Subscription
    where
        Sink: SinkExt<Vec<u8>> + Send + Sync + Unpin + 'static,
        Stream: StreamExt<Item = Result<Vec<u8>, E>> + Send + Sync + Unpin + 'static,
        <Sink as futures_util::Sink<Vec<u8>>>::Error: std::error::Error + Send + Sync,
        E: std::error::Error + Send + Sync + 'static,
        P: AsyncKafkaProtocol + std::marker::Send + 'static,
    {
        info!("Creating subscription with custom protocol");

        let sink_task = {
            let mut receiver = self.sender.subscribe();
            tokio::spawn(
                async move {
                    let mut message_count = 0u64;
                    debug!("Sink task started");
                    
                    while let Ok(msg) = receiver.recv().await {
                        message_count += 1;
                        trace!(message_count = message_count, message_size = msg.len(), "Sending message to sink");
                        
                        if let Err(e) = sink.send(msg).await {
                            warn!(error = %e, message_count = message_count, "Sink send failed");
                            return Err(format!("Sink send failed after {} messages: {}", message_count, e));
                        }
                    }
                    
                    info!(message_count = message_count, "Sink task completed successfully");
                    Ok(())
                }
                .instrument(tracing::info_span!("sink_task"))
            )
        };

        let stream_task = {
            let yobject = self.yobject();
            let sender = self.sender.clone();
            tokio::spawn(
                async move {
                    let mut message_count = 0u64;
                    debug!("Stream task started");
                    
                    while let Some(incoming) = stream.next().await {
                        message_count += 1;
                        
                        let msg = match incoming {
                            Ok(message) => {
                                trace!(message_count = message_count, message_size = message.len(), "Received message from stream");
                                message
                            },
                            Err(err) => {
                                error!(error = %err, message_count = message_count, "Socket error, skipping message");
                                continue;
                            }
                        };

                        match protocol.handle(yobject.clone(), &msg) {
                            Err(e) => {
                                error!(error = %e, message_count = message_count, "Protocol handler error");
                                continue;
                            }
                            Ok(list_of_reply) => {
                                let reply_count = list_of_reply.len();
                                trace!(message_count = message_count, reply_count = reply_count, "Processing protocol replies");
                                
                                for (reply_idx, reply) in list_of_reply.into_iter().enumerate() {
                                    let mut buffer = Vec::default();
                                    match write_sync_message(&mut buffer, &reply.unwrap()) {
                                        Ok(_) => {
                                            match sender.send(buffer) {
                                                Ok(subscriber_count) => {
                                                    trace!(
                                                        message_count = message_count, 
                                                        reply_idx = reply_idx, 
                                                        subscriber_count = subscriber_count,
                                                        "Broadcasted reply message"
                                                    );
                                                },
                                                Err(_) => {
                                                    debug!(message_count = message_count, reply_idx = reply_idx, "No active subscribers");
                                                }
                                            }
                                        },
                                        Err(e) => {
                                            error!(error = %e, message_count = message_count, reply_idx = reply_idx, "Failed to write sync message");
                                        }
                                    }
                                }
                            }
                        }
                    }
                    
                    info!(message_count = message_count, "Stream task completed");
                    Ok(())
                }
                .instrument(tracing::info_span!("stream_task"))
            )
        };

        debug!("Subscription created successfully");
        Subscription {
            sink_task,
            stream_task,
        }
    }

    #[instrument(skip(self, msg), fields(msg_size = msg.len()))]
    fn broadcast(&self, msg: Vec<u8>) -> Result<(), SendError<Vec<u8>>> {
        match self.sender.send(msg) {
            Ok(subscriber_count) => {
                debug!(subscriber_count = subscriber_count, "Message broadcasted successfully");
                Ok(())
            }
            Err(e) => {
                warn!(error = %e, "Failed to broadcast message");
                Err(e)
            }
        }
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