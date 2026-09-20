//! Draws every suite item from several seed blocks and records what came
//! back, one line per draw.
//!
//! `lev-seed-sweep` measured accuracy and the mean top share and printed a
//! document. Neither the per-item draws nor the certainty bands survived the
//! run, so nothing else could be computed from it afterwards. A calibration
//! metric is not a mean over items — ECE is computed inside bins, and a map
//! has to be fitted on one split and scored on another — so measuring its
//! block-to-block spread means keeping the draws and computing from them.
//!
//! This binary asks the door and writes the rows. It scores nothing.
//! `gym spread` reads the rows it writes and produces the document, so the
//! analysis can be corrected without spending another two hours of device
//! time.
//!
//! The certainty band comes from one greedy call per item, which is what
//! produced the band numbers already recorded. Greedy decoding does not read
//! the seed block, so the band is drawn once per door and reused across
//! blocks rather than redrawn eight times.
//!
//! # This reads items `support-v2-three-way` locks, and says how many
//!
//! `support-v2` and `support-v2-three-way` are the same 196 items under two
//! partitionings, so a tool reading the older file can spend the newer
//! file's locked partition. `lev-band` and `training/lev-adapter/convert.py`
//! hold those items back. This one does not, and reports the count instead.
//!
//! The reason is what the sweep is for. It measures how far a metric moves
//! when only the seed block moves, on an unchanged door, so that published
//! claims can be judged against that movement. Those claims were made on
//! `support-v2`'s 98 evaluation items, and a floor measured on a different
//! 79 of them is not the floor those claims need. Nothing here is fitted,
//! tuned, or selected: the door does not change between blocks, so no
//! decision can leak from the items into a choice. It is still a read, and a
//! run says how many locked items it read so the fact travels with the
//! numbers.
//!
//! Run it with the helper built:
//!
//! ```text
//! ./scripts/build-lev-bridge.sh
//! cargo run --release -p lev --bin lev-calibration-sweep -- \
//!     --out crates/gym/results/support-v2-calibration-blocks.jsonl \
//!     --door lev-base --split evaluation --blocks 8
//! ```
//!
//! The run appends, skips any draw the file already holds, and can be
//! interrupted: what it measured stays measured.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

use lev::api::{Extensions, SystemOneRequest};
use lev::bridge::{Bridge, Call, Pool, Sampling};
use lev::estimator::l2_pool_with;
use lev::schema::{BANDS, compile};
use lev::suite::{Item, Suite};
use serde::{Deserialize, Serialize};

const SUITE: &str = include_str!("../../suites/support-v2.json");

/// The schema tag every recorded draw carries.
const SCHEMA: &str = "openagents.gym.block_draw.v1";

/// One item, answered once, from one seed block.
#[derive(Clone, Debug, Deserialize, Serialize)]
struct Draw {
    schema: String,
    suite: String,
    door: String,
    adapter: Option<String>,
    base_signature: String,
    split: String,
    family: String,
    item: String,
    block: u64,
    samples: u64,
    top: f64,
    choice: String,
    truth: String,
    correct: bool,
    band: Option<String>,
    refused: u64,
}

struct Options {
    out: String,
    door: String,
    adapter: Option<String>,
    split: String,
    blocks: u64,
    samples: u64,
    helpers: usize,
    band: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            out: String::new(),
            door: "lev-base".to_string(),
            adapter: None,
            split: "evaluation".to_string(),
            blocks: 8,
            samples: 8,
            helpers: 4,
            band: true,
        }
    }
}

fn read_options() -> Options {
    let mut options = Options::default();
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--out" => options.out = args.next().unwrap_or_default(),
            "--door" => options.door = args.next().unwrap_or(options.door),
            "--adapter" => options.adapter = args.next().filter(|value| !value.is_empty()),
            "--split" => options.split = args.next().unwrap_or(options.split),
            "--blocks" => {
                options.blocks = args
                    .next()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(options.blocks);
            }
            "--samples" => {
                options.samples = args
                    .next()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(options.samples);
            }
            "--helpers" => {
                options.helpers = args
                    .next()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(options.helpers);
            }
            // The band costs one greedy call per item and only a banded map
            // reads it. A door whose band is known to carry no signal does
            // not need it drawn again.
            "--no-band" => options.band = false,
            other => {
                eprintln!("unknown flag {other}");
                std::process::exit(2);
            }
        }
    }
    options
}

/// What the draws file already holds for one door.
struct Recorded {
    /// The item-and-block pairs already drawn, which are skipped.
    drawn: HashSet<(String, u64)>,
    /// The band each item was given. Greedy decoding does not read the seed
    /// block, so a band recorded once is the band for every block.
    bands: HashMap<String, Option<String>>,
}

/// What the file already holds: the draws to skip, and the band each item
/// was already given.
fn already(path: &Path, door: &str) -> Recorded {
    let mut done = HashSet::new();
    let mut bands = HashMap::new();
    let Ok(file) = File::open(path) else {
        return Recorded { drawn: done, bands };
    };
    for line in BufReader::new(file).lines().map_while(Result::ok) {
        let Ok(draw) = serde_json::from_str::<Draw>(&line) else {
            continue;
        };
        if draw.door != door {
            continue;
        }
        bands.insert(draw.item.clone(), draw.band.clone());
        done.insert((draw.item, draw.block));
    }
    Recorded { drawn: done, bands }
}

