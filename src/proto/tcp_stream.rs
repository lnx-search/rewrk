use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll};
use tokio::io::ReadBuf;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;

use std::io::Result;

pub struct CustomTcpStream {
    inner: TcpStream,
    counter: Arc<AtomicUsize>,
}

impl CustomTcpStream {
    pub fn new(stream: TcpStream, counter: Arc<AtomicUsize>) -> Self {
        Self {
            inner: stream,
            counter: counter,
        }
    }
}

impl AsyncRead for CustomTcpStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<Result<()>> {
        let result = Pin::new(&mut self.inner).poll_read(cx, buf);

        self.counter
            .fetch_add(buf.filled().len(), Ordering::Release);

        result
    }
}

impl AsyncWrite for CustomTcpStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize>> {
        Pin::new(&mut self.inner).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}
