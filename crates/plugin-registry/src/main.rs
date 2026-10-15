//! The standalone registry command shares its implementation with openagents.
#[tokio::main]
async fn main() {
    match plugin_registry::cli::run(std::env::args().skip(1).collect()).await {
        Ok(Some(value)) => println!("{}", plugin_registry::cli::display(&value)),
        Ok(None) => {}
        Err(error) => {
            eprintln!(
                "{}",
                plugin_registry::cli::display(&serde_json::json!({"error":error}))
            );
            std::process::exit(1);
        }
    }
}
