use crate::protocol::StreamHeader;
use async_trait::async_trait;
use futures::{Sink, SinkExt as _, Stream};
use serde::{Serialize, de::DeserializeOwned};
use std::{
    io,
    marker::PhantomData,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};
use tokio::{
    io::{AsyncRead, AsyncWrite, DuplexStream, ReadHalf, WriteHalf},
    sync::{Mutex, Notify, mpsc},
};
use tokio_util::{
    bytes::Bytes,
    codec::{FramedRead, FramedWrite, LengthDelimitedCodec},
};

/// The send half of a bidirectional stream.
pub trait PeerSendStream: AsyncWrite + Send + Unpin {
    /// Close the send stream immediately.
    ///
    /// No new data can be written after calling this method. Locally buffered data is dropped,
    /// and previously transmitted data will no longer be retransmitted if lost. If an attempt has
    /// already been made to finish the stream, the peer may still receive all written data.
    ///
    /// See `noq::SendStream::reset`.
    fn reset(&mut self, code: u32) -> Result<(), anyhow::Error>;
}

/// The receive half of a bidirectional stream.
pub trait PeerRecvStream: AsyncRead + Send + Unpin {
    /// Stop accepting data.
    ///
    /// Discards unread data and notifies the peer to stop transmitting.
    ///
    /// See `noq::RecvStream::stop`.
    fn stop(&mut self, code: u32) -> Result<(), anyhow::Error>;
}

pub type BoxSendStream = Box<dyn PeerSendStream>;
pub type BoxRecvStream = Box<dyn PeerRecvStream>;

/// A length-delimited-framed [`PeerConnection`] send stream.
pub type FramedSendStream = FramedWrite<BoxSendStream, LengthDelimitedCodec>;

/// A length-delimited-framed [`PeerConnection`] receive stream.
pub type FramedRecvStream = FramedRead<BoxRecvStream, LengthDelimitedCodec>;

/// A connection that can open and accept bidirectional length-delimited-framed streams.
#[async_trait]
pub trait PeerConnection: Send + Sync {
    async fn open_bi(&self) -> Result<(FramedSendStream, FramedRecvStream), anyhow::Error>;

    async fn accept_bi(&self) -> Result<(FramedSendStream, FramedRecvStream), anyhow::Error>;

    /// Closes the connection immediately.
    ///
    /// Pending operations will fail immediately. No more data is sent to the peer and the peer may
    /// drop buffered data upon receiving the CONNECTION_CLOSE frame.
    ///
    /// See `iroh::endpoint::connection::Connection::close`.
    fn close(&self, code: u32, reason: &[u8]);

    /// Waits for the connection to be closed for any reason.
    ///
    /// See `iroh::endpoint::connection::Connection::closed`.
    async fn closed(&self);
}

fn framed(tx: BoxSendStream, rx: BoxRecvStream) -> (FramedSendStream, FramedRecvStream) {
    let tx = FramedWrite::new(tx, LengthDelimitedCodec::new());
    let rx = FramedRead::new(rx, LengthDelimitedCodec::new());

    (tx, rx)
}

/// A message that can be sent or received on a [`PeerConnection`] stream.
///
/// Encoded with `postcard`.
pub trait Message: Serialize + DeserializeOwned + Sized {
    fn encode(&self) -> Bytes {
        postcard::to_stdvec(self)
            .expect("Failed to serialize message")
            .into()
    }

    fn decode(bytes: &[u8]) -> Result<Self, anyhow::Error> {
        postcard::from_bytes(bytes)
            .map_err(|e| anyhow::anyhow!("Failed to deserialize message: {e:?}"))
    }
}

/// A [`FramedSendStream`] for a specific [`Message`] type with access to the underlying stream.
pub struct MessageSink<M> {
    inner: FramedSendStream,
    // This is required for `MessageSink` to be Unpin or something like that
    _message: PhantomData<fn() -> M>,
}

