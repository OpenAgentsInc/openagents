use std::env;
use autopilot_core::preflight::PreflightConfig;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cwd = env::current_dir()?;

    println!("=== Autopilot Preflight Demo ===\n");

    println!("Step 1: Running preflight checks...");
    let config = PreflightConfig::run(&cwd)?;

    println!("\n--- Preflight Results ---");
    println!("{}", config.to_system_prompt());

    println!("\nStep 2: Saving config...");
    let config_path = config.save()?;
    println!("Saved to: {}", config_path.display());
    println!("\n--- Demo Complete ---");

    Ok(())
}
