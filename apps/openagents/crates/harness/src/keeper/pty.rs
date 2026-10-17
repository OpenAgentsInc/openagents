//! The terminal keeper (#11951): a shell on a PTY that outlives the engine
//! that opened it.
//!
//! The keeper holds the PTY's master side, and the shell leads a session of
//! its own on the other side. Output is journaled as stdout frames carrying
//! base64 bytes, batched over a few milliseconds the way the engine batches a
//! terminal it holds itself. The journal keeps only the most recent output,
//! a little more than an engine shows when it picks the terminal up.
//! Engines send [`ToKeeper::Input`] and [`ToKeeper::Resize`];
//! [`ToKeeper::Finish`] hangs up the shell and removes the run folder.

use std::collections::BTreeSet;
use std::io::{BufRead, Read, Write};
use std::path::Path;
use std::time::Duration;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use portable_pty::{CommandBuilder, native_pty_system};
use tokio::sync::mpsc;

use super::protocol::{FrameBody, KeeperSignal, ToKeeper};
use super::server::{
    Connection, DIR_ENV, Event, JOURNAL, Journal, Launch, ORPHAN_ENV, PtySize, STOP_GRACE,
    Standing, deliver, finish, greet, listen, private_file, retire, signal_group,
};

/// How long output gathers before it becomes one frame.
const BATCH: Duration = Duration::from_millis(12);
/// The most output one frame carries.
const BATCH_MAX: usize = 64 * 1024;
/// Compact the journal once this much has been written to it.
const COMPACT_AT: u64 = 4 * 1024 * 1024;
/// How much journal compaction keeps: more than the 1 MiB an engine replays.
const KEEP: usize = 1536 * 1024;
/// After the shell exits, how long its last output has to arrive. A
/// background job can hold the PTY open after the shell is gone.
const EXIT_DRAIN: Duration = Duration::from_secs(2);

