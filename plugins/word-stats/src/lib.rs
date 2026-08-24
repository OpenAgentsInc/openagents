//! Text statistics, as a `packet-v0` guest plugin on the owned PDK.
//!
//! The whole ABI — `packet_alloc`, `handle_packet`, the JSON envelope, the
//! return-word packing — lives in `openagents-pdk`. This crate is one typed
//! function. Compare the demo predecessor, which carried its own allocator,
//! pointer packing, and a hand-rolled JSON scanner.
//!
//! `spin: true` loops forever, existing solely so the host's timeout bound
//! is demonstrable against a real runaway guest.

use openagents_pdk::{plugin_entry, Refusal};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
struct Input {
    text: String,
    #[serde(default)]
    spin: bool,
}

#[derive(Serialize)]
struct Output {
    bytes: usize,
    chars: usize,
    words: usize,
    lines: usize,
    longest_word: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_word: Option<TopWord>,
}

#[derive(Serialize)]
struct TopWord {
    word: String,
    count: u32,
}

fn handle(input: Input) -> Result<Output, Refusal> {
    if input.spin {
        // A runaway guest, on request, so the host's timeout is testable.
        let mut n: u64 = 0;
        loop {
            n = std::hint::black_box(n.wrapping_add(1));
        }
    }

    let text = &input.text;
    let words: Vec<&str> = text.split_whitespace().collect();
    let longest = words.iter().max_by_key(|word| word.len()).copied().unwrap_or("");

    let mut counts: Vec<(String, u32)> = Vec::new();
    for word in &words {
        let lowered: String = word
            .chars()
            .filter(|c| c.is_alphanumeric())
            .collect::<String>()
            .to_lowercase();
        if lowered.is_empty() {
            continue;
        }
        match counts.iter_mut().find(|(seen, _)| *seen == lowered) {
            Some((_, n)) => *n += 1,
            None => counts.push((lowered, 1)),
        }
    }
    let top_word = counts
        .into_iter()
        .max_by_key(|(_, n)| *n)
        .map(|(word, count)| TopWord { word, count });

    Ok(Output {
        bytes: text.len(),
        chars: text.chars().count(),
        words: words.len(),
        lines: if text.is_empty() { 0 } else { text.lines().count() },
        longest_word: longest.to_string(),
        top_word,
    })
}

plugin_entry!(handle);
