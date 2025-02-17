use futures_util::{SinkExt, StreamExt};
use log::{error, trace};
use tokio::task::JoinHandle;
use tokio::{select, sync::broadcast::Sender, sync::mpsc::unbounded_channel};
use y_octo::write_sync_message;

use super::protocol::{self, MessageType};
use super::YObjectRef;

pub struct BroadcastGroup {
    sender: Sender<Vec<u8>>,
    yobject_ref: YObjectRef,
}

impl BroadcastGroup {
    /// Returns a reference to an underlying [Awareness] instance.
    pub fn yobject(&self) -> &YObjectRef {
        &self.yobject_ref
    }

    /// Subscribes a new connection - represented by `sink`/`stream` pair implementing a futures
    /// Sink and Stream protocols - to a current broadcast group.
    ///
    /// Returns a subscription structure, which can be dropped in order to unsubscribe or awaited
    /// via [Subscription::completed] method in order to complete of its own volition (due to
    /// an internal connection error or closed connection).
    pub fn subscribe<Sink, Stream, E>(&self, sink: Sink, mut stream: Stream) -> Subscription
    where
        Sink: SinkExt<Vec<u8>> + Send + Sync + Unpin + 'static,
        Stream: StreamExt<Item = Result<Vec<u8>, E>> + Send + Sync + Unpin + 'static,
        <Sink as futures_util::Sink<Vec<u8>>>::Error: std::error::Error + Send + Sync,
        E: std::error::Error + Send + Sync + 'static,
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
            let yobject = self.yobject().clone();
            tokio::spawn(async move {
                while let Some(incoming) = stream.next().await {
                    let msg = match incoming {
                        Ok(message) => message,
                        Err(err) => {
                            error!("Socket error: {err}");
                            break;
                        }
                    };

                    let list_of_reply = protocol::handle(yobject.clone(), &msg)
                        .await
                        .map_err(|e| format!("Error handling messages {e}"))?;

                    let mut update_buffer = Vec::with_capacity(list_of_reply.len());
                    let mut query_buffer = Vec::with_capacity(list_of_reply.len());

                    for reply in list_of_reply {
                        match reply {
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
