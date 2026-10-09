//! The resident sales-owner remote adapter, run on the owner host beside the
//! private pipeline. It listens on numeric loopback only; put authenticated
//! TLS in front of it. Public sites never open the pipeline themselves.
use coder::task::sales::remote::Service;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;

const USAGE: &str = "usage: sales-remote serve PRIVATE_BINDINGS_JSON LOOPBACK_ADDRESS";

#[tokio::main]
async fn main() -> Result<(), String> {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let [operation, config, address] = arguments.as_slice() else {
        return Err(USAGE.into());
    };
    if operation != "serve" {
        return Err(USAGE.into());
    }
    let address: SocketAddr = address.parse().map_err(|_| USAGE)?;
    if !address.ip().is_loopback() {
        return Err("bind numeric loopback; terminate authenticated TLS in front".into());
    }
    let service = Arc::new(Service::open(Path::new(config))?);
    let listener = tokio::net::TcpListener::bind(address)
        .await
        .map_err(|e| e.to_string())?;
    println!("Sales remote adapter listening on loopback.");
    axum::serve(listener, openagents_web::sales_remote::router(service))
        .await
        .map_err(|e| e.to_string())
}
