//! Capture every screen of the desktop app for Design Lab (#12010), offscreen.
//! Nothing is shown on screen: each shot renders with Metal into an image.
//!
//! cargo run -p openagents-ui --example design-lab-tour --features account-fixture -- \
//!     OUT_DIR [--light | --dark] [--only NAME[,NAME...]] [--list]
//!
//! It writes `<area>--<screen>.png` and `<area>--<screen>@dark.png` into
//! OUT_DIR (light and dark unless `--light` or `--dark` picks one). `--only`
//! keeps shots whose `<area>--<screen>` or area matches a name. The signed-in
//! account is a fixture served from loopback; credentials stay in memory, and
//! no engine, real account or chat is used. `scripts/design-lab-capture-desktop.sh`
//! runs it into a Design Lab capture run.
use ::openagents_account::{
    Controller,
    openagents::{Client, CredentialRecord, CredentialStore, Deployment, StorageError},
};
use axum::{Json, Router, routing::get};
use openagents_ui::shell::design_lab_tour::{Tour, shots};
use serde_json::json;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

#[derive(Default)]
struct Store(Mutex<Option<CredentialRecord>>);
impl CredentialStore for Store {
    fn load(&self, _: &Deployment) -> Result<Option<CredentialRecord>, StorageError> {
        Ok(self.0.lock().unwrap().clone())
    }
    fn save(&self, r: &CredentialRecord) -> Result<(), StorageError> {
        *self.0.lock().unwrap() = Some(r.clone());
        Ok(())
    }
    fn clear(&self, _: &Deployment) -> Result<(), StorageError> {
        *self.0.lock().unwrap() = None;
        Ok(())
    }
}

fn main() -> anyhow::Result<()> {
    let mut out = None;
    let mut modes = vec![false, true];
    let mut only: Vec<String> = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--light" => modes = vec![false],
            "--dark" => modes = vec![true],
            "--only" => only.extend(
                args.next()
                    .expect("--only needs a name")
                    .split(',')
                    .map(str::to_owned),
            ),
            "--list" => {
                for shot in shots() {
                    println!("{}", shot.name());
                }
                return Ok(());
            }
            _ if out.is_none() && !arg.starts_with("--") => out = Some(PathBuf::from(arg)),
            _ => anyhow::bail!("usage: design-lab-tour OUT_DIR [--light|--dark] [--only NAME]"),
        }
    }
    let out = out.ok_or_else(|| anyhow::anyhow!("pass the folder to write the pictures to"))?;
    std::fs::create_dir_all(&out)?;
    let scratch = tempfile::tempdir()?;

    // A fixture account on loopback: the sidebar's account row and the Runs
    // on menu show a signed-in person without any real sign-in.
    let runtime = tokio::runtime::Runtime::new()?;
    let origin = runtime.block_on(async {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let origin = format!("http://{}", listener.local_addr()?);
        let account =
            json!({"account": {"id": "acct_fixture", "label": "Alex Rivera"}, "workspaces": []});
        let empty = json!({"data": [], "next": null});
        let router = Router::new()
            .route("/v1/account", get(move || async move { Json(account) }))
            .fallback(get(move || async move { Json(empty) }));
        tokio::spawn(async move { axum::serve(listener, router).await });
        anyhow::Ok(origin)
    })?;
    let controller = |store: Arc<Store>, profile: &str| -> anyhow::Result<Arc<Controller>> {
        let deployment = Deployment::new(&format!("{origin}/v1"), &origin)?;
        Ok(Controller::new(
            Client::new(deployment)?,
            store,
            scratch.path().join(profile),
        ))
    };
    let signed_in_store = Arc::new(Store::default());
    signed_in_store.save(&serde_json::from_value(json!({
        "deployment": Deployment::new(&format!("{origin}/v1"), &origin)?,
        "account": {"id": "acct_fixture", "label": "Alex Rivera"},
        "expires_at": null,
        "credential": format!("sess_{:064x}", 1),
    }))?)?;
    let signed_in = controller(signed_in_store, "profile/signed-in")?;
    runtime.block_on(signed_in.restore());
    anyhow::ensure!(signed_in.lease().is_some(), "fixture sign-in failed");
    let signed_out = controller(Arc::new(Store::default()), "profile/signed-out")?;

    let new_app = || openagents_gpui_test::headless_app(Arc::new(openagents_ui::icons::Assets));
    let tour = Tour {
        new_app: &new_app,
        tokio: runtime.handle().clone(),
        signed_in,
        signed_out,
        scratch: scratch.path().join("shots"),
    };
    let keep = |shot: &openagents_ui::shell::design_lab_tour::Shot| {
        only.is_empty() || only.iter().any(|n| *n == shot.name() || n == shot.area)
    };
    let report = tour.run(&out, &keep, &modes);
    for path in &report.written {
        println!("Wrote {}", path.display());
    }
    for (file, why) in &report.failed {
        eprintln!("Couldn't draw {file}: {why}");
    }
    println!(
        "{} pictures in {}, {} failed",
        report.written.len(),
        out.display(),
        report.failed.len()
    );
    let _ = scratch.close();
    // Exit at once: the headless apps hold windows and tasks we don't need
    // to tear down one by one.
    std::process::exit(if report.failed.is_empty() { 0 } else { 1 });
}
