//! A native window over the shared terminal application and glyph renderer.
mod window;
fn main() {
    if let Err(error) = window::run() {
        eprintln!("openagents-terminal: {error}");
        std::process::exit(1);
    }
}