pub(super) async fn serve(
    dir: &Path,
    launch: &Launch,
    token: &str,
    size: PtySize,
    orphan_after: Duration,
) -> std::io::Result<()> {
    let journal_path = dir.join(JOURNAL);
    let mut journal = Journal::open(&journal_path)?;
    let pair = native_pty_system()
        .openpty(pty_size(size))
        .map_err(std::io::Error::other)?;
    let mut command = CommandBuilder::new(&launch.program);
    command.args(&launch.args);
    if let Some(cwd) = &launch.cwd {
        command.cwd(cwd);
    }
    command.env_remove(DIR_ENV);
    command.env_remove(ORPHAN_ENV);
    let mut child = match pair.slave.spawn_command(command) {
        Ok(child) => child,
        Err(error) => {
            journal.append(FrameBody::Stderr {
                line: format!("could not start {}: {error}", launch.program.display()),
            })?;
            journal.append(FrameBody::Exit {
                code: None,
                signal: None,
            })?;
            return Err(std::io::Error::other(error));
        }
    };
    drop(pair.slave);
    let pid = child.process_id().unwrap_or(0);
    let killer = child.clone_killer();
    let reader = pair
        .master
        .try_clone_reader()
        .map_err(std::io::Error::other)?;
    let writer = pair.master.take_writer().map_err(std::io::Error::other)?;
    let mut master = Some(pair.master);

    // Writes to a PTY block while the shell isn't reading, so they get a thread.
    let (input_tx, input_rx) = std::sync::mpsc::channel::<Vec<u8>>();
    std::thread::Builder::new()
        .name("keeper-pty-write".into())
        .spawn(move || write_input(writer, input_rx))?;
    let mut input = Some(input_tx);

    let (events_tx, mut events) = mpsc::unbounded_channel::<Event>();
    {
        let tx = events_tx.clone();
        std::thread::Builder::new()
            .name("keeper-pty-read".into())
            .spawn(move || read_output(reader, tx))?;
    }
    {
        let tx = events_tx.clone();
        tokio::task::spawn_blocking(move || {
            let code = child.wait().ok().map(|status| status.exit_code() as i32);
            let _ = tx.send(Event::Exited(code));
        });
    }
    listen(&launch.socket, &events_tx)?;

    let held = BTreeSet::new();
    let mut acked = 0u64;
    let mut connection: Option<Connection> = None;
    let mut next_connection = 0u64;
    let mut orphan_since = tokio::time::Instant::now();
    let mut buffer: Vec<u8> = Vec::new();
    let mut flush_at: Option<tokio::time::Instant> = None;
    let mut reading = true;
    let mut shell_exit: Option<Option<i32>> = None;
    let mut drain_until: Option<tokio::time::Instant> = None;
    let mut exit_seq: Option<u64> = None;
    let mut orphan_stop: Option<(tokio::time::Instant, KeeperSignal)> = None;

    loop {
        if !reading && exit_seq.is_none() && shell_exit.is_some() {
            if !buffer.is_empty() {
                output(&mut journal, &journal_path, &mut connection, &mut buffer)?;
            }
            let frame = journal.append(FrameBody::Exit {
                code: shell_exit.flatten(),
                signal: None,
            })?;
            exit_seq = Some(frame.seq);
            deliver(&mut connection, frame);
            if orphan_stop.is_some() {
                retire(dir, &launch.socket);
                return Ok(());
            }
        }
        let orphan_deadline = orphan_since + orphan_after;
        let event = tokio::select! {
            event = events.recv() => match event {
                Some(event) => event,
                None => break,
            },
            _ = tokio::time::sleep_until(flush_at.unwrap_or_else(tokio::time::Instant::now)),
                if flush_at.is_some() =>
            {
                flush_at = None;
                output(&mut journal, &journal_path, &mut connection, &mut buffer)?;
                continue;
            }
            _ = tokio::time::sleep_until(drain_until.unwrap_or_else(tokio::time::Instant::now)),
                if drain_until.is_some() && reading =>
            {
                reading = false;
                continue;
            }
            _ = tokio::time::sleep_until(orphan_deadline),
                if connection.is_none() && orphan_stop.is_none() =>
            {
                // No engine for the whole window: hang up the shell, as
                // closing its terminal window would, and keep the journal.
                if exit_seq.is_some() {
                    retire(dir, &launch.socket);
                    return Ok(());
                }
                hang_up(pid);
                orphan_stop = Some((tokio::time::Instant::now() + STOP_GRACE, KeeperSignal::Kill));
                continue;
            }
            _ = tokio::time::sleep_until(orphan_stop.map_or_else(tokio::time::Instant::now, |(at, _)| at)),
                if orphan_stop.is_some() && exit_seq.is_none() =>
            {
                signal_group(pid, KeeperSignal::Kill);
                orphan_stop = Some((tokio::time::Instant::now() + STOP_GRACE, KeeperSignal::Kill));
                continue;
            }
        };
        match event {
            Event::Pty(Some(bytes)) => {
                buffer.extend_from_slice(&bytes);
                if buffer.len() >= BATCH_MAX {
                    flush_at = None;
                    output(&mut journal, &journal_path, &mut connection, &mut buffer)?;
                } else if flush_at.is_none() {
                    flush_at = Some(tokio::time::Instant::now() + BATCH);
                }
            }
            Event::Pty(None) => reading = false,
            Event::Exited(code) => {
                shell_exit = Some(code);
                // Close this side so the reader sees the end once the shell's
                // side is closed too.
                master.take();
                input.take();
                drain_until = Some(tokio::time::Instant::now() + EXIT_DRAIN);
            }
            Event::Accepted(stream) => {
                next_connection += 1;
                let standing = Standing {
                    pid,
                    last_seq: journal.last_seq,
                    exited: exit_seq.is_some(),
                    acked,
                    held: &held,
                };
                if let Some(c) = greet(
                    stream,
                    next_connection,
                    token,
                    standing,
                    journal_path.clone(),
                    &events_tx,
                )
                .await
                {
                    connection = Some(c);
                }
            }
            Event::Closed(id) => {
                if connection.as_ref().is_some_and(|c| c.id == id) {
                    connection = None;
                    orphan_since = tokio::time::Instant::now();
                }
            }
            Event::Message(id, message) => {
                if connection.as_ref().is_none_or(|c| c.id != id) {
                    continue; // a replaced engine's late message
                }
                match message {
                    ToKeeper::Input { data } => {
                        if let (Some(input), Ok(bytes)) = (&input, BASE64.decode(data)) {
                            let _ = input.send(bytes);
                        }
                    }
                    ToKeeper::Resize { cols, rows } => {
                        if let Some(master) = &master {
                            let _ = master.resize(pty_size(PtySize { cols, rows }));
                        }
                    }
                    ToKeeper::Ack { seq } => acked = acked.max(seq),
                    ToKeeper::Signal { signal } => {
                        if exit_seq.is_none() {
                            signal_group(pid, signal);
                        }
                    }
                    ToKeeper::Finish => {
                        if shell_exit.is_none() {
                            let mut killer = killer;
                            // Hang up, then kill what's left of the shell's group.
                            let _ = tokio::task::spawn_blocking(move || killer.kill()).await;
                            signal_group(pid, KeeperSignal::Kill);
                        }
                        drop(master);
                        finish(dir, &launch.socket);
                        return Ok(());
                    }
                    ToKeeper::Hello { .. }
                    | ToKeeper::Stdin { .. }
                    | ToKeeper::CloseStdin
                    | ToKeeper::Hold { .. }
                    | ToKeeper::Release { .. }
                    | ToKeeper::Unknown => {}
                }
            }
            Event::Out(_) | Event::Err(_) => {}
        }
    }
    Ok(())
}

