use std::sync::{Arc, RwLock};

use futures_util::{SinkExt, StreamExt};
use log::{error, trace};
use tokio::sync::broadcast::error::SendError;
use tokio::task::JoinHandle;
use tokio::{select, sync::broadcast::Sender, sync::mpsc::unbounded_channel};
use y_octo::write_sync_message;

use super::protocol::{AsyncKafkaProtocol, MarkcollabProtocol, MessageType};
use super::{YObject, YObjectRef};

pub struct BroadcastGroup {
    _sender: Sender<Vec<u8>>, // kafka sendernek kellene lennie
    // kell meg ide egy kafka receiver lehet kell neki egy sajat task
    yobject: YObjectRef,
}

impl BroadcastGroup {
    /// Creates a new [BroadcastGroup] over a provided `awareness` instance. All changes triggered
    /// by this awareness structure or its underlying document will be propagated to all subscribers
    /// which have been registered via [BroadcastGroup::subscribe] method.
    ///
    /// The overflow of the incoming events that needs to be propagates will be buffered up to a
    /// provided `buffer_capacity` size.
    pub async fn new() -> Self {
        BroadcastGroup {
            _sender: Sender::new(10),
            yobject: Arc::new(RwLock::new(YObject::default())),
        }
    }

    /// Returns a reference to an underlying [Awareness] instance.
    pub fn yobject(&self) -> YObjectRef {
        self.yobject.clone()
    }

    /// Subscribes a new connection - represented by `sink`/`stream` pair implementing a futures
    /// Sink and Stream protocols - to a current broadcast group.
    ///
    /// Returns a subscription structure, which can be dropped in order to unsubscribe or awaited
    /// via [Subscription::completed] method in order to complete of its own volition (due to
    /// an internal connection error or closed connection).
    pub fn subscribe<Sink, Stream, E>(&self, sink: Sink, stream: Stream) -> Subscription
    where
        Sink: SinkExt<Vec<u8>> + Send + Sync + Unpin + 'static,
        Stream: StreamExt<Item = Result<Vec<u8>, E>> + Send + Sync + Unpin + 'static,
        <Sink as futures_util::Sink<Vec<u8>>>::Error: std::error::Error + Send + Sync,
        E: std::error::Error + Send + Sync + 'static,
    {
        self.subscribe_with(sink, stream, MarkcollabProtocol)
    }

    /// Subscribes a new connection - represented by `sink`/`stream` pair implementing a futures
    /// Sink and Stream protocols - to a current broadcast group.
    ///
    /// Returns a subscription structure, which can be dropped in order to unsubscribe or awaited
    /// via [Subscription::completed] method in order to complete of its own volition (due to
    /// an internal connection error or closed connection).
    pub fn subscribe_with<Sink, Stream, E, P>(
        &self,
        _sink: Sink,
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
        let (internal_sink, mut internal_stream) = unbounded_channel::<Vec<u8>>();
        let sink_task = {
            tokio::spawn(async move {
                while let Some(msg) = internal_stream.recv().await {
                    if let Err(e) = internal_sink.send(msg) {
                        trace!("Internal messaging channel is closed {e}");
                        return Err(format!("Internal messaging channel is closed {e}"));
                    }
                }
                Ok(())
            })
        };
        let stream_task = {
            let yobject = self.yobject();
            tokio::spawn(async move {
                while let Some(incoming) = stream.next().await {
                    let msg = match incoming {
                        Ok(message) => message,
                        Err(err) => {
                            error!("Socket error: {err}");
                            break;
                        }
                    };

                    let list_of_reply = protocol
                        .handle(yobject.clone(), &msg)
                        .map_err(|e| format!("Error handling messages {e}"))?;

                    let mut update_buffer = Vec::with_capacity(list_of_reply.len());
                    let mut query_buffer = Vec::with_capacity(list_of_reply.len());

                    for reply in list_of_reply {
                        let _ = match reply {
                            MessageType::UpdateMessage(update) => {
                                write_sync_message(&mut update_buffer, &update)
                            }
                            MessageType::QueryMessage(reply) => {
                                write_sync_message(&mut query_buffer, &reply)
                            }
                        };
                    }
                }
                Ok(())
            })
        };

        Subscription {
            sink_task,
            stream_task,
        }
    }

    /// Broadcasts user message to all active subscribers. Returns error if message could not have
    /// been broadcasted.
    fn _broadcast(&self, msg: Vec<u8>) -> Result<(), SendError<Vec<u8>>> {
        self._sender.send(msg)?;
        Ok(())
    }
}

#[derive(Debug)]
pub struct Subscription {
    sink_task: JoinHandle<Result<(), String>>, // TODO: create good error enums
    stream_task: JoinHandle<Result<(), String>>,
}

impl Subscription {
    /// Consumes current subscription, waiting for it to complete. If an underlying connection was
    /// closed because of failure, an error which caused it to happen will be returned.
    ///
    /// This method doesn't invoke close procedure. If you need that, drop current subscription instead.
    pub async fn completed(self) -> Result<(), String> {
        let res = select! {
            r1 = self.sink_task => r1,
            r2 = self.stream_task => r2,
        };
        res.map_err(|e| e.to_string())?
    }
}
