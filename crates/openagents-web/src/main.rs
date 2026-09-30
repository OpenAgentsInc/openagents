use std::net::SocketAddr;
use std::path::PathBuf;

const USAGE: &str = "usage: openagents-web [--store DIRECTORY] [--listen ADDRESS] \
[--public-host HOST]... [--published DIRECTORY] [--releases-url URL]";

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
            "--published" => config.published = PathBuf::from(value),
            "--releases-url" => config.releases_url = value,
            _ => return Err(USAGE.into()),
        }
    }
    config.port = listen.port();
    let listener = tokio::net::TcpListener::bind(listen).await?;
    println!("OpenAgents web is listening on http://{listen} (development backend)");
    axum::serve(listener, openagents_web::router(config)).await?;
    Ok(())
}
