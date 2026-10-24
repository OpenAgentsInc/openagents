use super::{Result, digest, failed, valid_hash};
use std::{
    io::Write,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
use tokio_util::sync::CancellationToken;

#[derive(Clone, uniffi::Record)]
pub struct MedievalTownLoadingProgress {
    pub stage: String,
    pub fraction: f32,
    pub completed_bytes: u64,
    pub total_bytes: u64,
    pub completed_files: u32,
    pub total_files: u32,
    pub current_asset: String,
    pub error: Option<String>,
    pub finished: bool,
}
impl Default for MedievalTownLoadingProgress {
    fn default() -> Self {
        Self {
            stage: "Opening the town".into(),
            fraction: 0.,
            completed_bytes: 0,
            total_bytes: 0,
            completed_files: 0,
            total_files: 0,
            current_asset: String::new(),
            error: None,
            finished: false,
        }
    }
}
struct ProgressState {
    value: MedievalTownLoadingProgress,
    bytes: Vec<u64>,
    files: Vec<bool>,
}
pub(super) struct Progress {
    generation: AtomicU64,
    state: Mutex<ProgressState>,
}
#[derive(Clone)]
pub(super) struct Session {
    pub generation: u64,
    pub cancel: CancellationToken,
    progress: Arc<Progress>,
}
impl Progress {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            generation: AtomicU64::new(0),
            state: Mutex::new(ProgressState {
                value: Default::default(),
                bytes: Vec::new(),
                files: Vec::new(),
            }),
        })
    }
    pub fn begin(self: &Arc<Self>) -> Session {
        let generation = self.generation.fetch_add(1, Ordering::AcqRel) + 1;
        *self.state.lock().unwrap() = ProgressState {
            value: Default::default(),
            bytes: Vec::new(),
            files: Vec::new(),
        };
        Session {
            generation,
            cancel: CancellationToken::new(),
            progress: self.clone(),
        }
    }
    pub fn snapshot(&self) -> MedievalTownLoadingProgress {
        self.state.lock().unwrap().value.clone()
    }
    pub fn stop(&self) {
        self.generation.fetch_add(1, Ordering::AcqRel);
        let mut state = self.state.lock().unwrap();
        state.value.stage = "Loading stopped".into();
        state.value.error = None;
        state.value.finished = false;
    }
    pub fn finish(&self, generation: u64) {
        if generation != self.generation.load(Ordering::Acquire) {
            return;
        }
        let mut state = self.state.lock().unwrap();
        if generation != self.generation.load(Ordering::Acquire) {
            return;
        }
        if state.value.finished {
            return;
        }
        state.value.stage = "Ready".into();
        state.value.current_asset.clear();
        state.value.fraction = 1.;
        state.value.finished = true;
    }
}
impl Session {
    fn update(&self, f: impl FnOnce(&mut ProgressState)) {
        if self.cancel.is_cancelled()
            || self.generation != self.progress.generation.load(Ordering::Acquire)
        {
            return;
        }
        let mut state = self.progress.state.lock().unwrap();
        if self.generation == self.progress.generation.load(Ordering::Acquire)
            && !self.cancel.is_cancelled()
        {
            f(&mut state);
        }
    }
    pub fn check(&self) -> Result<()> {
        if self.cancel.is_cancelled()
            || self.generation != self.progress.generation.load(Ordering::Acquire)
        {
            Err(failed("Loading stopped."))
        } else {
            Ok(())
        }
    }
    pub fn files(&self, sizes: &[usize]) {
        self.update(|state| {
            state.value.total_bytes = sizes.iter().map(|n| *n as u64).sum();
            state.value.completed_bytes = 0;
            state.value.total_files = sizes.len() as u32;
            state.value.completed_files = 0;
            state.bytes = vec![0; sizes.len()];
            state.files = vec![false; sizes.len()];
        });
    }
    pub fn phase(&self, stage: &str, fraction: f32, asset: &str) {
        self.update(|state| {
            state.value.stage = stage.into();
            state.value.fraction = fraction.clamp(0., 0.99);
            state.value.current_asset = asset.into();
        });
    }
    pub fn received(&self, index: Option<usize>, bytes: usize, complete: bool, asset: &str) {
        self.update(|state| {
            state.value.stage = "Downloading the town".into();
            state.value.current_asset = asset.into();
            if let Some(index) = index {
                state.bytes[index] = bytes as u64;
                state.files[index] = complete;
                state.value.completed_bytes = state.bytes.iter().sum();
                state.value.completed_files =
                    state.files.iter().filter(|done| **done).count() as u32;
                state.value.fraction = 0.02
                    + 0.78 * state.value.completed_bytes as f32
                        / state.value.total_bytes.max(1) as f32;
            } else {
                state.value.completed_bytes = bytes as u64;
            }
        });
    }
    pub fn status(&self, stage: &str, asset: &str) {
        self.update(|state| {
            state.value.stage = stage.into();
            state.value.current_asset = asset.into();
        });
    }
    pub fn retry(&self, asset: &str, attempt: u32) {
        self.update(|state| {
            state.value.stage = format!("Trying the download again ({attempt}/2)");
            state.value.current_asset = asset.into();
        });
    }
    pub fn fail(&self, message: &str) {
        self.update(|state| {
            state.value.stage = "Couldn’t open the town".into();
            state.value.error = Some(message.into());
            state.value.finished = false;
        });
    }
}
#[derive(Clone, Copy)]
pub(super) struct DownloadPolicy {
    pub attempts: u32,
    pub stall: Duration,
    pub total: Duration,
}
impl Default for DownloadPolicy {
    fn default() -> Self {
        Self {
            attempts: 2,
            stall: Duration::from_secs(20),
            total: Duration::from_secs(600),
        }
    }
}
static CACHE_WRITE: AtomicU64 = AtomicU64::new(0);
fn verified(path: &Path, expected: &str, size: Option<usize>) -> Option<Vec<u8>> {
    let length = std::fs::metadata(path).ok()?.len();
    if length > size.unwrap_or(8 * 1024 * 1024) as u64 || size.is_some_and(|n| n as u64 != length) {
        return None;
    }
    let bytes = std::fs::read(path).ok()?;
    (size.is_none_or(|n| n == bytes.len()) && digest(&bytes) == expected).then_some(bytes)
}
fn atomic_save(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().unwrap();
    std::fs::create_dir_all(parent)
        .map_err(|_| failed("The town could not be saved on this device."))?;
    let temporary = parent.join(format!(
        ".{}.{}.{}.download",
        path.file_name().unwrap().to_string_lossy(),
        std::process::id(),
        CACHE_WRITE.fetch_add(1, Ordering::Relaxed)
    ));
    struct Remove(PathBuf);
    impl Drop for Remove {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    let _remove = Remove(temporary.clone());
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|_| failed("The town could not be saved on this device."))?;
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .and_then(|_| std::fs::rename(&temporary, path))
        .map_err(|_| failed("The town could not be saved on this device."))
}
fn cached(root: &Path, expected: &str, size: Option<usize>) -> Result<Option<Vec<u8>>> {
    let path = root.join("files").join(expected);
    if let Some(bytes) = verified(&path, expected, size) {
        return Ok(Some(bytes));
    }
    // Older releases saved identical files below each map version. Check the
    // contents before moving a copy into the shared file cache.
    if let Ok(entries) = std::fs::read_dir(root) {
        for entry in entries.filter_map(std::result::Result::ok).take(64) {
            let name = entry.file_name().to_string_lossy().into_owned();
            if !valid_hash(&name) || !entry.file_type().is_ok_and(|kind| kind.is_dir()) {
                continue;
            }
            let old = entry.path().join(if name == expected {
                "manifest.json"
            } else {
                expected
            });
            if let Some(bytes) = verified(&old, expected, size) {
                atomic_save(&path, &bytes)?;
                return Ok(Some(bytes));
            }
        }
    }
    Ok(None)
}
pub(super) async fn download(
    client: &reqwest::Client,
    url: &reqwest::Url,
    expected: &str,
    size: Option<usize>,
    root: &Path,
    session: &Session,
    index: Option<usize>,
    asset: &str,
    policy: DownloadPolicy,
) -> Result<Vec<u8>> {
    session.check()?;
    session.status("Checking saved files", asset);
    if let Some(bytes) = cached(root, expected, size)? {
        session.check()?;
        session.received(index, bytes.len(), true, asset);
        return Ok(bytes);
    }
    let maximum = size.unwrap_or(8 * 1024 * 1024);
    let mut last = String::new();
    for attempt in 0..policy.attempts {
        session.check()?;
        session.received(index, 0, false, asset);
        if attempt > 0 {
            session.retry(asset, attempt + 1);
        }
        let transfer = async {
            let mut response = tokio::time::timeout(policy.stall, client.get(url.clone()).send())
                .await
                .map_err(|_| "download did not start".to_string())?
                .map_err(|e| e.to_string())?
                .error_for_status()
                .map_err(|e| e.to_string())?;
            if response
                .content_length()
                .is_some_and(|n| n > maximum as u64)
            {
                return Err("file is larger than expected".into());
            }
            let mut bytes = Vec::with_capacity(maximum.min(8 * 1024 * 1024));
            loop {
                let chunk = tokio::time::timeout(policy.stall, response.chunk())
                    .await
                    .map_err(|_| "download stopped receiving bytes".to_string())?
                    .map_err(|e| e.to_string())?;
                let Some(chunk) = chunk else {
                    break;
                };
                if bytes.len() + chunk.len() > maximum {
                    return Err("file is larger than expected".into());
                }
                bytes.extend_from_slice(&chunk);
                session.received(index, bytes.len(), false, asset);
            }
            if size.is_some_and(|n| n != bytes.len()) || digest(&bytes) != expected {
                return Err("file did not finish downloading".into());
            }
            Ok::<Vec<u8>, String>(bytes)
        };
        let outcome = tokio::select! { biased;
            _ = session.cancel.cancelled() => return Err(failed("Loading stopped.")),
            result = tokio::time::timeout(policy.total, transfer) => result.unwrap_or_else(|_| Err("download took too long".into())),
        };
        match outcome {
            Ok(bytes) => {
                session.check()?;
                atomic_save(&root.join("files").join(expected), &bytes)?;
                session.received(index, bytes.len(), true, asset);
                return Ok(bytes);
            }
            Err(error) => last = error,
        }
    }
    eprintln!(
        "Medieval Town download {}: {}",
        url.path(),
        last.chars().take(160).collect::<String>()
    );
    Err(failed(
        "The town stopped downloading. Check your connection and try again.",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{Read, Write},
        net::{TcpListener, TcpStream},
        sync::atomic::{AtomicBool, AtomicUsize},
    };
    struct Server {
        url: reqwest::Url,
        requests: Arc<AtomicUsize>,
        stop: Arc<AtomicBool>,
        thread: Option<std::thread::JoinHandle<()>>,
    }
    impl Server {
        fn new(handler: impl Fn(TcpStream, usize, &AtomicBool) + Send + 'static) -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            listener.set_nonblocking(true).unwrap();
            let url =
                reqwest::Url::parse(&format!("http://{}/part", listener.local_addr().unwrap()))
                    .unwrap();
            let requests = Arc::new(AtomicUsize::new(0));
            let stop = Arc::new(AtomicBool::new(false));
            let (count, quit) = (requests.clone(), stop.clone());
            let thread = std::thread::spawn(move || {
                while !quit.load(Ordering::Acquire) {
                    if let Ok((mut stream, _)) = listener.accept() {
                        let mut request = [0u8; 2048];
                        let _ = stream.read(&mut request);
                        handler(stream, count.fetch_add(1, Ordering::AcqRel), &quit);
                    } else {
                        std::thread::sleep(Duration::from_millis(2));
                    }
                }
            });
            Self {
                url,
                requests,
                stop,
                thread: Some(thread),
            }
        }
    }
    impl Drop for Server {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::Release);
            self.thread.take().unwrap().join().unwrap();
        }
    }
    fn response(stream: &mut TcpStream, declared: usize, body: &[u8]) {
        let _ = write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Length: {declared}\r\nConnection: close\r\n\r\n"
        );
        let _ = stream.write_all(body);
        let _ = stream.flush();
    }
    fn policy() -> DownloadPolicy {
        DownloadPolicy {
            attempts: 2,
            stall: Duration::from_millis(300),
            total: Duration::from_secs(2),
        }
    }
    fn client() -> reqwest::Client {
        reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap()
    }
    #[tokio::test]
    async fn streaming_progress_counts_real_chunks_and_waits_for_the_first_frame() {
        let body = vec![7; 8192];
        let expected = digest(&body);
        let sent = body.clone();
        let server = Server::new(move |mut stream, _, stop| {
            response(&mut stream, sent.len(), &sent[..4096]);
            for _ in 0..15 {
                if stop.load(Ordering::Acquire) {
                    return;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            let _ = stream.write_all(&sent[4096..]);
        });
        let cache = tempfile::tempdir().unwrap();
        let progress = Progress::new();
        let session = progress.begin();
        session.files(&[body.len()]);
        let (owned, url, path) = (session.clone(), server.url.clone(), cache.path().to_owned());
        let task = tokio::spawn(async move {
            download(
                &client(),
                &url,
                &expected,
                Some(8192),
                &path,
                &owned,
                Some(0),
                "Buildings",
                policy(),
            )
            .await
        });
        let mut observed = false;
        for _ in 0..50 {
            let value = progress.snapshot();
            if value.completed_bytes > 0 && value.completed_bytes < 8192 {
                observed = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        assert!(
            observed,
            "Progress must update before the entire body arrives"
        );
        assert_eq!(task.await.unwrap().unwrap(), body);
        let value = progress.snapshot();
        assert_eq!(value.completed_bytes, 8192);
        assert_eq!(value.completed_files, 1);
        assert!(!value.finished && value.fraction < 1.);
        session.phase("Drawing the town", 1., "");
        assert!(progress.snapshot().fraction < 1.);
        progress.finish(session.generation);
        assert!(progress.snapshot().finished);
    }
    #[tokio::test]
    async fn verified_older_files_are_reused_without_a_network_request() {
        let body = b"unchanged building file";
        let hash = digest(body);
        let cache = tempfile::tempdir().unwrap();
        let old = cache.path().join("a".repeat(64));
        std::fs::create_dir_all(&old).unwrap();
        std::fs::write(old.join(&hash), body).unwrap();
        let server = Server::new(|_, _, _| panic!("Verified cache reuse must not download"));
        let p = Progress::new();
        let session = p.begin();
        session.files(&[body.len()]);
        assert_eq!(
            download(
                &client(),
                &server.url,
                &hash,
                Some(body.len()),
                cache.path(),
                &session,
                Some(0),
                "Buildings",
                policy()
            )
            .await
            .unwrap(),
            body
        );
        assert_eq!(server.requests.load(Ordering::Acquire), 0);
        assert_eq!(
            std::fs::read(cache.path().join("files").join(hash)).unwrap(),
            body
        );
    }
    #[tokio::test]
    async fn a_bad_hash_is_retried_and_never_saved_as_a_good_file() {
        let body = b"good";
        let hash = digest(body);
        let server = Server::new(|mut stream, attempt, _| {
            response(&mut stream, 4, if attempt == 0 { b"nope" } else { b"good" })
        });
        let cache = tempfile::tempdir().unwrap();
        let p = Progress::new();
        let session = p.begin();
        session.files(&[4]);
        assert_eq!(
            download(
                &client(),
                &server.url,
                &hash,
                Some(4),
                cache.path(),
                &session,
                Some(0),
                "Buildings",
                policy()
            )
            .await
            .unwrap(),
            body
        );
        assert_eq!(server.requests.load(Ordering::Acquire), 2);
        assert_eq!(p.snapshot().completed_bytes, 4);
        assert_eq!(p.snapshot().completed_files, 1);
    }
    #[tokio::test]
    async fn truncated_bodies_fail_after_two_attempts_without_a_partial_cache() {
        let hash = digest(b"complete");
        let server = Server::new(|mut stream, _, _| response(&mut stream, 8, b"short"));
        let cache = tempfile::tempdir().unwrap();
        let p = Progress::new();
        let session = p.begin();
        session.files(&[8]);
        assert!(
            download(
                &client(),
                &server.url,
                &hash,
                Some(8),
                cache.path(),
                &session,
                Some(0),
                "Buildings",
                policy()
            )
            .await
            .is_err()
        );
        assert_eq!(server.requests.load(Ordering::Acquire), 2);
        assert!(!cache.path().join("files").join(hash).exists());
        assert_eq!(p.snapshot().completed_files, 0);
    }
    #[tokio::test]
    async fn cancellation_interrupts_an_active_body_and_old_progress_cannot_replace_a_retry() {
        let server = Server::new(|mut stream, _, stop| {
            response(&mut stream, 100, b"x");
            while !stop.load(Ordering::Acquire) {
                std::thread::sleep(Duration::from_millis(5));
            }
        });
        let cache = tempfile::tempdir().unwrap();
        let p = Progress::new();
        let session = p.begin();
        session.files(&[100]);
        let (owned, url, path) = (session.clone(), server.url.clone(), cache.path().to_owned());
        let task = tokio::spawn(async move {
            download(
                &client(),
                &url,
                &digest(&[0; 100]),
                Some(100),
                &path,
                &owned,
                Some(0),
                "Buildings",
                policy(),
            )
            .await
        });
        for _ in 0..40 {
            if p.snapshot().completed_bytes > 0 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        session.cancel.cancel();
        assert!(
            tokio::time::timeout(Duration::from_millis(100), task)
                .await
                .unwrap()
                .unwrap()
                .is_err()
        );
        p.stop();
        let retry = p.begin();
        retry.phase("Reading the town map", 0., "Town map");
        session.received(Some(0), 100, true, "Old file");
        session.fail("Old failure");
        p.finish(session.generation);
        assert_eq!(p.snapshot().stage, "Reading the town map");
        assert!(!p.snapshot().finished);
        assert!(p.snapshot().error.is_none());
    }
}
