//! Checks a `.fmadapter` package and says which rule failed.
//!
//! ```text
//! cargo run -p lev --bin lev-adapter-check -- runs/lev-v1/lev.fmadapter
//! ```
//!
//! The runtime's own error for a bad package is "the adapter asset is
//! invalid", which is true and not useful. This names the rule.

use lev::adapter::Package;
use lev::bridge::Bridge;

fn main() {
    let Some(path) = std::env::args().nth(1) else {
        eprintln!("usage: lev-adapter-check <package.fmadapter>");
        std::process::exit(2);
    };

    let package = match Package::open(&path) {
        Ok(package) => package,
        Err(refusal) => {
            eprintln!("{path}: {}", refusal.message);
            std::process::exit(1);
        }
    };

    println!("identifier   {}", package.metadata.adapter_identifier);
    println!("base         {}", package.metadata.base_model_signature);
    println!("rank         {}", package.metadata.lora_rank);
    println!("records      {}", package.records.len());
    println!(
        "payload      {} bytes",
        package.records.iter().map(|record| record.length).sum::<u64>()
    );
    println!("draft model  {}", if package.has_draft { "present" } else { "absent" });

    // If the device is reachable, check the pinning too. This is the failure
    // that costs a training run: an adapter built against a base the device
    // no longer runs.
    match Bridge::discover().and_then(|mut bridge| bridge.base_signature_prefix()) {
        Ok(prefix) => {
            if package.metadata.base_model_signature.starts_with(&prefix) {
                println!("device       matches (prefix {prefix})");
            } else {
                eprintln!(
                    "device       MISMATCH: this device accepts `{prefix}...` and the package is \
                     pinned to `{}`",
                    package.metadata.base_model_signature
                );
                std::process::exit(1);
            }
        }
        Err(refusal) => println!("device       not checked ({})", refusal.message),
    }
}
