use std::{env, path::PathBuf};

use psionic_train::{
    write_xtrain_explorer_artifacts, XTRAIN_EXPLORER_INDEX_FIXTURE_PATH,
    XTRAIN_EXPLORER_SNAPSHOT_FIXTURE_PATH,
};

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut snapshot_path = None;
    let mut index_path = None;
    let mut args = env::args().skip(1);
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--snapshot-output" => snapshot_path = args.next().map(PathBuf::from),
            "--index-output" => index_path = args.next().map(PathBuf::from),
            "--help" | "-h" => {
                print_usage();
                return Ok(());
            }
            other => return Err(format!("unsupported argument `{other}`").into()),
        }
    }

    let snapshot_path =
        snapshot_path.unwrap_or_else(|| PathBuf::from(XTRAIN_EXPLORER_SNAPSHOT_FIXTURE_PATH));
    let index_path =
        index_path.unwrap_or_else(|| PathBuf::from(XTRAIN_EXPLORER_INDEX_FIXTURE_PATH));
    let (snapshot, index) =
        write_xtrain_explorer_artifacts(snapshot_path.as_path(), index_path.as_path())?;
    println!(
        "wrote snapshot={} index={} participants={} events={}",
        snapshot_path.display(),
        index_path.display(),
        snapshot.participants.len(),
        snapshot.events.len(),
    );
    println!(
        "snapshot_digest={} index_digest={}",
        snapshot.snapshot_digest, index.index_digest
    );
    Ok(())
}

fn print_usage() {
    eprintln!(
        "usage: cargo run -q -p psionic-train --bin xtrain_explorer_artifacts -- [--snapshot-output <path>] [--index-output <path>]"
    );
}
