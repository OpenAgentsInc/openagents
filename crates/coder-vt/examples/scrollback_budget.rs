//! Measures what a host's retained terminal history costs, and what idle
//! history compression would save (#10692).
//!
//! The workload is the host's own bound: 16 terminals (`coder-pty`'s
//! `terminals_max`), each keeping 1,000 history lines (`coder-host`'s
//! `TERMINAL_HISTORY`) at 120 columns, filled with build-log, colored
//! listing, and test output. It reports the cell grid's heap, the encoded
//! `HISTORY` records a snapshot already writes, those records under
//! DEFLATE level 1, the time to compress and restore one page, and the
//! parse, snapshot, and history-read percentiles the change would have to
//! keep.
//!
//! Run with `cargo run --release -p coder-vt --example scrollback_budget`.

use std::time::{Duration, Instant};

use coder_pty::ext::{Record, encode_stream};
use coder_pty::wire::TerminalRef;
use coder_vt::{Binding, Cell, Terminal};
use flate2::Compression;
use flate2::read::DeflateDecoder;
use flate2::write::DeflateEncoder;
use std::io::{Read, Write};

const TERMINALS: usize = 16;
const SCROLLBACK: usize = 1000;
const ROWS: usize = 40;
const COLS: usize = 120;

/// One frame of realistic output, varied by `n`.
fn frame(n: usize) -> String {
    match n % 4 {
        0 => format!(
            "   \x1b[1;32mCompiling\x1b[0m crate-{n} v0.{}.{} (/home/user/work/crates/crate-{n})\r\n",
            n % 7,
            n % 13
        ),
        1 => format!(
            "\x1b[34mdrwxr-xr-x\x1b[0m  12 user  staff   384 Oct  6 01:{:02} \x1b[1;34msrc-{n}\x1b[0m\r\n",
            n % 60
        ),
        2 => format!("test tests::case_{n}_returns_the_expected_value ... \x1b[32mok\x1b[0m\r\n"),
        _ => format!(
            "warning: unused variable `value_{n}` in crates/crate-{}/src/lib.rs:{}:{}\r\n",
            n % 40,
            n % 900,
            n % 80
        ),
    }
}

fn percentile(samples: &mut [Duration], p: f64) -> Duration {
    samples.sort();
    let index = ((samples.len() as f64 - 1.0) * p).round() as usize;
    samples[index]
}

fn heap(terminal: &Terminal) -> usize {
    terminal
        .scrollback()
        .map(|row| {
            std::mem::size_of_val(row)
                + row.cells.capacity() * std::mem::size_of::<Cell>()
                + row
                    .cells
                    .iter()
                    .map(|cell| cell.combining.capacity() * 4)
                    .sum::<usize>()
        })
        .sum()
}

fn binding(index: usize) -> Binding {
    Binding {
        terminal: TerminalRef {
            generation: "1".repeat(64),
            terminal: format!("{index:064x}"),
        },
        through: 1,
        exit: None,
    }
}

fn main() {
    let mut terminals = Vec::new();
    let mut parse = Vec::new();
    let mut n = 0;
    for _ in 0..TERMINALS {
        let mut terminal = Terminal::new(ROWS, COLS, SCROLLBACK);
        // Enough output to fill the history twice over.
        for _ in 0..(SCROLLBACK + ROWS) * 2 {
            let bytes = frame(n);
            n += 1;
            let started = Instant::now();
            terminal.feed(bytes.as_bytes());
            parse.push(started.elapsed());
        }
        terminals.push(terminal);
    }

    let heap_total: usize = terminals.iter().map(heap).sum();
    let mut encoded_total = 0;
    let mut deflated_total = 0;
    let mut compress = Vec::new();
    let mut restore = Vec::new();
    let mut history = Vec::new();
    let mut snapshot = Vec::new();
    for (index, terminal) in terminals.iter_mut().enumerate() {
        let binding = binding(index);
        let end = terminal.history_dropped() + terminal.scrollback_len() as u64;
        let started = Instant::now();
        let records = terminal
            .history_stream(&binding, 1, end, SCROLLBACK as u64)
            .expect("a history stream");
        history.push(started.elapsed());
        let pages: Vec<Record> = records
            .into_iter()
            .filter(|record| matches!(record, Record::History(_)))
            .collect();
        for page in pages {
            let bytes = encode_stream(std::slice::from_ref(&page));
            encoded_total += bytes.len();
            let started = Instant::now();
            let mut encoder = DeflateEncoder::new(Vec::new(), Compression::fast());
            encoder.write_all(&bytes).unwrap();
            let deflated = encoder.finish().unwrap();
            compress.push(started.elapsed());
            deflated_total += deflated.len();
            let started = Instant::now();
            let mut back = Vec::new();
            DeflateDecoder::new(&deflated[..])
                .read_to_end(&mut back)
                .unwrap();
            restore.push(started.elapsed());
            assert_eq!(back, bytes, "a page round-trips exactly");
        }
        let started = Instant::now();
        terminal.snapshot(&binding).expect("a snapshot");
        snapshot.push(started.elapsed());
    }

    let mib = |bytes: usize| bytes as f64 / (1024.0 * 1024.0);
    println!("workload: {TERMINALS} terminals x {SCROLLBACK} history lines x {COLS} columns");
    println!("size_of::<Cell>() = {} bytes", std::mem::size_of::<Cell>());
    println!("cell-grid history heap: {:.1} MiB", mib(heap_total));
    println!(
        "encoded HISTORY records: {:.2} MiB ({:.1}x smaller)",
        mib(encoded_total),
        heap_total as f64 / encoded_total as f64
    );
    println!(
        "encoded, DEFLATE level 1: {:.2} MiB ({:.1}x smaller than the grid)",
        mib(deflated_total),
        heap_total as f64 / deflated_total as f64
    );
    println!(
        "parse per frame: p50 {:?} p99 {:?}",
        percentile(&mut parse, 0.5),
        percentile(&mut parse, 0.99)
    );
    println!(
        "compress per page: p50 {:?} p99 {:?}; restore per page: p50 {:?} p99 {:?}",
        percentile(&mut compress, 0.5),
        percentile(&mut compress, 0.99),
        percentile(&mut restore, 0.5),
        percentile(&mut restore, 0.99)
    );
    println!(
        "history read of {SCROLLBACK} rows: p50 {:?} p99 {:?}",
        percentile(&mut history, 0.5),
        percentile(&mut history, 0.99)
    );
    println!(
        "snapshot (attach): p50 {:?} p99 {:?}",
        percentile(&mut snapshot, 0.5),
        percentile(&mut snapshot, 0.99)
    );
}
