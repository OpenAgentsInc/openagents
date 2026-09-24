//! `skills-moderate` — the operator's moderation path for the skill
//! directory.
//!
//! The book lives in `skills.json` beside the registry. Every verb is
//! one sealed mutation on it; `list` and `show` only read. There is no
//! HTTP route for any of this — takedown, reinstatement, appeal-granted
//! admission, and evidence attachment are operator acts, recorded in
//! the audit trail under the `operator` actor.
//!
//! ```text
//! skills-moderate list      --registry DIR [--state STATE]
//! skills-moderate show      --registry DIR --name NAME --version VER
//! skills-moderate takedown  --registry DIR --name NAME --version VER --reason TEXT
//! skills-moderate reinstate --registry DIR --name NAME --version VER
//! skills-moderate admit     --registry DIR --name NAME --version VER --reason TEXT
//! skills-moderate evidence  --registry DIR --name NAME --version VER \
//!                           --suite SUITE --report REPORT --digest DIGEST
//! ```

use std::path::Path;

use tenancy::skills::{self, Directory, Evidence};

fn usage() -> ! {
    eprintln!(
        "Usage:\n  \
         skills-moderate list      --registry DIR [--state STATE]\n  \
         skills-moderate show      --registry DIR --name NAME --version VER\n  \
         skills-moderate takedown  --registry DIR --name NAME --version VER --reason TEXT\n  \
         skills-moderate reinstate --registry DIR --name NAME --version VER\n  \
         skills-moderate admit     --registry DIR --name NAME --version VER --reason TEXT\n  \
         skills-moderate evidence  --registry DIR --name NAME --version VER \\\n  \
         \t\t\t   --suite SUITE --report REPORT --digest DIGEST"
    );
    std::process::exit(2);
}

fn main() {
    let mut args = std::env::args().skip(1);
    let Some(verb) = args.next() else {
        usage();
    };
    let mut registry_dir = None;
    let mut name = None;
    let mut version = None;
    let mut reason = None;
    let mut state = None;
    let mut suite = None;
    let mut report = None;
    let mut digest = None;
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--registry" => registry_dir = args.next(),
            "--name" => name = args.next(),
            "--version" => version = args.next(),
            "--reason" => reason = args.next(),
            "--state" => state = args.next(),
            "--suite" => suite = args.next(),
            "--report" => report = args.next(),
            "--digest" => digest = args.next(),
            _ => usage(),
        }
    }
    let Some(dir) = registry_dir else { usage() };
    let dir = Path::new(&dir);
    let directory = match Directory::open(dir) {
        Ok(directory) => directory,
        Err(trouble) => {
            eprintln!("{trouble}");
            std::process::exit(1);
        }
    };

    let named = |name: Option<String>, version: Option<String>| -> (String, String) {
        let (Some(name), Some(version)) = (name, version) else {
            usage();
        };
        (name, version)
    };

    let result: Result<(), skills::Refusal> = match verb.as_str() {
        "list" => {
            let Ok(store) = directory.store() else {
                eprintln!("can't read the skill directory in `skills.json`");
                std::process::exit(1);
            };
            for entry in store.book.entries.values() {
                for version in entry.versions.values() {
                    if state
                        .as_ref()
                        .is_some_and(|want| version.state.name() != want)
                    {
                        continue;
                    }
                    println!(
                        "{}\t{}\t{}\t{}\t{}",
                        entry.name,
                        version.version,
                        version.state.name(),
                        version.author,
                        version.digest,
                    );
                }
            }
            Ok(())
        }
        "show" => {
            let (name, version) = named(name, version);
            let Ok(store) = directory.store() else {
                eprintln!("can't read the skill directory in `skills.json`");
                std::process::exit(1);
            };
            let Some(version) = store
                .book
                .entries
                .get(&name)
                .and_then(|entry| entry.versions.get(&version))
            else {
                eprintln!("{name} {version} isn't in the skill directory");
                std::process::exit(1);
            };
            println!(
                "{}",
                serde_json::to_string_pretty(version).unwrap_or_default()
            );
            Ok(())
        }
        "takedown" => {
            let (name, version) = named(name, version);
            let Some(reason) = reason else { usage() };
            directory
                .mutate(|book, _, now| book.takedown(&name, &version, "operator", &reason, now))
                .map(|()| println!("Took down `{name} {version}`; it is no longer listed."))
        }
        "reinstate" => {
            let (name, version) = named(name, version);
            directory
                .mutate(|book, _, now| book.reinstate(&name, &version, "operator", now))
                .map(|()| println!("Reinstated `{name} {version}`; it is listed again."))
        }
        "admit" => {
            let (name, version) = named(name, version);
            let Some(reason) = reason else { usage() };
            directory
                .mutate(|book, _, now| {
                    book.moderate_admit(&name, &version, "operator", &reason, now)
                })
                .map(|()| println!("Approved and published `{name} {version}`."))
        }
        "evidence" => {
            let (name, version) = named(name, version);
            let (Some(suite), Some(report), Some(digest)) = (suite, report, digest) else {
                usage();
            };
            directory
                .mutate(|book, _, now| {
                    book.attach_evidence(
                        &name,
                        &version,
                        Evidence {
                            suite,
                            report,
                            digest,
                            measured_at: now,
                        },
                        "operator",
                        now,
                    )
                })
                .map(|()| println!("Attached evidence to `{name} {version}`."))
        }
        _ => usage(),
    };
    if let Err(refusal) = result {
        eprintln!("{refusal}");
        std::process::exit(1);
    }
}
