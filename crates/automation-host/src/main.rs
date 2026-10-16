fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let json = args.iter().any(|s| s == "--json");
    let (value, failed) = match automation_host::cli::run(args) {
        Ok(v) => (v, false),
        Err(e) => (serde_json::json!({"error":e}), true),
    };
    if json {
        println!("{}", serde_json::to_string(&value).expect("JSON value"));
    } else {
        println!("{}", automation_host::cli::display(&value));
    }
    if failed {
        std::process::exit(1);
    }
}
