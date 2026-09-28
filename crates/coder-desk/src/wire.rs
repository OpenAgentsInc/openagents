//! The socket under both backends: connect, write one request, read one
//! answer, close.
//!
//! The desk protocol carries one JSON object per line, so its answer ends
//! at a newline. The Hyprland line protocol writes the answer and closes,
//! so its answer ends at the end of the stream. Both are local sockets
//! under the same bound on time and on size, and both run on a blocking
//! thread, which is what an async caller hands to the blocking pool.

use std::path::Path;
use std::time::Duration;

use crate::protocol::MAX_LINE_BYTES;

use crate::DeskError;

/// How much of one answer is read. A desk answers in JSON, and a thousand
/// windows is still an answer this bound holds.
pub(crate) const REPLY_LIMIT: u64 = 4 * 1024 * 1024;

/// How long the desk has to answer. It is a local socket, so a desk that
/// has not answered in this long is a desk that has stopped.
pub(crate) const ANSWER_LIMIT: Duration = Duration::from_secs(5);

/// Where one answer ends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Codec {
    /// At a newline, under [`MAX_LINE_BYTES`]. The desk protocol.
    Line,
    /// At the end of the stream, under [`REPLY_LIMIT`]. The Hyprland line
    /// protocol.
    Stream,
}

/// Whether something answers on a desk's socket. A session that ended
/// leaves its socket behind, so the test is whether something answers on
/// it and not whether the file is there.
#[cfg(unix)]
pub(crate) fn answers(socket: &Path) -> bool {
    std::os::unix::net::UnixStream::connect(socket).is_ok()
}

/// Whether something answers on a desk's socket, on a host that has no
/// such socket. A desk speaks over a Unix socket, so a host without one
/// reaches no desk.
#[cfg(not(unix))]
pub(crate) fn answers(_socket: &Path) -> bool {
    false
}

/// One request over the desk's socket, and the answer it wrote back.
#[cfg(unix)]
pub(crate) fn speak(socket: &Path, request: &str, codec: Codec) -> Result<String, DeskError> {
    use std::io::{BufRead, BufReader, Read, Write};
    use std::os::unix::net::UnixStream;

    let stream =
        UnixStream::connect(socket).map_err(|_| DeskError::Unreachable(socket.to_path_buf()))?;
    stream
        .set_read_timeout(Some(ANSWER_LIMIT))
        .and_then(|()| stream.set_write_timeout(Some(ANSWER_LIMIT)))
        .map_err(|error| {
            DeskError::Asked(format!("the desk's socket took no time limit: {error}"))
        })?;
    let line = match codec {
        Codec::Line => format!("{request}\n"),
        Codec::Stream => request.to_string(),
    };
    (&stream)
        .write_all(line.as_bytes())
        .map_err(|error| DeskError::Asked(format!("the desk was not asked: {error}")))?;
    match codec {
        Codec::Line => {
            let mut answer = Vec::new();
            BufReader::new((&stream).take(MAX_LINE_BYTES as u64))
                .read_until(b'\n', &mut answer)
                .map_err(|error| {
                    DeskError::Asked(format!("the desk stopped answering: {error}"))
                })?;
            Ok(String::from_utf8_lossy(&answer).trim_end().to_string())
        }
        Codec::Stream => {
            let mut answer = Vec::new();
            (&stream)
                .take(REPLY_LIMIT)
                .read_to_end(&mut answer)
                .map_err(|error| {
                    DeskError::Asked(format!("the desk stopped answering: {error}"))
                })?;
            Ok(String::from_utf8_lossy(&answer).into_owned())
        }
    }
}

/// One request over the desk's socket, on a host that has no such socket.
/// [`crate::Desk::found`] reaches no desk there, so nothing holds a desk to
/// ask with.
#[cfg(not(unix))]
pub(crate) fn speak(socket: &Path, _request: &str, _codec: Codec) -> Result<String, DeskError> {
    Err(DeskError::Unreachable(socket.to_path_buf()))
}
