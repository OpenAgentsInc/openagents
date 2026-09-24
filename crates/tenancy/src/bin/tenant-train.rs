//! `tenant-train` — the operator's path through tenant training.
//!
//! The store lives in `training/` beside the registry. Every verb is
//! one sealed step: a checked corpus, a frozen recipe, an appended
//! trial, a sealed candidate. There is no HTTP route for any of this —
//! corpus registration, recipe freezes, and candidate seals are
//! operator acts, and a sealed candidate is inspectable, never served.
//!
//! ```text
//! tenant-train check    --registry DIR --corpus FILE
//! tenant-train headroom --registry DIR --corpus NAME --scores FILE
//! tenant-train freeze   --registry DIR --recipe FILE
//! tenant-train trial    --registry DIR --record FILE
//! tenant-train seal     --registry DIR --candidate FILE
//! tenant-train inspect  --registry DIR --candidate NAME
//! tenant-train delete   --registry DIR --corpus NAME --reason TEXT
//! tenant-train list     --registry DIR
//! ```

use std::fs;
use std::path::PathBuf;
use std::process::exit;

use tenancy::training::Book;

fn usage() -> ! {
    eprintln!(
        "Usage:\n  \
         tenant-train check    --registry DIR --corpus FILE\n  \
         tenant-train headroom --registry DIR --corpus NAME --scores FILE\n  \
         tenant-train freeze   --registry DIR --recipe FILE\n  \
         tenant-train trial    --registry DIR --record FILE\n  \
         tenant-train seal     --registry DIR --candidate FILE\n  \
         tenant-train inspect  --registry DIR --candidate NAME\n  \
         tenant-train delete   --registry DIR --corpus NAME --reason TEXT\n  \
         tenant-train list     --registry DIR"
    );
    exit(2);
}

fn main() {
    let mut args = std::env::args().skip(1);
    let Some(verb) = args.next() else {
        usage();
    };
    let mut registry = None;
    let mut corpus = None;
    let mut scores = None;
    let mut recipe = None;
    let mut record = None;
    let mut candidate = None;
    let mut reason = None;
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--registry" => registry = args.next(),
            "--corpus" => corpus = args.next(),
            "--scores" => scores = args.next(),
            "--recipe" => recipe = args.next(),
            "--record" => record = args.next(),
            "--candidate" => candidate = args.next(),
            "--reason" => reason = args.next(),
            _ => usage(),
        }
    }
    let Some(dir) = registry.map(PathBuf::from) else {
        usage();
    };
    let book = match Book::open(&dir) {
        Ok(book) => book,
        Err(trouble) => {
            eprintln!("{trouble}");
            exit(1);
        }
    };
    let read = |path: &Option<String>| -> String {
        let Some(path) = path else { usage() };
        match fs::read_to_string(path) {
            Ok(text) => text,
            Err(error) => {
                eprintln!("can't read {path}: {error}");
                exit(1);
            }
        }
    };
    let now = || {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| format!("{}", d.as_secs()))
            .unwrap_or_default()
    };
    match verb.as_str() {
        "check" => match book.register_corpus(&read(&corpus)) {
            Ok(corpus) => {
                println!("Registered corpus {}. Items per partition:", corpus.name);
                for (role, count) in corpus.counts() {
                    println!("  {role}: {count}");
                }
                println!("  digest: {}", corpus.digest);
            }
            Err(trouble) => {
                eprintln!("{trouble}");
                exit(1);
            }
        },
        "headroom" => {
            let Some(name) = corpus else { usage() };
            match book.assess(&name, &read(&scores), &now()) {
                Ok(report) => {
                    println!(
                        "Headroom against baseline model {}: {} items scored, {} failed. Failures by cause:",
                        report.baseline_door, report.scored, report.failed
                    );
                    for (cause, count) in &report.causes {
                        println!("  {cause}: {count}");
                    }
                    println!(
                        "  verdict: {}",
                        serde_json::to_value(report.verdict).unwrap()
                    );
                    println!("  digest: {}", report.digest);
                }
                Err(trouble) => {
                    eprintln!("{trouble}");
                    exit(1);
                }
            }
        }
        "freeze" => match book.freeze_recipe(&read(&recipe)) {
            Ok(recipe) => println!(
                "Froze recipe {}; it can no longer change.\n  digest: {}",
                recipe.name, recipe.digest
            ),
            Err(trouble) => {
                eprintln!("{trouble}");
                exit(1);
            }
        },
        "trial" => match book.record_trial(&read(&record)) {
            Ok(position) => {
                println!("Recorded the trial at position {position} in the trials ledger.")
            }
            Err(trouble) => {
                eprintln!("{trouble}");
                exit(1);
            }
        },
        "seal" => match book.seal_candidate(&read(&candidate)) {
            Ok(candidate) => println!(
                "Sealed candidate {}; it can no longer change and isn't served.\n  signature: {}",
                candidate.name, candidate.signature
            ),
            Err(trouble) => {
                eprintln!("{trouble}");
                exit(1);
            }
        },
        "inspect" => {
            let Some(name) = candidate else { usage() };
            match book.candidate(&name) {
                Ok(candidate) => match serde_json::to_string_pretty(&candidate) {
                    Ok(text) => println!("{text}"),
                    Err(error) => {
                        eprintln!("can't format candidate {name} as JSON: {error}");
                        exit(1);
                    }
                },
                Err(trouble) => {
                    eprintln!("{trouble}");
                    exit(1);
                }
            }
        }
        "delete" => {
            let (Some(name), Some(reason)) = (corpus, reason) else {
                usage();
            };
            match book.delete_corpus(&name, &now(), &reason) {
                Ok(tombstone) => println!(
                    "Deleted corpus {name} at {}. Kept the SHA-256 digests of its {} items as a record of what was deleted.",
                    tombstone.deleted_at,
                    tombstone.item_digests.len()
                ),
                Err(trouble) => {
                    eprintln!("{trouble}");
                    exit(1);
                }
            }
        }
        "list" => {
            for name in book.corpora() {
                println!("corpus    {name}");
            }
            for recipe in book.recipes() {
                println!("recipe    {}  {}", recipe.name, recipe.digest);
            }
            for name in book.candidates() {
                println!("candidate {name}");
            }
        }
        _ => usage(),
    }
}
