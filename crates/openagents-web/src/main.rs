use std::net::SocketAddr;
use std::path::PathBuf;

const USAGE: &str = "usage: openagents-web [--store DIRECTORY] [--listen ADDRESS] \
[--pay-host http://HOST:PORT] [--public-host HOST]... [--upstream http://HOST:PORT] \
[--everglade DIRECTORY] [--pilot-config PRIVATE_JSON]";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let home = PathBuf::from(std::env::var_os("HOME").ok_or("HOME is unset")?);
    let mut config = openagents_web::Config::development(home.join(".openagents/tasks"));
    let mut listen: SocketAddr = "127.0.0.1:4300".parse()?;
    let mut pay_host = std::env::var("OPENAGENTS_WEB_PAY_HOST").ok();
    let mut upstream = std::env::var("OPENAGENTS_WEB_UPSTREAM").ok();
    let mut arguments = std::env::args().skip(1);
    while let Some(option) = arguments.next() {
        let value = arguments.next().ok_or(USAGE)?;
        match option.as_str() {
            "--store" => config.store = PathBuf::from(value),
            "--listen" => listen = value.parse().map_err(|_| USAGE)?,
            "--public-host" => config.public_hosts.push(value),
            "--pay-host" => pay_host = Some(value),
            "--upstream" => upstream = Some(value),
            "--everglade" => config.everglade = Some(PathBuf::from(value)),
            "--pilot-config" => {
                config.pilot = Some(std::sync::Arc::new(openagents_web::pilot::Intake::load(
                    std::path::Path::new(&value),
                )?))
            }
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
    // Paths the site doesn't own go to the previous server, if one is named.
    if let Some(url) = upstream.filter(|url| !url.is_empty()) {
        config.upstream = Some(std::sync::Arc::new(
            openagents_web::upstream::Upstream::new(&url)?,
        ));
        println!("Paths this site doesn't own are proxied to {url}");
    }
    if let Some(url) = pay_host {
        config.pay_upstream = Some(std::sync::Arc::new(
            openagents_web::upstream::Upstream::new(&url)?,
        ));
    }
    let listener = tokio::net::TcpListener::bind(listen).await?;
    let bound = listener.local_addr()?;
    config.port = bound.port();
    println!("OpenAgents web is listening on http://{bound} (development backend)");
    let router = openagents_web::router(config);
    axum::serve(
        listener,
        router.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;
    Ok(())
}
