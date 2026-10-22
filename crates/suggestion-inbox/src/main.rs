//! Run the dedicated suggestion inbox under its own service account.

use std::sync::Arc;
use std::time::Duration;

use nostr::domain::{RelaySigner, Tag};
use suggestion_inbox::{Config, Service, now};
use verse_net::net::{In, Link, Out};

fn collect(service: Arc<Service>, key_file: std::path::PathBuf) -> Result<(), String> {
    let key = suggestion_inbox::read_key(&key_file)?;
    let signer = RelaySigner::from_secret_hex(&key.display_secret().to_string())
        .map_err(|e| e.to_string())?;
    let advertised = signer.sign(
        now(),
        10_050,
        service
            .config
            .relays
            .iter()
            .map(|relay| Tag::new(vec!["relay".into(), relay.clone()]))
            .collect(),
        String::new(),
    );
    struct Reader {
        relay: String,
        link: Link,
        auth: Option<String>,
        count: usize,
        oldest: u64,
    }
    fn subscribe(reader: &Reader, inbox: &str, until: u64) {
        reader.link.send(Out::Subscribe {
            id: "suggestion-history".into(),
            filters: vec![
                serde_json::json!({"kinds":[1059],"#p":[inbox],"until":until,"limit":500}),
            ],
            live: false,
        });
    }
    fn begin(reader: &mut Reader, inbox: &str, advertised: &nostr::domain::Event) {
        reader.count = 0;
        reader.oldest = now();
        reader.link.send(Out::Publish(advertised.clone()));
        // Offline senders may publish wraps with old hidden timestamps.
        // The live subscription must accept them after history has finished.
        reader.link.send(Out::Subscribe {
            id: "suggestion-live".into(),
            filters: vec![serde_json::json!({"kinds":[1059],"#p":[inbox],"limit":1})],
            live: true,
        });
        subscribe(reader, inbox, now());
    }
    let mut readers: Vec<_> = service
        .config
        .relays
        .iter()
        .map(|relay| Reader {
            relay: relay.clone(),
            link: Link::start(relay),
            auth: None,
            count: 0,
            oldest: now(),
        })
        .collect();
    loop {
        for reader in &mut readers {
            for input in reader.link.drain() {
                match input {
                    In::Connected => begin(reader, &service.config.inbox, &advertised),
                    In::Auth(challenge) => {
                        let auth = signer.sign(
                            now(),
                            22242,
                            vec![
                                Tag::new(vec!["relay".into(), reader.relay.clone()]),
                                Tag::new(vec!["challenge".into(), challenge]),
                            ],
                            String::new(),
                        );
                        reader.auth = Some(auth.id.clone());
                        reader.link.send(Out::Auth(auth));
                    }
                    In::Ok {
                        id, accepted: true, ..
                    } if reader.auth.as_ref() == Some(&id) => {
                        reader.auth = None;
                        begin(reader, &service.config.inbox, &advertised);
                    }
                    In::Event { sub, event } => {
                        if sub == "suggestion-history" {
                            reader.count += 1;
                            reader.oldest = reader.oldest.min(event.created_at);
                        }
                        // A storage failure stops collection. The supervisor's
                        // restart repeats history without trusting a partial file.
                        if playtest::suggestion::open(&event, &key).is_ok() {
                            service.store.receive(&event, now())?;
                        }
                    }
                    In::Eose(id)
                        if id == "suggestion-history"
                            && reader.count >= 500
                            && reader.oldest > 0 =>
                    {
                        let until = reader.oldest.saturating_sub(1);
                        reader.count = 0;
                        reader.oldest = until;
                        subscribe(reader, &service.config.inbox, until);
                    }
                    _ => {}
                }
            }
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

#[tokio::main]
async fn main() -> Result<(), String> {
    let path = std::env::args()
        .nth(1)
        .ok_or("usage: suggestion-inbox CONFIG.json")?;
    let config: Config = serde_json::from_slice(&std::fs::read(path).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    let service = Arc::new(Service::new(&config)?);
    let reader = service.clone();
    std::thread::spawn(move || {
        if let Err(error) = collect(reader, config.key_file) {
            eprintln!("suggestion inbox stopped: {error}");
            std::process::exit(1);
        }
    });
    let listener = tokio::net::TcpListener::bind(&config.bind)
        .await
        .map_err(|e| e.to_string())?;
    axum::serve(listener, suggestion_inbox::router(service))
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await
        .map_err(|e| e.to_string())
}
