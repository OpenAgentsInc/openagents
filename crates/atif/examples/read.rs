//! Read an ATIF document back and say what it is: the check a consumer runs
//! on an exported trajectory, such as `openagents chat export`.
//!
//! ```sh
//! openagents chat export --thread ID > thread.json
//! cargo run -p atif --example read -- thread.json
//! ```
//!
//! Prints the version, `session_id`, `trajectory_id`, step count, and the
//! decision calls, or every problem [`atif::validate`] found, and exits 1.

fn main() {
    let Some(path) = std::env::args().nth(1) else {
        eprintln!("usage: read FILE");
        std::process::exit(64);
    };
    let text = std::fs::read_to_string(&path).unwrap_or_else(|error| {
        eprintln!("cannot read {path}: {error}");
        std::process::exit(1);
    });
    let mut document: serde_json::Value = serde_json::from_str(&text).unwrap_or_else(|error| {
        eprintln!("{path} is not JSON: {error}");
        std::process::exit(1);
    });
    let errors = atif::validate(&document);
    if !errors.is_empty() {
        for error in errors {
            eprintln!("{error}");
        }
        std::process::exit(1);
    }
    let version = atif::upgrade(&mut document).unwrap_or_else(|error| {
        eprintln!("{error}");
        std::process::exit(1);
    });
    let steps = document["steps"].as_array().map_or(0, Vec::len);
    println!(
        "{version} session_id={} trajectory_id={} steps={steps} decision_calls={}",
        document["session_id"].as_str().unwrap_or(""),
        document["trajectory_id"].as_str().unwrap_or(""),
        document["final_metrics"]["extra"]["decision_calls"]["total"],
    );
}