impl<M> MessageSink<M> {
    pub fn new(inner: FramedSendStream) -> Self {
        Self {
            inner,
            _message: PhantomData,
        }
    }

    /// Close the send stream immediately.
    ///
    /// No new data can be written after calling this method. Locally buffered data is dropped,
    /// and previously transmitted data will no longer be retransmitted if lost. If an attempt has
    /// already been made to finish the stream, the peer may still receive all written data.
    ///
    /// See `noq::SendStream::reset`.
    pub fn reset(&mut self, code: u32) -> Result<(), anyhow::Error> {
        self.inner.get_mut().reset(code)
    }
}

impl<M: Message> Sink<M> for MessageSink<M> {
    type Error = io::Error;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        Pin::new(&mut self.get_mut().inner).poll_ready(cx)
    }

    fn start_send(self: Pin<&mut Self>, item: M) -> Result<(), io::Error> {
        Pin::new(&mut self.get_mut().inner).start_send(item.encode())
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        Pin::new(&mut self.get_mut().inner).poll_flush(cx)
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        Pin::new(&mut self.get_mut().inner).poll_close(cx)
    }
}

/// A [`FramedRecvStream`] for a specific [`Message`] type with access to the underlying stream.
pub struct MessageStream<M> {
    inner: FramedRecvStream,
    // This is required for `MessageStream` to be Unpin or something like that
    _message: PhantomData<fn() -> M>,
}

impl<M> MessageStream<M> {
    pub fn new(inner: FramedRecvStream) -> Self {
        Self {
            inner,
            _message: PhantomData,
        }
    }

    /// Stop accepting data.
    ///
    /// Discards unread data and notifies the peer to stop transmitting.
    ///
    /// See `noq::RecvStream::stop`.
    pub fn stop(&mut self, code: u32) -> Result<(), anyhow::Error> {
        self.inner.get_mut().stop(code)
    }
}

impl<M: Message> Stream for MessageStream<M> {
    type Item = Result<M, anyhow::Error>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match Pin::new(&mut self.get_mut().inner).poll_next(cx) {
            Poll::Ready(Some(Ok(bytes))) => Poll::Ready(Some(M::decode(&bytes))),
            Poll::Ready(Some(Err(e))) => Poll::Ready(Some(Err(anyhow::anyhow!(
                "Failed to read from stream: {e:?}"
            )))),
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}

/// Opens a bidirectional stream, sends `header`, and returns a typed message sink and stream.
pub async fn open_message_stream<Tx, Rx>(
    connection: &dyn PeerConnection,
    header: StreamHeader,
) -> Result<(MessageSink<Tx>, MessageStream<Rx>), anyhow::Error>
where
    Tx: Message,
    Rx: Message,
{
    let (mut tx, rx) = connection.open_bi().await?;

    tx.send(header.encode())
        .await
        .map_err(|e| anyhow::anyhow!("Failed to send stream header: {e}"))?;

    Ok((MessageSink::new(tx), MessageStream::new(rx)))
}

impl PeerSendStream for iroh::endpoint::SendStream {
    fn reset(&mut self, code: u32) -> Result<(), anyhow::Error> {
        iroh::endpoint::SendStream::reset(self, iroh::endpoint::VarInt::from_u32(code))
            .map_err(anyhow::Error::from)
    }
}

impl PeerRecvStream for iroh::endpoint::RecvStream {
    fn stop(&mut self, code: u32) -> Result<(), anyhow::Error> {
        iroh::endpoint::RecvStream::stop(self, iroh::endpoint::VarInt::from_u32(code))
            .map_err(anyhow::Error::from)
    }
}

/// A [`PeerConnection`] backed by a real `iroh::endpoint::Connection`.
pub struct IrohPeerConnection(iroh::endpoint::Connection);

impl IrohPeerConnection {
    pub fn new(connection: iroh::endpoint::Connection) -> Arc<Self> {
        Arc::new(Self(connection))
    }
}

#[async_trait]
impl PeerConnection for IrohPeerConnection {
    async fn open_bi(&self) -> Result<(FramedSendStream, FramedRecvStream), anyhow::Error> {
        let (tx, rx) = self.0.open_bi().await?;
        Ok(framed(Box::new(tx), Box::new(rx)))
    }

