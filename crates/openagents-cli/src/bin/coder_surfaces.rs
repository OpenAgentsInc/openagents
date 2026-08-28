//! Check or rebuild the staged coder text surfaces.

fn main() {
    let write = std::env::args().any(|arg| arg == "--write");
    if write {
        openagents_cli::surfaces_build::write();
        return;
    }
    match openagents_cli::surfaces_build::check() {
        Ok(()) => {
            println!("coder surfaces are current (3 artifacts, digests pinned)");
        }
        Err(failures) => {
            eprintln!("coder surfaces are out of date:\n");
            for failure in failures {
                eprintln!("  - {failure}");
            }
            eprintln!();
            std::process::exit(1);
        }
    }
}
