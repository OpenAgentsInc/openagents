//! Disposable local coder-sync service for the browser visit check. No real account or bucket.
use coder_sync::server::{
    self, StateData, config::AuthMode, db::Database, rooms::Rooms, storage::Bucket,
};
#[tokio::main]
async fn main() {
    let output = std::env::args()
        .nth(1)
        .expect("Pass the scratch configuration path");
    let database = Database::new(
        &std::env::var("CODER_SYNC_TEST_DATABASE_URL").expect("Use a disposable test database"),
    )
    .unwrap();
    database.migrate().await.unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let state = StateData {
        database,
        bucket: Bucket::memory(),
        rooms: Rooms::default(),
        devices: Default::default(),
        auth: AuthMode::OpenAgents,
        accounts: None,
        revision: "local-verse-visit-fixture".into(),
        push: None,
        proof_origin: Some(origin.clone()),
        room_keys: vec![coder_access::room_token::public_key(&"07".repeat(32)).unwrap()],
    };
    std::fs::write(
        output,
        serde_json::to_vec(&serde_json::json!({"sync_origin":origin})).unwrap(),
    )
    .unwrap();
    println!("READY: isolated visit room service");
    axum::serve(listener, server::router(state)).await.unwrap();
}
