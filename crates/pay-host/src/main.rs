use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

type Error = Box<dyn std::error::Error + Send + Sync>;
struct Sources {
    ledger: rusqlite::Connection,
    publications: Option<PathBuf>,
    routes: Option<PathBuf>,
}
impl Sources {
    fn sync(&self, store: &mut pay_host::Store) -> Result<(), Error> {
        if let Some(path) = &self.publications {
            let text = std::fs::read_to_string(path)?;
            for line in text.lines().filter(|line| !line.trim().is_empty()) {
                store.register_publication(&serde_json::from_str(line)?)?;
            }
        }
        store.sync_sources(&self.ledger)?;
        if let Some(path) = &self.routes {
            store.sync_run_journal(path)?;
        }
        Ok(())
    }
}
#[tokio::main]
async fn main() -> Result<(), Error> {
    let path = std::env::var("OPENAGENTS_PAY_FLOW_DB")?;
    let source_path = std::env::var("OPENAGENTS_PAY_SOURCE_DB")?;
    let salt = std::env::var("OPENAGENTS_PAY_FLOW_SALT")?;
    let salt: [u8; 32] = hex::decode(salt)?
        .try_into()
        .map_err(|_| "Flow salt must be 64 hex characters")?;
    if std::path::Path::new(&path).exists()
        && std::fs::canonicalize(&path)? == std::fs::canonicalize(&source_path)?
    {
        return Err("Flow storage and the source ledger must be separate files".into());
    }
    let mut store = pay_host::Store::open(&path, salt)?;
    let sources = Sources {
        ledger: rusqlite::Connection::open_with_flags(
            source_path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )?,
        publications: std::env::var_os("OPENAGENTS_PAY_PUBLICATIONS").map(PathBuf::from),
        routes: std::env::var_os("OPENAGENTS_PAY_ROUTE_JOURNAL").map(PathBuf::from),
    };
    sources
        .sync(&mut store)
        .map_err(|_| "Initial payment flow ingestion failed")?;
    let store = Arc::new(Mutex::new(store));
    let ingestion = store.clone();
    tokio::task::spawn_blocking(move || {
        loop {
            std::thread::sleep(std::time::Duration::from_millis(250));
            let result = ingestion
                .lock()
                .map_err(|_| "Flow store lock failed".into())
                .and_then(|mut store| sources.sync(&mut store));
            if result.is_err() {
                // Source errors can contain private identifiers or source text.
                eprintln!(
                    "Payment flow ingestion failed; the last committed events remain available"
                );
            }
        }
    });
    let listener = tokio::net::TcpListener::bind(
        std::env::var("OPENAGENTS_PAY_LISTEN").unwrap_or_else(|_| "127.0.0.1:4400".into()),
    )
    .await?;
    axum::serve(listener, pay_host::router(store)).await?;
    Ok(())
}
