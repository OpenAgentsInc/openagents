//! A fixed broker TCP destination reached from the node manager's host network.
//! TLS, SNI, and authentication stay end to end in the gateway client.
use crate::Failure;
use std::{path::PathBuf, time::Duration};
use tokio::{
    net::{TcpStream, UnixListener},
    task::{JoinHandle, JoinSet},
};

/// The caller binds a private filesystem socket in a root-controlled directory.
/// Peer credentials restrict it to one gateway UID. Removing that gateway's
/// runtime directory stops the listener and every connection it owns.
pub fn spawn(
    listener: UnixListener,
    origin: &str,
    gateway_uid: u32,
    runtime_directory: PathBuf,
) -> Result<JoinHandle<()>, Failure> {
    let url = url::Url::parse(origin).map_err(|_| Failure::Invalid)?;
    if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.path() != "/"
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(Failure::Invalid);
    }
    let host = url.host_str().ok_or(Failure::Invalid)?.to_owned();
    let port = url.port_or_known_default().ok_or(Failure::Invalid)?;
    use std::os::unix::fs::MetadataExt;
    struct SocketFile(PathBuf, u64, u64);
    impl Drop for SocketFile {
        fn drop(&mut self) {
            if std::fs::symlink_metadata(&self.0)
                .is_ok_and(|m| m.dev() == self.1 && m.ino() == self.2)
            {
                let _ = std::fs::remove_file(&self.0);
            }
        }
    }
    let path = listener
        .local_addr()
        .map_err(|_| Failure::Unavailable)?
        .as_pathname()
        .ok_or(Failure::Invalid)?
        .to_owned();
    let metadata = std::fs::symlink_metadata(&path).map_err(|_| Failure::Unavailable)?;
    let cleanup = SocketFile(path, metadata.dev(), metadata.ino());
    Ok(tokio::spawn(async move {
        let _cleanup = cleanup;
        let mut connections = JoinSet::new();
        let mut timer = tokio::time::interval(Duration::from_secs(1));
        loop {
            tokio::select! {
                _ = timer.tick() => {
                    if !runtime_directory.is_dir() { break; }
                }
                Some(_) = connections.join_next(), if !connections.is_empty() => {}
                accepted = listener.accept() => {
                    let Ok((mut local, _)) = accepted else { break; };
                    if connections.len() >= 8 || local.peer_cred().ok().map(|c| c.uid()) != Some(gateway_uid) {
                        continue;
                    }
                    let host = host.clone();
                    connections.spawn(async move {
                        if let Ok(Ok(mut remote)) = tokio::time::timeout(
                            Duration::from_secs(5), TcpStream::connect((host.as_str(), port))
                        ).await {
                            let _ = tokio::io::copy_bidirectional(&mut local, &mut remote).await;
                        }
                    });
                }
            }
        }
        // Dropping the set also aborts every accepted connection on cancellation.
        connections.abort_all();
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::{TcpListener, UnixStream},
    };

    #[tokio::test]
    async fn relay_preserves_bytes_and_closes_connections_when_the_gateway_is_removed() {
        let directory =
            std::env::temp_dir().join(format!("oa-credential-relay-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&directory).unwrap();
        let socket = directory.join("broker.sock");
        let listener = UnixListener::bind(&socket).unwrap();
        std::fs::set_permissions(&socket, std::fs::Permissions::from_mode(0o600)).unwrap();
        let tcp = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = tcp.local_addr().unwrap();
        let denied_socket = directory.join("denied.sock");
        let denied_listener = UnixListener::bind(&denied_socket).unwrap();
        let denied = spawn(
            denied_listener,
            &format!("https://{address}"),
            unsafe { libc::geteuid() }.wrapping_add(1),
            directory.clone(),
        )
        .unwrap();
        let mut foreign = UnixStream::connect(&denied_socket).await.unwrap();
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(1), foreign.read(&mut [0]))
                .await
                .unwrap()
                .unwrap(),
            0
        );
        denied.abort();
        let _ = denied.await;
        let server = tokio::spawn(async move {
            let (mut stream, _) = tcp.accept().await.unwrap();
            let mut request = [0; 5];
            stream.read_exact(&mut request).await.unwrap();
            assert_eq!(&request, b"hello");
            stream.write_all(b"reply").await.unwrap();
            let mut byte = [0];
            assert_eq!(stream.read(&mut byte).await.unwrap(), 0);
        });
        let relay = spawn(
            listener,
            &format!("https://{address}"),
            unsafe { libc::geteuid() },
            directory.clone(),
        )
        .unwrap();
        let mut client = UnixStream::connect(&socket).await.unwrap();
        client.write_all(b"hello").await.unwrap();
        let mut response = [0; 5];
        client.read_exact(&mut response).await.unwrap();
        assert_eq!(&response, b"reply");
        std::fs::remove_dir_all(&directory).unwrap();
        tokio::time::timeout(Duration::from_secs(3), relay)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(client.read(&mut [0]).await.unwrap(), 0);
        server.await.unwrap();
    }
}
