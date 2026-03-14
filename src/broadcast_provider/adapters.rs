use axum::extract::ws::{Message, WebSocket};
use bytes::Bytes;
use futures_util::stream::{SplitSink, SplitStream};
use futures_util::{Sink, Stream};
use nom;
use std::pin::Pin;
use std::task::{Context, Poll};
use y_octo::{read_sync_message, write_sync_message, JwstCodecError, SyncMessage};

pub struct ProtocolSink<S> {
    inner: S,
}

impl<S> ProtocolSink<S> {
    pub fn new(inner: S) -> Self {
        Self { inner }
    }
}

impl<S> Sink<SyncMessage> for ProtocolSink<S>
where
    S: Sink<Vec<u8>> + Unpin,
{
    type Error = ProtocolSinkError<S::Error>;

    fn poll_ready(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Pin::new(&mut self.inner)
            .poll_ready(cx)
            .map_err(ProtocolSinkError::Sink)
    }

    fn start_send(mut self: Pin<&mut Self>, item: SyncMessage) -> Result<(), Self::Error> {
        let mut buffer = Vec::new();
        write_sync_message(&mut buffer, &item).map_err(|e| ProtocolSinkError::Serialization(e))?;

        Pin::new(&mut self.inner)
            .start_send(buffer)
            .map_err(ProtocolSinkError::Sink)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Pin::new(&mut self.inner)
            .poll_flush(cx)
            .map_err(ProtocolSinkError::Sink)
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Pin::new(&mut self.inner)
            .poll_close(cx)
            .map_err(ProtocolSinkError::Sink)
    }
}

/// Error type for ProtocolSink
#[derive(Debug)]
pub enum ProtocolSinkError<E> {
    Serialization(std::io::Error),
    Sink(E),
}

impl<E: std::fmt::Display> std::fmt::Display for ProtocolSinkError<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Serialization(e) => write!(f, "Serialization error: {}", e),
            Self::Sink(e) => write!(f, "Sink error: {}", e),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for ProtocolSinkError<E> {}

pub struct ProtocolStream<S> {
    inner: S,
    buffer: Vec<u8>,
}

impl<S> ProtocolStream<S> {
    pub fn new(inner: S) -> Self {
        Self {
            inner,
            buffer: Vec::new(),
        }
    }
}

impl<S, E> Stream for ProtocolStream<S>
where
    S: Stream<Item = Result<Bytes, E>> + Unpin,
{
    type Item = Result<SyncMessage, ProtocolError<E>>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match Pin::new(&mut self.inner).poll_next(cx) {
            Poll::Ready(Some(Ok(bytes))) => {
                self.buffer.extend_from_slice(&bytes);

                match read_sync_message(&self.buffer) {
                    Ok((remaining, msg)) => {
                        let consumed = self.buffer.len() - remaining.len();
                        self.buffer.drain(..consumed);
                        Poll::Ready(Some(Ok(msg)))
                    }
                    Err(nom::Err::Incomplete(_))
                    | Err(nom::Err::Error(nom::error::Error {
                        code: nom::error::ErrorKind::Eof,
                        ..
                    }))
                    | Err(nom::Err::Failure(nom::error::Error {
                        code: nom::error::ErrorKind::Eof,
                        ..
                    })) => Poll::Pending,
                    Err(_) => {
                        self.buffer.clear();
                        Poll::Ready(Some(Err(ProtocolError::Decode(
                            JwstCodecError::IncompleteDocument(String::from("invalid buffer")),
                        ))))
                    }
                }
            }
            Poll::Ready(Some(Err(e))) => Poll::Ready(Some(Err(ProtocolError::Transport(e)))),
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}

#[derive(Debug)]
pub enum ProtocolError<E> {
    Transport(E),
    Decode(JwstCodecError),
}

impl<E: std::fmt::Display> std::fmt::Display for ProtocolError<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Transport(e) => write!(f, "Transport error: {}", e),
            Self::Decode(e) => write!(f, "Protocol decode error: {}", e),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for ProtocolError<E> {}

// Adapter to convert Axum WebSocket Sink to work with BroadcastGroup
pub struct AxumSink {
    pub inner: SplitSink<WebSocket, Message>,
}

impl Sink<Vec<u8>> for AxumSink {
    type Error = axum::Error;

    fn poll_ready(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Pin::new(&mut self.inner).poll_ready(cx)
    }

    fn start_send(mut self: Pin<&mut Self>, item: Vec<u8>) -> Result<(), Self::Error> {
        Pin::new(&mut self.inner).start_send(Message::Binary(Bytes::from(item)))
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Pin::new(&mut self.inner).poll_close(cx)
    }
}

// Adapter to convert Axum WebSocket Stream to work with BroadcastGroup
pub struct AxumStream {
    pub inner: SplitStream<WebSocket>,
}

impl Stream for AxumStream {
    type Item = Result<Bytes, axum::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let temp = Pin::new(&mut self.inner).poll_next(cx);
        //match Pin::new(&mut self.inner).poll_next(cx) {
        match temp {
            Poll::Ready(Some(Ok(msg))) => {
                match msg {
                    Message::Binary(data) => Poll::Ready(Some(Ok(data))),
                    //Message::Text(text) => Poll::Ready(Some(Ok(text))),
                    Message::Close(_) => Poll::Ready(None),
                    _ => Poll::Pending,
                }
            }
            Poll::Ready(Some(Err(e))) => Poll::Ready(Some(Err(e))),
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}
