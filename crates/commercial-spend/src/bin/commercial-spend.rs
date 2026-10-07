//! Run the explicitly reviewed private commercial spend controller.
fn main() {
    let mut args = std::env::args_os().skip(1);
    let Some(config) = args.next() else {
        eprintln!("Usage: commercial-spend /absolute/private/config.json");
        std::process::exit(2)
    };
    if args.next().is_some() {
        std::process::exit(2);
    }
    let result =
        commercial_spend::Controller::open(std::path::Path::new(&config)).and_then(|controller| {
            std::sync::Arc::new(controller).run(std::sync::Arc::new(
                std::sync::atomic::AtomicBool::new(false),
            ))
        });
    if result.is_err() {
        eprintln!(
            "Shared commercial controller refused its private configuration or current authority."
        );
        std::process::exit(1);
    }
}
