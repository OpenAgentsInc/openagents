use std::net::SocketAddr;
use std::path::PathBuf;

const USAGE: &str = "usage: openagents-web [--store DIRECTORY] [--listen ADDRESS] \
[--public-host HOST]...";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let home = PathBuf::from(std::env::var_os("HOME").ok_or("HOME is unset")?);
    let mut config = openagents_web::Config::development(home.join(".openagents/tasks"));
    let mut listen: SocketAddr = "127.0.0.1:4300".parse()?;
    let mut arguments = std::env::args().skip(1);
    while let Some(option) = arguments.next() {
        let value = arguments.next().ok_or(USAGE)?;
        match option.as_str() {
            "--store" => config.store = PathBuf::from(value),
            "--listen" => listen = value.parse().map_err(|_| USAGE)?,
            "--public-host" => config.public_hosts.push(value),
            _ => return Err(USAGE.into()),
        }
    }
    config.port = listen.port();
    // A public deployment is served over HTTPS behind its proxy.
    config.secure_cookies = !config.public_hosts.is_empty();
    // Instances that share visitors share the secret their keys come from.
    if let Ok(salt) = std::env::var("OPENAGENTS_WEB_ASK_SALT") {
        let bytes: Vec<u8> = (0..salt.len())
            .step_by(2)
            .filter_map(|at| salt.get(at..at + 2))
            .filter_map(|pair| u8::from_str_radix(pair, 16).ok())
            .collect();
        config.ask_salt = bytes
            .try_into()
            .map_err(|_| "OPENAGENTS_WEB_ASK_SALT is not 64 hex characters")?;
    }
    let listener = tokio::net::TcpListener::bind(listen).await?;
    println!("OpenAgents web is listening on http://{listen} (development backend)");
    axum::serve(listener, openagents_web::router(config)).await?;
    Ok(())
}
