//! Prints what the NOSTR tab would show for ten seconds:
//! `cargo run -p verse --example nostr_feed`.

fn main() {
    let mut feed = verse::feed::Feed::start();
    let start = std::time::Instant::now();
    let mut printed = 0;
    while start.elapsed() < std::time::Duration::from_secs(12) {
        feed.tick(std::time::Instant::now());
        for line in feed.lines.iter().skip(printed) {
            println!(
                "{}: {} {}",
                line.from,
                line.text,
                line.note.clone().unwrap_or_default()
            );
        }
        printed = feed.lines.len();
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    println!("-- {} ; {} visitors", feed.title(), feed.visitors.len());
}
