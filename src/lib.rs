use pin_project::pin_project;
use tokio::io::{AsyncRead, AsyncWrite};

pub mod socks5;

#[pin_project]
pub struct ObfucationAsyncReader<R> {
    #[pin]
    inner: R,
}

impl<R> ObfucationAsyncReader<R> {
    pub fn new(inner: R) -> Self {
        Self { inner }
    }
}

impl<R: AsyncRead> AsyncRead for ObfucationAsyncReader<R> {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        let this = self.project();
        let old = buf.filled().len();
        let result = this.inner.poll_read(cx, buf);
        let filled_buf = buf.filled_mut();
        for i in old..filled_buf.len() {
            filled_buf[i] = !filled_buf[i];
        }
        result
    }
}

#[pin_project]
pub struct ObfucationAsyncWriter<W> {
    #[pin]
    inner: W,
}

impl<W> ObfucationAsyncWriter<W> {
    pub fn new(inner: W) -> Self {
        Self { inner }
    }
}

impl<W> AsyncWrite for ObfucationAsyncWriter<W>
where
    W: AsyncWrite,
{
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<Result<usize, std::io::Error>> {
        let new_buf: Vec<_> = buf.iter().map(|v| !v).collect();
        let this = self.project();
        this.inner.poll_write(cx, &new_buf)
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        let this = self.project();
        this.inner.poll_flush(cx)
    }

    fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        let this = self.project();
        this.inner.poll_shutdown(cx)
    }
}
