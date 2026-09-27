use std::net::{Ipv4Addr, SocketAddrV4};
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = std::env::args().skip(1);
    let store = match (arguments.next(), arguments.next(), arguments.next()) {
        (Some(option), Some(path), None) if option == "--store" => PathBuf::from(path),
        (None, None, None) => PathBuf::from(std::env::var_os("HOME").ok_or("HOME is unset")?)
            .join(".openagents/tasks"),
        _ => return Err("usage: coder-web [--store DIRECTORY]".into()),
    };
    let listener =
        tokio::net::TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 4300)).await?;
    println!("Coder web is listening on http://127.0.0.1:4300");
    axum::serve(listener, coder_web::router(store)).await?;
    Ok(())
}
