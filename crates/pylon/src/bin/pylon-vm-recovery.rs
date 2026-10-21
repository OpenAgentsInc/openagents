//! The maintained Flint service's small, separately hosted VM recovery loop.
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() != 4 {
        return Err("Usage: pylon-vm-recovery PROJECT ZONE INSTANCE NUMERIC_ID".into());
    }
    let target = pylon::vm_recovery::Target::new(&args[0], &args[1], &args[2], &args[3])?;
    let client = reqwest::Client::builder()
        .no_proxy()
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|e| e.to_string())?;
    loop {
        match target.recover(&client).await {
            Ok(action) => println!("Flint VM recovery: {action}"),
            Err(why) => eprintln!("Flint VM recovery: {why}"),
        }
        tokio::select! {
            _ = tokio::signal::ctrl_c() => break,
            _ = tokio::time::sleep(Duration::from_secs(60)) => {},
        }
    }
    Ok(())
}
