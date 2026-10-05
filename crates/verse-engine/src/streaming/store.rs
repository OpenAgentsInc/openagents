//! Explicit-root chunk I/O and cooking. No URLs, scripts, or home-directory defaults.
use super::{Decoded, Descriptor, Manifest, Ticket};
use std::{
    fs::File,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex, mpsc},
    thread,
};

#[derive(Clone)]
pub struct Store {
    #[cfg(unix)]
    root: Arc<File>,
    #[cfg(not(unix))]
    root: PathBuf,
}
impl Store {
    pub fn open(root: &Path) -> Result<Self, String> {
        let metadata = std::fs::symlink_metadata(root).map_err(|e| e.to_string())?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err("Cooked source root must be a real directory".into());
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            let file = std::fs::OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
                .open(root)
                .map_err(|e| e.to_string())?;
            Ok(Self {
                root: Arc::new(file),
            })
        }
        #[cfg(not(unix))]
        {
            Ok(Self {
                root: root.to_owned(),
            })
        }
    }
    pub fn read(&self, descriptor: &Descriptor) -> Result<Decoded, String> {
        // Shape and exact extent are admitted before an allocation or filename lookup.
        descriptor
            .payload
            .bytes()
            .checked_add(32)
            .filter(|n| *n == descriptor.encoded_bytes && *n <= super::format::MAX_CHUNK_BYTES)
            .ok_or("Invalid source chunk extent")?;
        if descriptor.sha256.len() != 64
            || !descriptor
                .sha256
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err("Invalid source chunk digest".into());
        }
        let name = format!("{}.vsc", descriptor.sha256);
        #[cfg(unix)]
        let mut file = crate::loading::open_texture(&self.root, &name)?;
        #[cfg(not(unix))]
        let mut file = {
            let path = self.root.join(&name);
            let metadata = std::fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
            if !metadata.is_file() || metadata.file_type().is_symlink() {
                return Err("Cooked source must be a regular file".into());
            }
            File::open(path).map_err(|e| e.to_string())?
        };
        let metadata = file.metadata().map_err(|e| e.to_string())?;
        if !metadata.is_file() || metadata.len() != descriptor.encoded_bytes {
            return Err("Cooked source size differs from its manifest".into());
        }
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(descriptor.encoded_bytes as usize)
            .map_err(|_| "Cannot allocate admitted chunk")?;
        bytes.resize(descriptor.encoded_bytes as usize, 0);
        file.read_exact(&mut bytes).map_err(|e| e.to_string())?;
        let mut extra = [0];
        if file.read(&mut extra).map_err(|e| e.to_string())? != 0 {
            return Err("Cooked source grew beyond its admitted size".into());
        }
        Decoded::decode(bytes, descriptor)
    }
}
/// The cooker verifies its own output before writing a content-addressed file.
pub fn install(root: &Path, descriptor: &Descriptor, bytes: &[u8]) -> Result<PathBuf, String> {
    Decoded::decode(bytes.to_vec(), descriptor)?;
    std::fs::create_dir_all(root).map_err(|e| e.to_string())?;
    Store::open(root)?;
    let path = root.join(format!("{}.vsc", descriptor.sha256));
    // Publish a fully synced file without replacing another writer's digest.
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let temporary = Temporary(root.join(format!(
        ".vsc-{}-{}-{nonce}",
        descriptor.sha256,
        std::process::id()
    )));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary.0)
        .map_err(|e| e.to_string())?;
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|e| e.to_string())?;
    match std::fs::hard_link(&temporary.0, &path) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            Store::open(root)?.read(descriptor)?;
        }
        Err(e) => return Err(e.to_string()),
    }
    Ok(path)
}
struct Temporary(PathBuf);
impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

struct Job {
    ticket: Ticket,
    descriptor: Descriptor,
    source: Store,
}
/// A fixed worker set and bounded queues. Canceled jobs remain memory-accounted
/// by `Residency` until a result is delivered; workers never touch a render device.
pub struct Workers {
    send: Option<mpsc::SyncSender<Job>>,
    receive: mpsc::Receiver<(Ticket, Result<Decoded, String>)>,
}
impl Workers {
    pub fn new(count: usize) -> Result<Self, String> {
        if !(1..=8).contains(&count) {
            return Err("Invalid source worker count".into());
        }
        let (send, input) = mpsc::sync_channel::<Job>(count);
        let input = Arc::new(Mutex::new(input));
        let (output, receive) = mpsc::sync_channel(count);
        for index in 0..count {
            let input = input.clone();
            let output = output.clone();
            thread::Builder::new()
                .name(format!("verse-chunk-{index}"))
                .spawn(move || {
                    loop {
                        let job = input.lock().unwrap_or_else(|e| e.into_inner()).recv();
                        let Ok(job) = job else {
                            break;
                        };
                        let result = job.source.read(&job.descriptor);
                        if output.send((job.ticket, result)).is_err() {
                            break;
                        }
                    }
                })
                .map_err(|e| e.to_string())?;
        }
        Ok(Self {
            send: Some(send),
            receive,
        })
    }
    pub fn submit(&self, ticket: Ticket, manifest: &Manifest, source: Store) -> Result<(), String> {
        let descriptor = manifest
            .chunks
            .get(ticket.id())
            .ok_or("Source ticket is not in the current manifest")?
            .clone();
        self.send
            .as_ref()
            .ok_or("Source workers are closed")?
            .try_send(Job {
                ticket,
                descriptor,
                source,
            })
            .map_err(|_| "Source worker queue is full or closed".into())
    }
    pub fn poll(&self) -> Option<(Ticket, Result<Decoded, String>)> {
        self.receive.try_recv().ok()
    }
}
impl Drop for Workers {
    fn drop(&mut self) {
        self.send.take();
    }
}
