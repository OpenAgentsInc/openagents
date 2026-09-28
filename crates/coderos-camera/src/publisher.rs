//! The hands socket: one line a frame to every reader.
//!
//! [`Publisher::bind`] listens and keeps every reader that connects.
//! [`Publisher::publish`] writes one line to each of them and drops a
//! reader whose socket is full, because a reader that has stopped reading
//! would otherwise hold the tracker's thread.

use std::io::{ErrorKind, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread;

pub struct Publisher {
    readers: Arc<Mutex<Vec<UnixStream>>>,
}

impl Publisher {
    /// Binds `path`, removing a socket a dead daemon left.
    pub fn bind(path: &Path) -> Result<Publisher, String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|err| format!("{}: {err}", parent.display()))?;
        }
        let _ = std::fs::remove_file(path);
        let listener =
            UnixListener::bind(path).map_err(|err| format!("{}: {err}", path.display()))?;
        let readers: Arc<Mutex<Vec<UnixStream>>> = Arc::new(Mutex::new(Vec::new()));
        let kept = Arc::clone(&readers);
        thread::Builder::new()
            .name("camera-hands-accept".into())
            .spawn(move || {
                for stream in listener.incoming() {
                    let Ok(stream) = stream else { continue };
                    if stream.set_nonblocking(true).is_err() {
                        continue;
                    }
                    if let Ok(mut readers) = kept.lock() {
                        readers.push(stream);
                    }
                }
            })
            .map_err(|err| format!("hands thread: {err}"))?;
        Ok(Publisher { readers })
    }

    /// How many readers are connected.
    #[cfg(test)]
    pub fn readers(&self) -> usize {
        self.readers.lock().map(|r| r.len()).unwrap_or(0)
    }

    /// The count, shareable with whoever reports it.
    pub fn reader_count(&self) -> Arc<Mutex<Vec<UnixStream>>> {
        Arc::clone(&self.readers)
    }

    /// Writes `line` to every reader, dropping the ones that are gone or
    /// full.
    pub fn publish(&self, line: &str) {
        let Ok(mut readers) = self.readers.lock() else {
            return;
        };
        readers.retain_mut(|reader| match reader.write_all(line.as_bytes()) {
            Ok(()) => true,
            Err(err) if err.kind() == ErrorKind::WouldBlock => false,
            Err(_) => false,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader};
    use std::time::{Duration, Instant};

    #[test]
    fn a_reader_gets_every_line_and_a_closed_reader_is_dropped() {
        let dir = std::env::temp_dir().join(format!("coderos-camera-hands-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let socket = dir.join("hands.sock");
        let publisher = Publisher::bind(&socket).expect("bind");
        let reader = UnixStream::connect(&socket).expect("connect");
        let deadline = Instant::now() + Duration::from_secs(5);
        while publisher.readers() == 0 && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(publisher.readers(), 1);
        publisher.publish("{\"a\":1}\n");
        publisher.publish("{\"a\":2}\n");
        let mut lines = BufReader::new(&reader);
        let mut line = String::new();
        lines.read_line(&mut line).expect("first");
        assert_eq!(line, "{\"a\":1}\n");
        line.clear();
        lines.read_line(&mut line).expect("second");
        assert_eq!(line, "{\"a\":2}\n");
        drop(lines);
        drop(reader);
        // The next writes find the peer gone and drop it.
        for _ in 0..5 {
            publisher.publish("{\"a\":3}\n");
            thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(publisher.readers(), 0);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
