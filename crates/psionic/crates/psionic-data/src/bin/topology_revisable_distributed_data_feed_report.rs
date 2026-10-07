use std::{env, path::PathBuf, process};

use psionic_data::write_topology_revisable_distributed_data_feed_semantics_report;

fn main() {
    let output_path = match env::args().nth(1) {
        Some(path) => PathBuf::from(path),
        None => {
            eprintln!(
                "usage: cargo run -p psionic-data --bin topology_revisable_distributed_data_feed_report -- <output-path>"
            );
            process::exit(2);
        }
    };
    if let Err(error) =
        write_topology_revisable_distributed_data_feed_semantics_report(&output_path)
    {
        eprintln!("error: {error}");
        process::exit(1);
    }
}
