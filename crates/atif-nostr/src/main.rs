//! Standalone entry point for the same trajectory CLI the main command uses.
#[tokio::main]
async fn main() {
    match atif_nostr::cli::run(std::env::args().skip(1).collect()).await {
        Ok(Some(value)) => println!("{value}"),
        Ok(None) => {}
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}
