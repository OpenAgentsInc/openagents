use std::net::SocketAddr;
use std::path::PathBuf;

const USAGE: &str = "usage: openagents-web [--store DIRECTORY] [--customer DIRECTORY] [--listen ADDRESS] \
[--pay-host http://HOST:PORT] [--public-host HOST]... [--upstream http://HOST:PORT] \
[--chat-store DIRECTORY | --chat-bucket BUCKET] [--chat-build DIRECTORY] [--everglade DIRECTORY] [--bunny DIRECTORY] [--components-build DIRECTORY] [--cloud-build DIRECTORY] \
[--cloud-config PRIVATE_JSON] [--cloud-hosts PRIVATE_JSON] [--cloud-byo PRIVATE_DIR] [--pilot-config PRIVATE_JSON] \
[--environments PRIVATE_JSON] [--github-oauth PRIVATE_JSON] [--github-redirect URL]";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let home = PathBuf::from(std::env::var_os("HOME").ok_or("HOME is unset")?);
    let mut config = openagents_web::Config::development(home.join(".openagents/tasks"));
    let mut listen: SocketAddr = "127.0.0.1:4300".parse()?;
    let mut chat_bucket = std::env::var("OPENAGENTS_WEB_CHAT_BUCKET").ok();
    let mut pay_host = std::env::var("OPENAGENTS_WEB_PAY_HOST").ok();
    let mut upstream = std::env::var("OPENAGENTS_WEB_UPSTREAM").ok();
    let mut github_oauth: Option<PathBuf> = None;
    let mut github_redirect: Option<String> = None;
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
            "--chat-build" => config.chat_build = Some(PathBuf::from(value)),
            "--customer" => config.customer = Some(PathBuf::from(value)),
            "--listen" => listen = value.parse().map_err(|_| USAGE)?,
            "--public-host" => config.public_hosts.push(value),
            "--pay-host" => pay_host = Some(value),
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
            "--cloud-byo" => {
                config.cloud_byo = Some(std::sync::Arc::new(
                    openagents_web::cloud::byo::Computers::open(std::path::Path::new(&value))?,
                ));
            }
            // The OAuth App's private file ({client_id, client_secret,
            // token_encryption_key}); the web server reads the client id only.
            "--github-oauth" => github_oauth = Some(PathBuf::from(value)),
            "--github-redirect" => github_redirect = Some(value),
            "--environments" => {
                let studio =
                    coder_environment_operator::studio::Config::load(std::path::Path::new(&value))?;
                config.environments =
                    Some(coder_environment_operator::studio::Studio::open(studio).await?);
                println!("Environments are on at /environments");
            }
            "--pilot-config" => {
                config.pilot = Some(std::sync::Arc::new(openagents_web::pilot::Intake::load(
                    std::path::Path::new(&value),
                )?))
            }
            _ => return Err(USAGE.into()),
        }
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
