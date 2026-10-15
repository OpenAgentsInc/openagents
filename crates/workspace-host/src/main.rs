//! Standalone entry point for the shared workspace command.
fn main() {
    match workspace_host::cli::run(std::env::args().skip(1).collect()) {
        Ok(Some(value)) => println!("{}", workspace_host::cli::display(&value)),
        Ok(None) => {}
        Err(error) => {
            eprintln!("{}", serde_json::json!({"error":error}));
            std::process::exit(1);
        }
    }
}
