//! Owner-only local named pipes for the same terminal request protocol.
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver, SyncSender},
};
use std::time::Duration;
pub use terminal_core::control::Request;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::windows::named_pipe::{ClientOptions, NamedPipeServer, ServerOptions};
pub const SOCKET_ENV: &str = "VERSE_TERMINAL_SOCKET";
pub type Pending = (Request, SyncSender<Value>);

pub fn default_path() -> Option<PathBuf> {
    std::env::var_os(SOCKET_ENV)
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            private_fs::user_sid()
                .ok()
                .map(|sid| PathBuf::from(format!(r"\\.\pipe\openagents-terminal-{sid}")))
        })
}

pub struct Listener {
    path: PathBuf,
    requests: Receiver<Pending>,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl std::fmt::Debug for Listener {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Listener")
            .field("path", &self.path)
            .finish()
    }
}
impl Listener {
    pub fn bind(path: &Path) -> Result<Self, String> {
        let name = path
            .to_str()
            .ok_or("The pipe name is not Unicode.")?
            .to_owned();
        if !name.strip_prefix(r"\\.\pipe\").is_some_and(|rest| {
            !rest.is_empty() && rest.len() <= 200 && !rest.contains(['/', '\\'])
        }) {
            return Err("Use a local Windows pipe name.".into());
        }
        let sid = private_fs::user_sid().map_err(|error| error.to_string())?;
        let (sender, requests) = mpsc::sync_channel(64);
        let (ready, bound) = mpsc::sync_channel(1);
        let stop = Arc::new(AtomicBool::new(false));
        let stopping = Arc::clone(&stop);
        let thread = std::thread::spawn(move || {
            let runtime = match runtime() {
                Ok(runtime) => runtime,
                Err(error) => {
                    let _ = ready.send(Err(error));
                    return;
                }
            };
            runtime.block_on(async move {
                let mut next = match create(&name, &sid, true) {
                    Ok(pipe) => pipe,
                    Err(error) => {
                        let _ = ready.send(Err(error));
                        return;
                    }
                };
                let _ = ready.send(Ok(()));
                let peers = Arc::new(tokio::sync::Semaphore::new(8));
                while !stopping.load(Ordering::Relaxed) {
                    match tokio::time::timeout(Duration::from_millis(100), next.connect()).await {
                        Err(_) => continue,
                        Ok(Err(_)) => break,
                        Ok(Ok(())) => {}
                    }
                    let fresh = match create(&name, &sid, false) {
                        Ok(pipe) => pipe,
                        Err(_) => break,
                    };
                    let connected = std::mem::replace(&mut next, fresh);
                    let Ok(permit) = peers.clone().try_acquire_owned() else {
                        drop(connected);
                        continue;
                    };
                    let sender = sender.clone();
                    tokio::spawn(async move {
                        let _permit = permit;
                        let _ =
                            tokio::time::timeout(Duration::from_secs(15), serve(connected, sender))
                                .await;
                    });
                }
            });
        });
        bound
            .recv()
            .map_err(|_| "The control thread ended.".to_string())??;
        Ok(Self {
            path: path.into(),
            requests,
            stop,
            thread: Some(thread),
        })
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn next(&self) -> Option<Pending> {
        self.requests.try_recv().ok()
    }
}
impl Drop for Listener {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
fn runtime() -> Result<tokio::runtime::Runtime, String> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| error.to_string())
}
fn create(name: &str, sid: &str, first: bool) -> Result<NamedPipeServer, String> {
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Authorization::{
        ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
    };
    use windows_sys::Win32::Security::{PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES};
    let text: Vec<u16> = format!("O:{sid}D:P(A;;GA;;;{sid})")
        .encode_utf16()
        .chain(Some(0))
        .collect();
    let mut descriptor: PSECURITY_DESCRIPTOR = std::ptr::null_mut();
    // SAFETY: the terminated descriptor string lives through the call; Win32 allocates the output.
    if unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            text.as_ptr(),
            SDDL_REVISION_1,
            &mut descriptor,
            std::ptr::null_mut(),
        )
    } == 0
    {
        return Err(std::io::Error::last_os_error().to_string());
    }
    let mut attributes = SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: descriptor,
        bInheritHandle: 0,
    };
    // SAFETY: attributes and the allocated descriptor outlive pipe creation.
    let result = unsafe {
        ServerOptions::new()
            .first_pipe_instance(first)
            .reject_remote_clients(true)
            .create_with_security_attributes_raw(
                name,
                (&mut attributes as *mut SECURITY_ATTRIBUTES).cast(),
            )
    };
    // SAFETY: Win32 allocated this descriptor, and the pipe has copied it.
    unsafe { LocalFree(descriptor) };
    result.map_err(|error| error.to_string())
}
async fn serve(pipe: NamedPipeServer, sender: SyncSender<Pending>) {
    let mut reader = BufReader::new(pipe);
    let mut line = Vec::new();
    // Limit the bytes read before decoding; a peer never owns unbounded memory.
    use tokio::io::AsyncReadExt;
    let count = (&mut reader)
        .take(256 * 1024 + 1)
        .read_until(b'\n', &mut line)
        .await
        .unwrap_or(0);
    if count == 0 || count > 256 * 1024 {
        return;
    }
    let reply = match serde_json::from_slice::<Request>(&line) {
        Ok(request) => {
            let (reply, received) = mpsc::sync_channel(1);
            if sender.try_send((request, reply)).is_err() {
                serde_json::json!({"ok":false,"error":"The terminal control queue is full."})
            } else {
                tokio::task::spawn_blocking(move || received.recv_timeout(Duration::from_secs(10)))
                    .await
                    .ok()
                    .and_then(Result::ok)
                    .unwrap_or_else(
                        || serde_json::json!({"ok":false,"error":"The terminal did not answer."}),
                    )
            }
        }
        Err(_) => serde_json::json!({"ok":false,"error":"Invalid terminal control request."}),
    };
    let _ = reader
        .get_mut()
        .write_all(format!("{reply}\n").as_bytes())
        .await;
}
pub fn call(path: &Path, request: &Value) -> Result<Value, String> {
    runtime()?.block_on(async {
        tokio::time::timeout(Duration::from_secs(15), async {
            let name = path.to_str().ok_or("The pipe name is not Unicode.")?;
            if !name.strip_prefix(r"\\.\pipe\").is_some_and(|rest| {
                !rest.is_empty() && rest.len() <= 200 && !rest.contains(['/', '\\'])
            }) {
                return Err("Use a local Windows pipe name.".into());
            }
            let mut pipe = ClientOptions::new()
                .open(name)
                .map_err(|error| error.to_string())?;
            pipe.write_all(format!("{request}\n").as_bytes())
                .await
                .map_err(|error| error.to_string())?;
            let mut line = Vec::new();
            use tokio::io::AsyncReadExt;
            let count = BufReader::new(pipe)
                .take(256 * 1024 + 1)
                .read_until(b'\n', &mut line)
                .await
                .map_err(|error| error.to_string())?;
            if count == 0 || count > 256 * 1024 {
                return Err("Invalid terminal control reply.".into());
            }
            serde_json::from_slice(&line).map_err(|error| error.to_string())
        })
        .await
        .map_err(|_| "The terminal control call timed out.".to_string())?
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scratch_named_pipe_serves_one_request_and_releases_name() {
        let path = PathBuf::from(format!(
            r"\\.\pipe\openagents-terminal-fixture-{}",
            std::process::id()
        ));
        let listener = Listener::bind(&path).unwrap();
        let remote = path.clone();
        let client = std::thread::spawn(move || call(&remote, &serde_json::json!({"op":"status"})));
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        loop {
            if let Some((_, reply)) = listener.next() {
                reply.send(serde_json::json!({"ok":true})).unwrap();
                break;
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(client.join().unwrap().unwrap()["ok"], true);
        drop(listener);
        assert!(Listener::bind(&path).is_ok());
    }
}
