use std::process;

fn main() {
    if let Err(err) = autopilot_app::run() {
        eprintln!("Error: {}", err);
        process::exit(1);
    }
}