fn pty_size(size: PtySize) -> portable_pty::PtySize {
    portable_pty::PtySize {
        cols: size.cols.clamp(2, 500),
        rows: size.rows.clamp(1, 300),
        pixel_width: 0,
        pixel_height: 0,
    }
}

/// Journal the gathered output as one frame and send it to the engine.
fn output(
    journal: &mut Journal,
    path: &Path,
    connection: &mut Option<Connection>,
    buffer: &mut Vec<u8>,
) -> std::io::Result<()> {
    if buffer.is_empty() {
        return Ok(());
    }
    let frame = journal.append(FrameBody::Stdout {
        line: BASE64.encode(std::mem::take(buffer)),
    })?;
    deliver(connection, frame);
    if journal.bytes >= COMPACT_AT {
        compact(journal, path)?;
    }
    Ok(())
}

/// Keep only the journal's last [`KEEP`] bytes of whole frames. An engine
/// replaying the old file keeps reading it; the next one reads the new one.
fn compact(journal: &mut Journal, path: &Path) -> std::io::Result<()> {
    let file = std::fs::File::open(path)?;
    let lines: Vec<String> = std::io::BufReader::new(file)
        .lines()
        .collect::<Result<_, _>>()?;
    let mut kept = 0usize;
    let mut from = lines.len();
    while from > 0 && kept + lines[from - 1].len() < KEEP {
        from -= 1;
        kept += lines[from].len() + 1;
    }
    let partial = path.with_extension("jsonl.compact");
    let _ = std::fs::remove_file(&partial);
    {
        let mut out = std::io::BufWriter::new(private_file(&partial)?);
        for line in &lines[from..] {
            out.write_all(line.as_bytes())?;
            out.write_all(b"\n")?;
        }
        out.flush()?;
    }
    std::fs::rename(&partial, path)?;
    journal.file = private_file(path)?;
    journal.bytes = kept as u64;
    Ok(())
}

fn hang_up(pid: u32) {
    if pid != 0 {
        // SAFETY: the shell leads its own session and process group.
        unsafe { libc::kill(-(pid as i32), libc::SIGHUP) };
    }
}

fn read_output(mut reader: Box<dyn Read + Send>, events: mpsc::UnboundedSender<Event>) {
    let mut buf = [0u8; 8192];
    loop {
        // A closed PTY reads as an error on some platforms (EIO on Linux).
        match reader.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                if events.send(Event::Pty(Some(buf[..n].to_vec()))).is_err() {
                    return;
                }
            }
        }
    }
    let _ = events.send(Event::Pty(None));
}

fn write_input(mut writer: Box<dyn Write + Send>, input: std::sync::mpsc::Receiver<Vec<u8>>) {
    while let Ok(bytes) = input.recv() {
        if writer
            .write_all(&bytes)
            .and_then(|_| writer.flush())
            .is_err()
        {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compaction_keeps_the_latest_whole_frames() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(JOURNAL);
        let mut journal = Journal::open(&path).unwrap();
        let chunk = "x".repeat(64 * 1024);
        for _ in 0..80 {
            journal
                .append(FrameBody::Stdout {
                    line: chunk.clone(),
                })
                .unwrap();
        }
        compact(&mut journal, &path).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        let seqs: Vec<u64> = text
            .lines()
            .map(|line| {
                serde_json::from_str::<super::super::Frame>(line)
                    .unwrap()
                    .seq
            })
            .collect();
        assert!(text.len() <= KEEP && text.len() > KEEP - 70 * 1024);
        assert_eq!(seqs.last(), Some(&80));
        assert!(seqs.windows(2).all(|w| w[1] == w[0] + 1));
        // Appends go on after the kept frames.
        journal
            .append(FrameBody::Stdout { line: "y".into() })
            .unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.lines().last().unwrap().contains(r#""seq":81"#));
    }
}
