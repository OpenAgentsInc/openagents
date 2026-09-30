//! A bounded loopback mirror of committed public results for offline previews.
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Component, PathBuf};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

pub struct Publication {
    pub base: String,
    pub cache: PathBuf,
    stop: Arc<AtomicBool>,
}

impl Publication {
    pub fn new() -> Result<Self, String> {
        let listener = std::net::TcpListener::bind("127.0.0.1:0")
            .map_err(|_| "Could not start the results fixture".to_owned())?;
        listener
            .set_nonblocking(true)
            .map_err(|_| "Could not configure the results fixture".to_owned())?;
        let address = listener
            .local_addr()
            .map_err(|_| "Could not locate the results fixture".to_owned())?;
        let root = super::store::fixture_root().join("bench/terminal-bench/published");
        let stop = Arc::new(AtomicBool::new(false));
        let cancellation = stop.clone();
        std::thread::Builder::new().name("grid-results-fixture".into()).spawn(move || {
            while !cancellation.load(Ordering::Acquire) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let _ = stream.set_read_timeout(Some(Duration::from_secs(1)));
                        let _ = stream.set_write_timeout(Some(Duration::from_secs(1)));
                        let mut reader = BufReader::new((&stream).take(8192));
                        let mut line = String::new();
                        let mut size = 0;
                        let mut path = None;
                        while size < 8192 {
                            match reader.read_line(&mut line) {
                                Ok(0) | Err(_) => break,
                                Ok(n) => { size += n; }
                            }
                            if path.is_none() { path = line.split_whitespace().nth(1).map(str::to_owned); }
                            if line.trim().is_empty() { break; }
                            line.clear();
                        }
                        let body = path.as_deref().and_then(|path| path.strip_prefix('/')).and_then(|path| path.split_once('/')).map(|(_, file)| PathBuf::from(file))
                            .filter(|file| file.components().all(|part| matches!(part, Component::Normal(_))))
                            .and_then(|file| { let file = root.join(file); let metadata = std::fs::metadata(&file).ok()?; (metadata.is_file() && metadata.len() <= 64 * 1024 * 1024).then(|| std::fs::read(file).ok()).flatten() });
                        drop(reader);
                        if let Some(body) = body {
                            let _ = write!(stream, "HTTP/1.1 200 OK\r\ncontent-length: {}\r\nconnection: close\r\n\r\n", body.len());
                            let _ = stream.write_all(&body);
                        } else { let _ = stream.write_all(b"HTTP/1.1 404 Not Found\r\ncontent-length: 0\r\nconnection: close\r\n\r\n"); }
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => std::thread::sleep(Duration::from_millis(10)),
                    Err(_) => break,
                }
            }
        }).map_err(|_| "Could not start the results fixture worker".to_owned())?;
        Ok(Self {
            base: format!("http://{address}/{{ref}}/"),
            cache: std::env::temp_dir()
                .join(format!("desktop-grid-fixture-{}", uuid::Uuid::new_v4())),
            stop,
        })
    }
}

impl Drop for Publication {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        let _ = std::fs::remove_dir_all(&self.cache);
    }
}
