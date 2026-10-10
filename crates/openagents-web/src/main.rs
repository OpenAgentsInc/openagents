use std::net::SocketAddr;
use std::path::PathBuf;

const USAGE: &str = "usage: openagents-web [--store DIRECTORY] [--customer DIRECTORY] [--listen ADDRESS] \
[--pay-host http://HOST:PORT] [--inference http://HOST:PORT] [--public-host HOST]... [--upstream http://HOST:PORT] \
[--chat-store DIRECTORY | --chat-bucket BUCKET] [--chat-retention-days DAYS] [--chat-build DIRECTORY] [--everglade DIRECTORY] [--bunny DIRECTORY] [--components-build DIRECTORY] [--cloud-build DIRECTORY] \
[--cloud-config PRIVATE_JSON] [--cloud-hosts PRIVATE_JSON] [--cloud-byo PRIVATE_DIR [--cloud-byo-keys PRIVATE_JSON]] [--pilot-config PRIVATE_JSON] \
[--environments PRIVATE_JSON] [--github-oauth PRIVATE_JSON] [--github-app PRIVATE_JSON] [--github-redirect URL] \
[--plan-meter PRIVATE_FILE] [--plan-checkout PLAN] [--own-runs-token PRIVATE_FILE]";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let home = PathBuf::from(std::env::var_os("HOME").ok_or("HOME is unset")?);
    let mut config = openagents_web::Config::development(home.join(".openagents/tasks"));
    let mut listen: SocketAddr = "127.0.0.1:4300".parse()?;
    let mut chat_bucket = std::env::var("OPENAGENTS_WEB_CHAT_BUCKET").ok();
    let mut pay_host = std::env::var("OPENAGENTS_WEB_PAY_HOST").ok();
    let mut inference = std::env::var("OPENAGENTS_WEB_INFERENCE").ok();
    let mut upstream = std::env::var("OPENAGENTS_WEB_UPSTREAM").ok();
    // Off unless set: chats untouched this many days are removed.
    let mut chat_retention = std::env::var("OPENAGENTS_WEB_CHAT_RETENTION_DAYS").ok();
    let mut github_oauth: Option<PathBuf> = None;
    let mut github_redirect: Option<String> = None;
    let mut github_app: Option<PathBuf> = None;
    let mut cloud_byo: Option<PathBuf> = None;
    let mut cloud_byo_keys: Option<PathBuf> = None;
    let mut plan_meter: Option<PathBuf> = None;
    let mut plan_checkout: Option<String> = None;
    let mut arguments = std::env::args().skip(1);
    while let Some(option) = arguments.next() {
        let value = arguments.next().ok_or(USAGE)?;
        match option.as_str() {
            "--store" => config.store = PathBuf::from(value),
            "--chat-store" => {
                chat_bucket = None;
                config.chat_store = std::sync::Arc::new(openagents_web::chat_store::Store::local(
                    PathBuf::from(value),
                ))
            }
            "--chat-bucket" => chat_bucket = Some(value),
            "--chat-retention-days" => chat_retention = Some(value),
            "--chat-build" => config.chat_build = Some(PathBuf::from(value)),
            "--customer" => config.customer = Some(PathBuf::from(value)),
            "--listen" => listen = value.parse().map_err(|_| USAGE)?,
            "--public-host" => config.public_hosts.push(value),
            "--pay-host" => pay_host = Some(value),
            "--inference" => inference = Some(value),
            "--upstream" => upstream = Some(value),
            "--everglade" => config.everglade = Some(PathBuf::from(value)),
            "--bunny" => config.bunny = Some(PathBuf::from(value)),
            "--components-build" => config.components_build = Some(PathBuf::from(value)),
            "--cloud-build" => config.cloud_build = Some(PathBuf::from(value)),
            "--cloud-config" => {
                config.cloud = Some(std::sync::Arc::new(
                    openagents_web::cloud::session::CloudSession::load(std::path::Path::new(
                        &value,
                    ))?,
                ));
            }
            "--cloud-hosts" => {
                config.cloud_hosts = Some(std::sync::Arc::new(
                    openagents_web::cloud::hosts::Hosts::load(std::path::Path::new(&value))?,
                ));
            }
            "--cloud-byo" => cloud_byo = Some(PathBuf::from(value)),
            "--cloud-byo-keys" => cloud_byo_keys = Some(PathBuf::from(value)),
            // The environment meter journal behind Settings > Plan, and the
            // billing plan Subscribe opens Stripe Checkout for (through the
            // account service), when checkout is set up.
            "--plan-meter" => plan_meter = Some(PathBuf::from(value)),
            "--plan-checkout" => plan_checkout = Some(value),
            // The inference gateway's token for starting coding runs on a
            // person's own linked computers (#11080); read when asked, so
            // the gateway may write it after this server starts.
            "--own-runs-token" => openagents_web::own_runs::set_token(PathBuf::from(value)),
            // The OAuth App's private file ({client_id, client_secret,
            // token_encryption_key}); the web server reads the client id only.
            "--github-oauth" => github_oauth = Some(PathBuf::from(value)),
            "--github-redirect" => github_redirect = Some(value),
            // The GitHub App's private file; the web server reads its
            // client id and slug only.
            "--github-app" => github_app = Some(PathBuf::from(value)),
            "--environments" => {
                let studio =
                    coder_environment_operator::studio::Config::load(std::path::Path::new(&value))?;
                // A studio that can't open (Boat or the model unreachable)
                // leaves the rest of the site up, without Environments.
                match coder_environment_operator::studio::Studio::open(studio).await {
                    Ok(studio) => {
                        config.environments = Some(studio);
                        println!("Environments are on at /environments");
                    }
                    Err(error) => eprintln!("Environments are off: {error}"),
                }
            }
            "--pilot-config" => {
                config.pilot = Some(std::sync::Arc::new(openagents_web::pilot::Intake::load(
                    std::path::Path::new(&value),
                )?))
            }
            _ => return Err(USAGE.into()),
        }
    }
    // Saved own-Claude credentials are encrypted at rest (#11041) under a
    // keyring kept outside the custody directory: a private file
    // (--cloud-byo-keys) or the keyring document in
    // OPENAGENTS_WEB_CLOUD_BYO_KEYS (a Secret Manager secret on Cloud Run).
    // Custody refuses to start without one.
    if let Some(directory) = cloud_byo {
        let keyring = match (
            cloud_byo_keys,
            std::env::var("OPENAGENTS_WEB_CLOUD_BYO_KEYS"),
        ) {
            (Some(path), _) => oa_seal::Keyring::load(&path)?,
            (None, Ok(document)) if !document.trim().is_empty() => {
                let keyring = oa_seal::Keyring::parse(document.as_bytes());
                let mut bytes = document.into_bytes();
                bytes.fill(0);
                keyring?
            }
            _ => {
                return Err("--cloud-byo needs a keyring to encrypt saved keys: \
--cloud-byo-keys PRIVATE_JSON or OPENAGENTS_WEB_CLOUD_BYO_KEYS"
                    .into());
            }
        };
        config.cloud_byo = Some(std::sync::Arc::new(
            openagents_web::cloud::byo::Computers::open(&directory, keyring)?,
        ));
    } else if cloud_byo_keys.is_some() {
        return Err("--cloud-byo-keys needs --cloud-byo".into());
    }
    if plan_meter.is_some() || plan_checkout.is_some() {
        config.plan = Some(std::sync::Arc::new(openagents_web::plan::Plans::open(
            plan_meter.as_deref(),
            plan_checkout,
        )?));
    }
    if let Some(path) = github_app {
        let redirect = match (github_redirect.clone(), config.cloud.as_deref()) {
            (Some(url), _) => url,
            (None, Some(cloud)) => format!("{}{}", cloud.origin(), oa_auth::CALLBACK_PATH),
            (None, None) => {
                return Err("--github-app needs --cloud-config (or --github-redirect)".into());
            }
        };
        config.github_install = Some(std::sync::Arc::new(oa_auth::AppInstall::load(
            &path,
            &redirect,
            oa_auth::Endpoints::default(),
        )?));
        println!("Repositories are added through the GitHub App");
    }
    if let Some(path) = github_oauth {
        // The callback defaults to the Cloud origin's /auth/github/callback.
        let redirect = match (github_redirect, config.cloud.as_deref()) {
            (Some(url), _) => url,
            (None, Some(cloud)) => format!("{}{}", cloud.origin(), oa_auth::CALLBACK_PATH),
            (None, None) => {
                return Err("--github-oauth needs --cloud-config (or --github-redirect)".into());
            }
        };
        config.github = Some(std::sync::Arc::new(oa_auth::GithubApp::load(
            &path,
            &redirect,
            oa_auth::Endpoints::default(),
        )?));
        println!("GitHub sign-in returns to {redirect}");
    }
    let shared_chats = chat_bucket.is_some();
    if let Some(bucket) = chat_bucket {
        config.chat_store = std::sync::Arc::new(openagents_web::chat_store::Store::gcs(
            bucket,
            "conversations".into(),
        )?);
    }
    if let Some(days) = chat_retention.filter(|value| !value.trim().is_empty()) {
        let days = openagents_web::chat_store::retention_days(&days)?;
        openagents_web::chat_store::spawn_expiry(config.chat_store.clone(), days);
        println!("Chats untouched for {days} days are removed");
    }
    let worker = openagents_web::ask::Worker::from_env()?;
    if !worker.is_production() {
        println!("Chat answers come from the worker {}", worker.worker());
    }
    config.chat = std::sync::Arc::new(worker);
    config.port = listen.port();
    // A public deployment is served over HTTPS behind its proxy.
    config.secure_cookies = !config.public_hosts.is_empty();
    if config.secure_cookies && !shared_chats {
        return Err(
            "A public chat deployment requires --chat-bucket or OPENAGENTS_WEB_CHAT_BUCKET".into(),
        );
    }
    // Instances that share visitors share the secret their keys come from.
    if let Ok(salt) = std::env::var("OPENAGENTS_WEB_ASK_SALT") {
        if salt.len() != 64 || !salt.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err("OPENAGENTS_WEB_ASK_SALT is not 64 hex characters".into());
        }
        let bytes: Vec<u8> = (0..salt.len())
            .step_by(2)
            .filter_map(|at| salt.get(at..at + 2))
            .filter_map(|pair| u8::from_str_radix(pair, 16).ok())
            .collect();
        config.ask_salt = bytes
            .try_into()
            .map_err(|_| "OPENAGENTS_WEB_ASK_SALT is not 64 hex characters")?;
    } else if config.secure_cookies {
        return Err("A public chat deployment requires OPENAGENTS_WEB_ASK_SALT".into());
    }
    // Paths the site doesn't own go to the previous server, if one is named.
    if let Some(url) = upstream.filter(|url| !url.is_empty()) {
        config.upstream = Some(std::sync::Arc::new(
            openagents_web::upstream::Upstream::new(&url)?,
        ));
        println!("Paths this site doesn't own are proxied to {url}");
    }
    // Staging's smoke suite makes its test account through the alias.
    config.api_operator_signup =
        std::env::var("OPENAGENTS_WEB_API_OPERATOR_SIGNUP").is_ok_and(|value| value == "1");
    // Accounts besides site admins that may do agent work (Environments,
    // Claude Code runs) on a public host: staging's smoke test account.
    config.agent_accounts = std::env::var("OPENAGENTS_WEB_AGENT_ACCOUNTS")
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|account| !account.is_empty())
        .map(str::to_owned)
        .collect();
    // The inference gateway: `/api/v1/...` and the API docs' rate card.
    if let Some(url) = inference.filter(|url| !url.is_empty()) {
        config.inference = Some(std::sync::Arc::new(
            openagents_web::upstream::Upstream::new(&url)?,
        ));
    }
    if let Some(url) = pay_host {
        config.pay_upstream = Some(std::sync::Arc::new(
            openagents_web::upstream::Upstream::new(&url)?,
        ));
    }
    // First-party, cookieless counts (#11153): written every minute to
    // OPENAGENTS_WEB_ANALYTICS_BUCKET (or _DIR), and once more when Cloud
    // Run stops the instance.
    let analytics = std::sync::Arc::new(openagents_web::analytics::Analytics::from_env()?);
    if analytics.has_store() {
        analytics.spawn();
        let last = analytics.clone();
        tokio::spawn(async move {
            #[cfg(unix)]
            if let Ok(mut term) =
                tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            {
                term.recv().await;
                let _ = last.flush().await;
                std::process::exit(0);
            }
        });
        println!("Analytics counts are kept");
    }
    config.analytics = analytics;
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