/// The items `support-v2-three-way` holds back.
///
/// A lock is a property of the item rather than of the file that names it,
/// and the two suite files name the same 196 items. This sweep reads them
/// anyway, for the reason in this module's documentation, and counts them so
/// the run says what it spent.
fn locked_items() -> BTreeSet<String> {
    let Ok(three_way) = gym::suite::support_v2_three_way() else {
        return BTreeSet::new();
    };
    three_way
        .items
        .iter()
        .filter(|item| item.partition == gym::suite::Partition::Locked)
        .map(|item| item.id.clone())
        .collect()
}

fn main() {
    let options = read_options();
    if options.out.is_empty() {
        eprintln!("--out is required: the draws are the point of the run");
        std::process::exit(2);
    }
    let suite = Suite::load(SUITE).expect("the shipped suite loads");
    let items: Vec<&Item> = suite
        .items
        .iter()
        .filter(|item| options.split == "all" || item.split == options.split)
        .collect();
    if items.is_empty() {
        eprintln!("no items in split {}", options.split);
        std::process::exit(2);
    }

    let pool = match Pool::discover(options.helpers) {
        Ok(pool) => pool,
        Err(refusal) => {
            eprintln!("no helper: {refusal}");
            std::process::exit(2);
        }
    };
    let availability = pool.availability().expect("availability answered");
    if !availability.is_available() {
        eprintln!(
            "the runtime is {}: {:?}",
            availability.status, availability.reason
        );
        std::process::exit(2);
    }
    let base_signature = pool.base_signature_prefix().unwrap_or_default();
    let adapter_identifier = options.adapter.as_deref().map(|path| {
        lev::adapter::Package::open(path).map_or_else(
            |error| {
                eprintln!("the adapter package did not open: {error}");
                std::process::exit(2);
            },
            |package| package.metadata.adapter_identifier,
        )
    });
    let mut bridge = Bridge::discover().expect("a helper starts");
    let bands: Vec<String> = BANDS.iter().map(|band| (*band).to_string()).collect();

    let path = Path::new(&options.out);
    let Recorded {
        drawn,
        bands: mut known_bands,
    } = already(path, &options.door);
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .expect("the draws file opens for appending");

    let locked = locked_items();
    let spending = items
        .iter()
        .filter(|item| locked.contains(&item.id))
        .count();
    eprintln!(
        "{spending} of these items are locked by support-v2-three-way and are read anyway; \
         see this binary's documentation for why"
    );
    eprintln!(
        "{} items in split {}, {} blocks, {} samples, pool {}, {} draws already recorded",
        items.len(),
        options.split,
        options.blocks,
        options.samples,
        pool.width(),
        drawn.len()
    );

    let mut written = 0_usize;
    let mut refused_items = 0_usize;
    // Block-major, the order `lev-seed-sweep` drew in: each block is one
    // complete pass over the items, so an interrupted run leaves whole
    // blocks rather than a grid with holes.
    for block in 0..options.blocks {
        for item in &items {
            if drawn.contains(&(item.id.clone(), block)) {
                continue;
            }
            let request = SystemOneRequest {
                state: item.state.clone(),
                model: None,
                questions: [("q".to_string(), item.question.clone())]
                    .into_iter()
                    .collect(),
                extensions: Extensions::default(),
            };
            let Ok(compiled) = compile(&request) else {
                eprintln!("{} did not compile", item.id);
                refused_items += 1;
                continue;
            };
            // The band is greedy, so it does not move with the seed block.
            // Draw it once per item and carry it.
            let band = match known_bands.get(&item.id) {
                Some(band) => band.clone(),
                None if !options.band => None,
                None => {
                    let call =
                        Call::decide(&compiled["q"], Sampling::Greedy).with_band(bands.clone());
                    let call = match options.adapter.as_deref() {
                        Some(path) => call.with_adapter(path),
                        None => call,
                    };
                    let band = bridge.decide(&call).ok().and_then(|outcome| outcome.band);
                    known_bands.insert(item.id.clone(), band.clone());
                    band
                }
            };
            let raw = match l2_pool_with(
                &pool,
                &compiled["q"],
                options.samples,
                block,
                options.adapter.as_deref(),
            ) {
                Ok(raw) => raw,
                Err(refusal) => {
                    eprintln!("{} block {block}: {refusal}", item.id);
                    refused_items += 1;
                    continue;
                }
            };
            let draw = Draw {
                schema: SCHEMA.to_string(),
                suite: suite.name.clone(),
                door: options.door.clone(),
                adapter: adapter_identifier.clone(),
                base_signature: base_signature.clone(),
                split: item.split.clone(),
                family: item.family.clone(),
                item: item.id.clone(),
                block,
                samples: options.samples,
                top: raw.top(),
                choice: raw.choice.clone(),
                truth: item.truth.clone(),
                correct: raw.choice == item.truth,
                band,
                refused: raw.refused,
            };
            let line = serde_json::to_string(&draw).expect("a draw serializes");
            writeln!(file, "{line}").expect("the draws file accepts a line");
            file.flush().expect("the draws file flushes");
            written += 1;
        }
        eprintln!("block {block} done, {written} draws written so far");
    }
    eprintln!("{written} draws written, {refused_items} items the door did not answer");
}
