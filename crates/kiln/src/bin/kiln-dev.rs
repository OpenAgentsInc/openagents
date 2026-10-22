//! `kiln-dev [--port N] [--dir DIR] [--runner KEY] [--repo 30617:KEY:ID]
//! [--relay URL] [--live-store STORE --live-pubkey HEX [--live-tenant NAME]
//! [--live-channel NAME] [--live-prefix PREFIX] [--live-relay URL]]`: serve
//! the Kiln page and its assets on `127.0.0.1` with the cross-origin
//! isolation headers (COOP and COEP). `/config.json` names the
//! `kiln-runner` key the page sends remote `cargo` runs to, the NIP-34
//! repository address patches go to, and the relay.
//!
//! With `--live-*`, the server follows a livehost tenant's signed pointer,
//! as `openagents-web` does, and answers each file from the manifest's
//! `PREFIX/<file>` (default `modules/kiln`) before the directory: the local
//! stand-in for Kiln's self-edit on the site (`docs/kiln/self-edit.md`).

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    dev::main();
}

#[cfg(target_arch = "wasm32")]
fn main() {}

#[cfg(not(target_arch = "wasm32"))]
mod dev {
    use std::net::TcpListener;
    use std::path::PathBuf;
    use std::sync::Arc;

    use kiln::serve::{self, Overlay, Server};

    struct Live {
        store: String,
        pubkey: String,
        tenant: String,
        channel: String,
        prefix: String,
        relay: String,
    }

    fn follow(live: &Live) -> Overlay {
        let runtime = Arc::new(
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .unwrap_or_else(|e| {
                    eprintln!("kiln-dev: {e}");
                    std::process::exit(1);
                }),
        );
        let store = livehost::store::Store::parse(&live.store).unwrap_or_else(|e| {
            eprintln!("kiln-dev: {e:#}");
            std::process::exit(1);
        });
        let config = livehost::follow::FollowConfig {
            relay: live.relay.clone(),
            store,
            tenant: live.tenant.clone(),
            channel: live.channel.clone(),
            pubkey: live.pubkey.clone(),
            lazy_prefixes: vec!["modules/".into()],
            lazy_budget: livehost::follow::LAZY_BUDGET,
        };
        let follower = runtime
            .block_on(async { livehost::follow::Follower::start(config) })
            .unwrap_or_else(|e| {
                eprintln!("kiln-dev: {e:#}");
                std::process::exit(1);
            });
        let prefix = format!("{}/", live.prefix.trim_end_matches('/'));
        Arc::new(move |name: &str| {
            let content = follower.current()?;
            let path = format!("{prefix}{name}");
            content.manifest.objects.get(&path)?;
            runtime
                .block_on(follower.object(&content, &path))
                .ok()
                .flatten()
                .map(|bytes| bytes.to_vec())
        })
    }

    pub fn main() {
        let mut port = 8787_u16;
        let mut dir = std::env::var("KILN_WEB").unwrap_or_else(|_| "dist".into());
        let mut runner = String::new();
        let mut repo = String::new();
        let mut relay = "wss://relay.openagents.com".to_string();
        let mut live = Live {
            store: String::new(),
            pubkey: String::new(),
            tenant: "kiln-dev".into(),
            channel: "staging".into(),
            prefix: "modules/kiln".into(),
            relay: String::new(),
        };
        let mut args = std::env::args().skip(1);
        while let Some(arg) = args.next() {
            let mut value = || args.next().unwrap_or_default();
            match arg.as_str() {
                "--port" => port = value().parse().unwrap_or(port),
                "--dir" => dir = value(),
                "--runner" => runner = value(),
                "--repo" => repo = value(),
                "--relay" => relay = value(),
                "--live-store" => live.store = value(),
                "--live-pubkey" => live.pubkey = value(),
                "--live-tenant" => live.tenant = value(),
                "--live-channel" => live.channel = value(),
                "--live-prefix" => live.prefix = value(),
                "--live-relay" => live.relay = value(),
                _ => {
                    eprintln!(
                        "usage: kiln-dev [--port N] [--dir DIR] [--runner KEY] [--repo 30617:KEY:ID] [--relay URL] [--live-store STORE --live-pubkey HEX [--live-tenant NAME] [--live-channel NAME] [--live-prefix PREFIX] [--live-relay URL]]"
                    );
                    std::process::exit(2);
                }
            }
        }
        let root = PathBuf::from(&dir);
        if !root.join("index.html").is_file() {
            eprintln!("kiln-dev: {dir} has no index.html; run scripts/build-kiln.sh first");
            std::process::exit(1);
        }
        let overlay = (!live.store.is_empty()).then(|| {
            if live.relay.is_empty() {
                live.relay = relay.clone();
            }
            follow(&live)
        });
        let listener = TcpListener::bind(("127.0.0.1", port)).unwrap_or_else(|e| {
            eprintln!("kiln-dev: 127.0.0.1:{port}: {e}");
            std::process::exit(1);
        });
        let port = listener.local_addr().map(|a| a.port()).unwrap_or(port);
        // The page reads these public values from here.
        let config =
            serde_json::json!({"runner": runner, "repo": repo, "relay": relay}).to_string();
        println!(
            "Kiln is at http://127.0.0.1:{port}/ (serving {dir}{})",
            if overlay.is_some() {
                format!(", {}/{} first", live.tenant, live.channel)
            } else {
                String::new()
            }
        );
        serve::run(
            &listener,
            &Server {
                root,
                config,
                overlay,
                log: true,
            },
        );
    }
}
