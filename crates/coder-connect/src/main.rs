//! Standalone command for read-only retained history pairing and observation.
#[tokio::main]
async fn main() {
    if let Err(error) = coder_connect::cli::run(std::env::args().skip(1)).await {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
