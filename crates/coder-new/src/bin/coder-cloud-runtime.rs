//! Headless access to the same Coder settings, chat, and delegation runtime.
fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|a| a == "--version") {
        println!(
            "coder-cloud-runtime {} ({} {})",
            env!("CARGO_PKG_VERSION"),
            env!("CODER_GIT_COMMIT"),
            env!("CODER_GIT_TREE")
        );
        return;
    }
    if args.first().is_some_and(|a| a == "--runtime-manifest") {
        println!(
            "{}",
            serde_json::json!({"schema":"openagents.coder.cloud-runtime.v1", "revision":env!("CODER_GIT_COMMIT"), "tree":env!("CODER_GIT_TREE"), "version":env!("CARGO_PKG_VERSION")})
        );
        return;
    }
    let json = args.first().is_some_and(|a| a == "--json");
    if json {
        args.remove(0);
    }
    if args.first().is_some_and(|a| a == "coder") {
        args.remove(0);
    }
    std::process::exit(i32::from(coder_new::programmatic::run(&args, json)));
}
