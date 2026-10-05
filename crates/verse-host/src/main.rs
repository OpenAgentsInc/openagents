fn main() {
    if let Err(error) = verse_host::run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
