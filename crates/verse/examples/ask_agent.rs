//! Asks the agent one question through the configured door and prints the
//! answer: `cargo run -p verse --example ask_agent -- "hello?"`.

fn main() {
    let question = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "Say hello in five words.".into());
    let mut brain = verse::brain::Brain::start("tester");
    eprintln!("door: {}", brain.door);
    brain.ask(verse::brain::Ask {
        text: question,
        surroundings: "- You are on the Plaza, next to a tall pylon.".into(),
    });
    let start = std::time::Instant::now();
    while start.elapsed() < std::time::Duration::from_secs(60) {
        for reply in brain.drain() {
            match reply {
                verse::brain::Reply::Piece(_) => {}
                verse::brain::Reply::Done(text) => {
                    println!("{text}");
                    return;
                }
                verse::brain::Reply::Failed(text) => {
                    println!("FAILED: {text}");
                    return;
                }
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    println!("timed out");
}