    async fn accept_bi(&self) -> Result<(FramedSendStream, FramedRecvStream), anyhow::Error> {
        let (tx, rx) = self.0.accept_bi().await?;
        Ok(framed(Box::new(tx), Box::new(rx)))
    }

    fn close(&self, code: u32, reason: &[u8]) {
        self.0.close(iroh::endpoint::VarInt::from_u32(code), reason);
    }

    async fn closed(&self) {
        self.0.closed().await;
    }
}

struct MemorySendStream(WriteHalf<DuplexStream>);

impl AsyncWrite for MemorySendStream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.get_mut().0).poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().0).poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().0).poll_shutdown(cx)
    }
}

impl PeerSendStream for MemorySendStream {
    fn reset(&mut self, _code: u32) -> Result<(), anyhow::Error> {
        Ok(())
    }
}

struct MemoryRecvStream(ReadHalf<DuplexStream>);

impl AsyncRead for MemoryRecvStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().0).poll_read(cx, buf)
    }
}

impl PeerRecvStream for MemoryRecvStream {
    fn stop(&mut self, _code: u32) -> Result<(), anyhow::Error> {
        Ok(())
    }
}

/// An in-memory [`PeerConnection`] for testing.
pub struct MemoryPeerConnection {
    incoming: Mutex<mpsc::UnboundedReceiver<(BoxSendStream, BoxRecvStream)>>,
    outgoing: mpsc::UnboundedSender<(BoxSendStream, BoxRecvStream)>,
    closed: Notify,
}

impl MemoryPeerConnection {
    /// Returns a pair of connected [`MemoryPeerConnection`]s.
    pub fn new() -> (Arc<Self>, Arc<Self>) {
        let (tx_a, rx_a) = mpsc::unbounded_channel();
        let (tx_b, rx_b) = mpsc::unbounded_channel();

        let a = Arc::new(Self {
            incoming: Mutex::new(rx_a),
            outgoing: tx_b,
            closed: Notify::new(),
        });
        let b = Arc::new(Self {
            incoming: Mutex::new(rx_b),
            outgoing: tx_a,
            closed: Notify::new(),
        });

        (a, b)
    }
}

#[async_trait]
impl PeerConnection for MemoryPeerConnection {
    async fn open_bi(&self) -> Result<(FramedSendStream, FramedRecvStream), anyhow::Error> {
        let (a, b) = tokio::io::duplex(MEMORY_STREAM_BUFFER);
        let (a_read, a_write) = tokio::io::split(a);
        let (b_read, b_write) = tokio::io::split(b);

        let local: (BoxSendStream, BoxRecvStream) = (
            Box::new(MemorySendStream(a_write)),
            Box::new(MemoryRecvStream(a_read)),
        );
        let remote: (BoxSendStream, BoxRecvStream) = (
            Box::new(MemorySendStream(b_write)),
            Box::new(MemoryRecvStream(b_read)),
        );

        self.outgoing
            .send(remote)
            .map_err(|_| anyhow::anyhow!("peer connection closed"))?;
        Ok(framed(local.0, local.1))
    }

    async fn accept_bi(&self) -> Result<(FramedSendStream, FramedRecvStream), anyhow::Error> {
        let (tx, rx) = self
            .incoming
            .lock()
            .await
            .recv()
            .await
            .ok_or_else(|| anyhow::anyhow!("peer connection closed"))?;
        Ok(framed(tx, rx))
    }

    fn close(&self, _code: u32, _reason: &[u8]) {
        self.closed.notify_waiters();
    }

    async fn closed(&self) {
        self.closed.notified().await;
    }
}

/// The buffer size used for each direction of an in-memory stream.
const MEMORY_STREAM_BUFFER: usize = 64 * 1024;
