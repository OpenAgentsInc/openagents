//! Hosted Lab tools share the turn's captured account. The cache contains only
//! manifests, keyed by a credential fingerprint; it never chooses an account.
use lab_terminal::{CancellationToken, Hosted, Registry};
use openagents_login::Saved;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

pub const WATCH: &str = "lab_watch";
pub const INSTRUCTIONS: &str = "Labs tools edit documents in your OpenAgents account. Use their declared arguments and return the document link with the result. lab_watch follows document changes; a canceled wait does not cancel a job. Local Lab tools are offered only for kinds that aren't hosted.\n";

type Entry = (String, Instant, Result<Registry, String>);
static CACHE: OnceLock<Mutex<Vec<Entry>>> = OnceLock::new();
fn fingerprint(account: &Saved) -> String {
    format!(
        "{:x}",
        Sha256::digest(format!("{}\0{}", account.origin, account.token()))
    )
}
fn registry(account: &Saved) -> Result<Registry, String> {
    if invalid_env() {
        return Err(
            "OPENAGENTS_TOKEN isn't a login key. Set a sign-in key or session token.".into(),
        );
    }
    let key = fingerprint(account);
    let cache = CACHE.get_or_init(|| Mutex::new(Vec::new()));
    if let Some((_, _, result)) = cache
        .lock()
        .map_err(|_| "Couldn't read the Lab tools.")?
        .iter()
        .find(|(k, at, _)| k == &key && at.elapsed() < Duration::from_secs(60))
    {
        return result.clone();
    }
    // Unit tests with synthetic accounts must never reach a public service.
    #[cfg(test)]
    if !account.origin.starts_with("http://127.0.0.1:") {
        return Err("No Lab fixture for this account.".into());
    }
    let account = account.clone();
    let result = std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|_| "Couldn't start the Lab client.".to_owned())?;
        runtime.block_on(async {
            let cancel = CancellationToken::new();
            tokio::time::timeout(
                Duration::from_secs(5),
                Hosted::connect(&account.origin, account.token(), "me", test_run(), &cancel),
            )
            .await
            .map_err(|_| "Labs didn't answer. Try again.".to_owned())?
            .map(|h| h.registry)
        })
    })
    .join()
    .map_err(|_| "Couldn't read the Lab tools.".to_owned())?;
    let mut cache = cache.lock().map_err(|_| "Couldn't read the Lab tools.")?;
    cache.retain(|(k, at, _)| k != &key && at.elapsed() < Duration::from_secs(60));
    if cache.len() >= 32 {
        let _ = cache.remove(0);
    }
    cache.push((key, Instant::now(), result.clone()));
    result
}
fn invalid_env() -> bool {
    let get = |name: &str| std::env::var(name).ok();
    openagents_login::token_env_set(get) && Saved::from_env(get).is_none()
}
fn test_run() -> bool {
    std::env::var("OPENAGENTS_TEST_RUN").as_deref() == Ok("1")
}
pub fn definitions(account: Option<&Saved>) -> Vec<Value> {
    let Some(registry) = account.and_then(|a| registry(a).ok()) else {
        return Vec::new();
    };
    let mut defs = registry.definitions();
    if !defs.is_empty() {
        defs.push(json!({"type":"function","function":{"name":WATCH,"description":"Follow changes to an account Lab document and return its latest card.","parameters":{"type":"object","required":["document"],"properties":{"document":{"type":"string"},"since":{"type":"integer","minimum":0},"seconds":{"type":"integer","minimum":1,"maximum":120}},"additionalProperties":false}}}));
    }
    defs
}
pub fn local_allowed(account: Option<&Saved>, kind: &str) -> bool {
    if invalid_env() {
        return false;
    }
    account.map_or(true, |a| registry(a).is_ok_and(|r| !r.hosts(kind)))
}
pub fn handles(account: Option<&Saved>, name: &str) -> bool {
    let Some(account) = account else { return false };
    name == WATCH
        || registry(account).is_ok_and(|r| r.kind_for(name).is_some())
        || ((crate::video_lab::is_tool(name) || crate::robot_lab::is_tool(name))
            && !local_allowed(
                Some(account),
                if crate::video_lab::is_tool(name) {
                    "video"
                } else {
                    "robot"
                },
            ))
}
pub async fn execute(
    account: &Saved,
    name: &str,
    args: Value,
    cancel: &Arc<AtomicBool>,
) -> Result<Value, String> {
    if invalid_env() {
        return Err(
            "OPENAGENTS_TOKEN isn't a login key. Set a sign-in key or session token.".into(),
        );
    }
    if cancel.load(Ordering::Relaxed) {
        return Err("The Lab call was canceled before it started.".into());
    }
    let token = CancellationToken::new();
    let work = async {
        let hosted =
            Hosted::connect(&account.origin, account.token(), "me", test_run(), &token).await?;
        let value = if name == WATCH {
            let document = args["document"].as_str().ok_or("Choose a document.")?;
            let since = args["since"].as_u64().unwrap_or(0);
            let seconds = args["seconds"].as_u64().unwrap_or(15);
            if !(1..=120).contains(&seconds) {
                return Err("Wait between 1 and 120 seconds.".into());
            }
            let mut events = hosted.events(document, since)?;
            let mut seen = Vec::new();
            let deadline = tokio::time::Instant::now() + Duration::from_secs(seconds);
            while seen.len() < 100 {
                match tokio::time::timeout_at(deadline, events.next(&token)).await {
                    Ok(Ok(event)) => seen.push(event),
                    Ok(Err(error)) => return Err(error.to_string()),
                    Err(_) => break,
                }
            }
            let mut latest = hosted.latest(document, &token).await?;
            latest["events"] = json!(seen);
            latest["cursor"] = json!(events.cursor());
            latest
        } else {
            let kind = hosted
                .registry
                .kind_for(name)
                .ok_or("That tool isn't available on this account.")?
                .kind
                .clone();
            hosted.invoke(&kind, name, args, &token).await?
        };
        let text = hosted.render(&value)?;
        Ok(json!({"text":text,"lab":value}))
    };
    tokio::select! {
        result=work => result,
        _=async {while !cancel.load(Ordering::Relaxed) {tokio::time::sleep(Duration::from_millis(25)).await;}} => {token.cancel();Err("The Lab call was canceled. Check the document before repeating an edit.".into())}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn accounts_do_not_share_a_manifest_or_fall_back_when_discovery_fails() {
        let a = Saved::with_token(
            "https://example.invalid",
            "alice",
            "Alice",
            u64::MAX,
            "synthetic-a",
        );
        let b = Saved::with_token(
            "https://example.invalid",
            "bob",
            "Bob",
            u64::MAX,
            "synthetic-b",
        );
        assert_ne!(fingerprint(&a), fingerprint(&b));
        assert!(!local_allowed(Some(&a), "video"));
        assert!(local_allowed(None, "video"));
        assert!(handles(Some(&a), "lab_open"));
        assert!(definitions(Some(&a)).is_empty());
    }
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn hosted_definitions_replace_local_video_and_use_the_captured_account() {
        use axum::{
            Json, Router,
            http::HeaderMap,
            routing::{get, post},
        };
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let posted = calls.clone();
        let router=Router::new()
            .route("/v1/account",get(|headers:HeaderMap|async move {
                assert_eq!(headers["authorization"],format!("Bearer sess_{}","a".repeat(64)));
                Json(json!({"account":{"id":"acct_test","label":"Test"},"workspaces":[]}))
            }))
            .route("/v1/labs/kinds",get(||async {Json(json!({"data":[{
                "kind":"video","name":"Video Lab","schemas":["cut.v1"],"tool_prefix":"lab_","doc_arg":"document",
                "tools":[{"name":"lab_new_cut","description":"Save a cut.","parameters":{"type":"object","properties":{"document":{"type":"string"}}}}],
                "job_types":[],"views":["video"],"limits":{"max_doc_bytes":1024,"max_docs_per_space":10,"max_versions_per_doc":10,"max_args_bytes":1024,"runs_per_hour":10}
            }]}))}))
            .route("/v1/labs/documents/film",get(||async {Json(json!({"id":"film","kind":"video","schema":"cut.v1","title":"Film","created_unix_ms":1,"latest":1}))}))
            .route("/v1/labs/kinds/video/tools/lab_new_cut",post(move |Json(body):Json<Value>|{
                let posted=posted.clone();async move {posted.fetch_add(1,Ordering::SeqCst);assert_eq!(body["arguments"]["document"],"film");Json(json!({"result":{"text":"Saved."}}))}
            }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let task = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        let saved = Saved::with_token(
            &origin,
            "acct_test",
            "Test",
            u64::MAX,
            &format!("sess_{}", "a".repeat(64)),
        );
        assert!(
            definitions(Some(&saved))
                .iter()
                .any(|d| d["function"]["name"] == "lab_new_cut")
        );
        assert!(!local_allowed(Some(&saved), "video"));
        assert!(local_allowed(Some(&saved), "robot"));
        assert!(handles(Some(&saved), "lab_new_cut"));
        let cancel = Arc::new(AtomicBool::new(false));
        assert_eq!(
            execute(&saved, "lab_new_cut", json!({"document":"film"}), &cancel)
                .await
                .unwrap()["text"],
            "Saved."
        );
        assert!(
            execute(&saved, "lab_open", json!({"document":"film"}), &cancel)
                .await
                .is_err()
        );
        cancel.store(true, Ordering::Relaxed);
        assert!(
            execute(&saved, "lab_new_cut", json!({"document":"film"}), &cancel)
                .await
                .is_err()
        );
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        task.abort();
    }
}
