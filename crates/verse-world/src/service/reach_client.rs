//! Client-side adaptation of a proved direct channel to bounded chamber IO.
use super::{
    client::Client,
    client_runtime::{self, Task},
};
use coder_reach::channel::{Channel, MAX_DATA_BYTES};
use secp256k1::{Keypair, Secp256k1};
use std::{
    io,
    pin::Pin,
    task::{Context, Poll},
};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, DuplexStream, ReadBuf};
struct Direct {
    stream: DuplexStream,
    tasks: Vec<Task>,
}
impl Drop for Direct {
    fn drop(&mut self) {
        for task in &self.tasks {
            task.abort();
        }
    }
}
impl AsyncRead for Direct {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().stream).poll_read(cx, buf)
    }
}
impl AsyncWrite for Direct {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bytes: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.get_mut().stream).poll_write(cx, bytes)
    }
    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().stream).poll_flush(cx)
    }
    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().stream).poll_shutdown(cx)
    }
}
/// The channel has already authenticated the host and checked the device grant.
/// The same device key signs the chamber challenge. No uncertain command is replayed.
pub async fn join<S>(
    channel: Channel<S>,
    device: &secp256k1::SecretKey,
    instance: u64,
    content: Option<[u8; 32]>,
) -> Result<Client, String>
where
    S: super::transport::Transport + 'static,
{
    let key = Keypair::from_secret_key(&Secp256k1::new(), device);
    if channel.binding().client != key.x_only_public_key().0.to_string() {
        return Err("Chamber identity differs from direct-channel identity".into());
    }
    let (mut reader, mut writer) = channel.into_split();
    let (near, far) = tokio::io::duplex(256 * 1024);
    let (mut far_read, mut far_write) = tokio::io::split(far);
    let (done_up, up_done) = tokio::sync::oneshot::channel();
    let (done_down, down_done) = tokio::sync::oneshot::channel();
    let up = client_runtime::spawn(async move {
        // Each partial channel read belongs to this task until completion.
        while let Ok(Some(bytes)) = reader.recv().await {
            if far_write.write_all(&bytes).await.is_err() {
                break;
            }
        }
        let _ = far_write.shutdown().await;
        let _ = done_up.send(());
    });
    let down = client_runtime::spawn(async move {
        let mut bytes = vec![0; MAX_DATA_BYTES];
        loop {
            match far_read.read(&mut bytes).await {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if writer.send(&bytes[..n]).await.is_err() {
                        break;
                    }
                }
            }
        }
        let _ = writer.close().await;
        let _ = done_down.send(());
    });
    // Each task is also held by the transport. A cancelled opening closes IO.
    struct Children(Vec<Task>);
    impl Drop for Children {
        fn drop(&mut self) {
            for task in &self.0 {
                task.abort();
            }
        }
    }
    let children = Children(vec![up, down]);
    let supervisor = client_runtime::spawn(async move {
        let _children = children;
        tokio::select! { _ = up_done => {}, _ = down_done => {} }
    });
    let direct = Direct {
        stream: near,
        tasks: vec![supervisor],
    };
    Client::connect_stream(Box::new(direct), instance, content, &key).await
}
