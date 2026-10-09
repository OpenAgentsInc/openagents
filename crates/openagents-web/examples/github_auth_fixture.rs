//! The website with GitHub sign-in, locally, end to end (docs/auth).
//!
//! ```text
//! cargo run -p openagents-web --example github_auth_fixture -- \
//!     NEW_SCRATCH_DIRECTORY 127.0.0.1:4301 \
//!     [--github-oauth ~/work/.secrets/github-oauth-local.json] \
//!     [--cloud-build CLOUD_WASM_DIRECTORY]
//! ```
//!
//! Without `--github-oauth` a fake GitHub runs in-process: "Continue with
//! GitHub" opens its page, where you pick a fake person or cancel. With the
//! owner's local OAuth App file it signs in with real GitHub; that App's
//! callback is `http://127.0.0.1:4301/auth/github/callback`, so listen on
//! 4301. Accounts and sessions are real tenancy stores in the scratch
//! directory, served by `oa_auth::local`. `--cloud-build` also serves the
//! signed-in Cloud pages (Settings, Billing) the account menu links to.
//! A scratch key store under the directory backs Settings, Claude, so a
//! GitHub account can add, see, and remove its own Anthropic API key.

use std::io::Write;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use oa_auth::fake::{self, Fake};
use oa_auth::local::LocalService;
use openagents_web::cloud::session::CloudSession;
use serde_json::json;

const USAGE: &str = "usage: github_auth_fixture NEW_SCRATCH_DIRECTORY 127.0.0.1:PORT [--github-oauth PRIVATE_JSON] [--cloud-build DIRECTORY]";

fn private_file(path: &Path, bytes: &[u8]) -> Result<(), String> {
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .and_then(|mut file| file.write_all(bytes))
        .map_err(|_| "fixture private file creation failed".into())
}

#[tokio::main]
async fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let directory = PathBuf::from(args.next().ok_or(USAGE)?);
    let listen: SocketAddr = args
        .next()
        .ok_or(USAGE)?
        .parse()
        .map_err(|_| "invalid listen address")?;
    let (mut oauth, mut cloud_build) = (None, None);
    while let Some(flag) = args.next() {
        let value = args.next().ok_or(USAGE)?;
        match flag.as_str() {
            "--github-oauth" => oauth = Some(PathBuf::from(value)),
            "--cloud-build" => cloud_build = Some(PathBuf::from(value)),
            _ => return Err(USAGE.into()),
        }
    }
    if !directory.is_absolute() || listen.ip() != IpAddr::V4(Ipv4Addr::LOCALHOST) {
        return Err("fixture requires an absolute directory and a 127.0.0.1 listen address".into());
    }
    std::fs::create_dir(&directory).map_err(|_| "fixture directory must be new")?;
    std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700))
        .map_err(|_| "fixture directory permissions failed")?;
    let directory = directory
        .canonicalize()
        .map_err(|_| "fixture path failed")?;
    let listener = tokio::net::TcpListener::bind(listen)
        .await
        .map_err(|_| "fixture web listener failed")?;
    let address = listener
        .local_addr()
        .map_err(|_| "fixture web address failed")?;
    let origin = format!("http://{address}");
    let redirect = format!("{origin}{}", oa_auth::CALLBACK_PATH);

    // GitHub: the owner's OAuth App, or the in-process fake.
    let (credentials, github_label) = match &oauth {
        Some(path) => (
            oa_auth::GithubCredentials::load(path, &redirect, oa_auth::Endpoints::default())?,
            "github.com".to_string(),
        ),
        None => {
            let (client, secret) = ("Ov23liLocalFake", "local-fake-secret");
            let fake = Fake::new(client, secret, &redirect, vec![fake::octo(), fake::quiet()]);
            let at = fake.spawn().await.map_err(|_| "fake GitHub failed")?;
            (fake::credentials(&at, client, secret, &redirect)?, at)
        }
    };
    let app = credentials.app.clone();

    // The account service over real stores.
    let stores = directory.join("accounts");
    let service = LocalService::install(
        &stores,
        oa_auth::Github::new(credentials)?,
        "local-signup",
        8 * 3600,
    )?;
    let account_service = service
        .spawn()
        .await
        .map_err(|_| "account service failed")?;

    // The web server, pointed at that account service.
    let secret = directory.join("csrf.key");
    private_file(&secret, &secp256k1::rand::random::<[u8; 32]>())?;
    let path = directory.join("cloud.json");
    private_file(
        &path,
        &serde_json::to_vec(&json!({"schema":"openagents.cloud.web-config.v1","public_origin":origin,"account_service":account_service,"csrf_secret":secret}))
            .map_err(|_| "fixture configuration failed")?,
    )?;
    let mut config = openagents_web::Config::development(directory.join("unopened-local-tasks"));
    config.port = address.port();
    config.cloud = Some(Arc::new(CloudSession::load(&path)?));
    config.github = Some(Arc::new(app));
    config.cloud_build = cloud_build;
    config.components_build = std::env::var_os("CLOUD_FIXTURE_COMPONENTS_BUILD").map(PathBuf::from);
    let byo = directory.join("byo");
    std::fs::create_dir_all(&byo).map_err(|_| "fixture credential store failed")?;
    std::fs::set_permissions(&byo, std::fs::Permissions::from_mode(0o700))
        .map_err(|_| "fixture credential store failed")?;
    config.cloud_byo = Some(Arc::new(openagents_web::cloud::byo::Computers::open(&byo)?));

    println!(
        "{}",
        json!({"origin": origin, "login": format!("{origin}/login"), "github": github_label, "account_service": account_service, "stores": stores})
    );
    axum::serve(listener, openagents_web::router(config))
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await
        .map_err(|_| "fixture web server stopped".into())
}
