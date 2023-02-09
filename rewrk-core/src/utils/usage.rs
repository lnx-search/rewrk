use std::io;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll};

use pin_project_lite::pin_project;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

#[derive(Clone, Default)]
/// A utility for wrapping streams and measuring the number of
/// bytes being passed through the wrapped stream.
pub(crate) struct UsageTracker {
    received: Arc<AtomicU64>,
}

impl UsageTracker {
    /// Create a new usage tracker.
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Wrap an existing stream with the usage tracker.
    pub(crate) fn wrap_stream<I>(&self, stream: I) -> RecordStream<I> {
        RecordStream::new(stream, self.clone())
    }

    /// Get the current usage count.
    pub(crate) fn get_count(&self) -> u64 {
        self.received.load(Ordering::SeqCst)
    }
}

pin_project! {
    pub(crate) struct RecordStream<I> {
        #[pin]
        inner: I,
        usage: UsageTracker,
    }
}

impl<I> RecordStream<I> {
    fn new(inner: I, usage: UsageTracker) -> Self {
        Self { inner, usage }
    }
}

impl<I: AsyncRead> AsyncRead for RecordStream<I> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = self.project();
        let poll_result = this.inner.poll_read(cx, buf);

        this.usage
            .received
            .fetch_add(buf.filled().len() as u64, Ordering::SeqCst);

        poll_result
    }
}

impl<I: AsyncWrite> AsyncWrite for RecordStream<I> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        self.project().inner.poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.project().inner.poll_flush(cx)
    }

    fn poll_shutdown(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<io::Result<()>> {
        self.project().inner.poll_shutdown(cx)
    }
}
