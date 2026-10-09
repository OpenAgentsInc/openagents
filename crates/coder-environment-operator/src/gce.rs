//! The optional GCE adapter for the packaged owners (ENV-09).
//!
//! `"provider": "gce"` with a `gce` section in the owners' config runs
//! setup, builder, and verifier machines as dedicated GCE instances
//! ([`coder_working_computer::gce`]) instead of Boat sandboxes. Each owner
//! gets its own provider: setup and builder machines carry the configured
//! named credentials, verifier machines none.
//!
//! The [`janitor`] reconciles GCE against the retained records at most
//! every [`RECONCILE_EVERY`]: it deletes owned instances no record holds,
//! and writes `environment-gce/reconciliation.json` under the operator
//! state with every owned instance and image, their sizes, run times, and
//! whether a record or saved version still references them. Images are
//! never deleted by the janitor.

use crate::{Config, Janitor, Layout, Providers};
use coder_cloud::runtime::Credentials;
use coder_working_computer::Computer;
use coder_working_computer::boat::BoatProvider;
use coder_working_computer::gce::{Compute, GceProvider, Gcloud, Reconciliation, Retained};
use coder_working_computer::provider::{
    CheckpointEvidence, CommandCursor, CommandRead, CommandSpec, Commands, ImageRecord, Images,
    Inspection, Meter, Outcome, Provider,
};
use coder_working_computer::{Checkpoint, ServiceDecl};
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// The least time between two GCE reconciliations.
pub const RECONCILE_EVERY: Duration = Duration::from_secs(300);

/// The provider the config selects.
pub enum Selected<G = Gcloud> {
    Boat(Box<BoatProvider>),
    Gce(GceProvider<G>),
}

/// The three providers `config` selects.
pub async fn providers(config: &Config) -> Result<Providers<Selected>, String> {
    config.validate()?;
    match &config.gce {
        None => {
            let p = crate::boat::providers(config).await?;
            Ok(Providers {
                setup: Selected::Boat(Box::new(p.setup)),
                build: Selected::Boat(Box::new(p.build)),
                verify: Selected::Boat(Box::new(p.verify)),
            })
        }
        Some(gce) => {
            let names: Vec<String> = config.credential_names.iter().cloned().collect();
            let credentials = Credentials::from_names(&names, |n| std::env::var(n).ok())?;
            let with = |credentials: Credentials| {
                GceProvider::new(gce.clone(), credentials, config.workdir.clone())
                    .map(Selected::Gce)
            };
            Ok(Providers {
                setup: with(credentials.clone())?,
                build: with(credentials)?,
                verify: with(Credentials::default())?,
            })
        }
    }
}

/// The janitor for `config`: `None` on Boat.
pub fn janitor_for(config: &Config) -> Result<Option<Janitor>, String> {
    match &config.gce {
        None => Ok(None),
        Some(gce) => {
            let provider =
                GceProvider::new(gce.clone(), Credentials::default(), config.workdir.clone())?;
            Ok(Some(janitor(Arc::new(provider), RECONCILE_EVERY)))
        }
    }
}

/// Every instance and image the records hold, read from the operator
/// state.
pub fn retained(layout: &Layout) -> Result<Retained, String> {
    let mut out = Retained::default();
    let store = coder_working_computer::store::Store::under(layout.computers());
    let entries = match fs::read_dir(layout.computers()) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(out),
        Err(_) => return Err("Cannot list the environment computers.".into()),
    };
    for entry in entries {
        let path = entry
            .map_err(|_| "Cannot read the computer directory.")?
            .path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let Some(id) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        let c = store
            .read(id)
            .map_err(|e| format!("Cannot read computer {id}: {e:?}"))?;
        out.hold(&c);
    }
    let envs = coder_environment::store::Store::under(layout.environments())
        .list()
        .map_err(|e| format!("Cannot list environments: {e:?}"))?;
    for env in envs {
        for image in env
            .builds
            .iter()
            .filter_map(|b| b.image.as_ref())
            .chain(env.versions.iter().map(|v| &v.image))
        {
            out.hold_image(image);
        }
    }
    Ok(out)
}

/// One reconciliation: list GCE, read the records, delete owned orphan
/// instances, and retain the report.
pub async fn reconcile<G: Compute>(
    provider: &GceProvider<G>,
    layout: &Layout,
) -> Result<Reconciliation, String> {
    let report = provider.reconcile(|| retained(layout)).await?;
    let swept = provider.sweep(&report).await;
    let dir = layout.state().join("environment-gce");
    fs::create_dir_all(&dir).map_err(|_| "Cannot create the GCE state directory.")?;
    let doc = serde_json::json!({
        "schema": "openagents.environment.gce.reconciliation.v1",
        "at_ms": crate::now_ms(),
        "report": report,
        "swept": swept.iter().map(|(name, o)| serde_json::json!({
            "instance": name,
            "outcome": o,
        })).collect::<Vec<_>>(),
    });
    let path = dir.join("reconciliation.json");
    let temp = dir.join("reconciliation.json.writing");
    fs::write(
        &temp,
        serde_json::to_vec_pretty(&doc).expect("report encodes"),
    )
    .and_then(|_| fs::rename(&temp, &path))
    .map_err(|_| "Cannot retain the GCE reconciliation.")?;
    Ok(report)
}

/// Reconcile at most once per `every`.
pub fn janitor<G: Compute + Send + Sync + 'static>(
    provider: Arc<GceProvider<G>>,
    every: Duration,
) -> Janitor {
    let last: Arc<Mutex<Option<Instant>>> = Arc::new(Mutex::new(None));
    Arc::new(move |layout: Layout| {
        let provider = provider.clone();
        let last = last.clone();
        Box::pin(async move {
            {
                let mut l = last.lock().expect("janitor clock");
                if l.is_some_and(|t| t.elapsed() < every) {
                    return Ok(None);
                }
                *l = Some(Instant::now());
            }
            let report = reconcile(&provider, &layout).await?;
            Ok(Some(format!(
                "{} instances ({} orphaned), {} images ({} unreferenced)",
                report.instances.len(),
                report.orphans().len(),
                report.images.len(),
                report.unreferenced_images().len()
            )))
        })
    })
}

/// The last retained reconciliation, if any.
pub fn last_report(layout: &Layout) -> Option<PathBuf> {
    let p = layout.state().join("environment-gce/reconciliation.json");
    p.exists().then_some(p)
}

macro_rules! each {
    ($self:ident, $p:ident => $e:expr) => {
        match $self {
            Selected::Boat($p) => $e,
            Selected::Gce($p) => $e,
        }
    };
}

impl<G: Compute> Provider for Selected<G> {
    fn kind(&self) -> coder_environment::Provider {
        each!(self, p => p.kind())
    }
    fn admits_base(&self, base: &coder_environment::ImagePin) -> Result<(), &'static str> {
        each!(self, p => p.admits_base(base))
    }
    async fn create(&self, c: &Computer, operation: &str) -> Outcome<String> {
        each!(self, p => p.create(c, operation).await)
    }
    async fn restore(
        &self,
        c: &Computer,
        resource: &str,
        checkpoint: Option<&Checkpoint>,
    ) -> Outcome<String> {
        each!(self, p => p.restore(c, resource, checkpoint).await)
    }
    async fn apply_credentials(&self, c: &Computer, resource: &str) -> Outcome<String> {
        each!(self, p => p.apply_credentials(c, resource).await)
    }
    async fn start_service(
        &self,
        c: &Computer,
        resource: &str,
        service: &ServiceDecl,
    ) -> Outcome<String> {
        each!(self, p => p.start_service(c, resource, service).await)
    }
    async fn checkpoint(
        &self,
        c: &Computer,
        resource: &str,
        generation: u64,
    ) -> Outcome<CheckpointEvidence> {
        each!(self, p => p.checkpoint(c, resource, generation).await)
    }
    async fn shutdown_processes(&self, c: &Computer, resource: &str) -> Outcome<String> {
        each!(self, p => p.shutdown_processes(c, resource).await)
    }
    async fn stop(&self, c: &Computer, resource: &str) -> Outcome<String> {
        each!(self, p => p.stop(c, resource).await)
    }
    async fn meter(&self, c: &Computer, resource: &str) -> Outcome<Meter> {
        each!(self, p => p.meter(c, resource).await)
    }
    async fn delete(&self, c: &Computer, resource: &str) -> Outcome<String> {
        each!(self, p => p.delete(c, resource).await)
    }
    async fn inspect(&self, c: &Computer, resource: &str) -> Outcome<Inspection> {
        each!(self, p => p.inspect(c, resource).await)
    }
}
impl<G: Compute> Commands for Selected<G> {
    async fn start_command(
        &self,
        c: &Computer,
        resource: &str,
        spec: &CommandSpec,
    ) -> Outcome<String> {
        each!(self, p => p.start_command(c, resource, spec).await)
    }
    async fn read_command(
        &self,
        c: &Computer,
        resource: &str,
        id: &str,
        cursor: CommandCursor,
        max_bytes: u64,
    ) -> Outcome<CommandRead> {
        each!(self, p => p.read_command(c, resource, id, cursor, max_bytes).await)
    }
    async fn stop_command(&self, c: &Computer, resource: &str, id: &str) -> Outcome<String> {
        each!(self, p => p.stop_command(c, resource, id).await)
    }
}
impl<G: Compute> Images for Selected<G> {
    async fn capture_image(
        &self,
        c: &Computer,
        resource: &str,
        name: &str,
    ) -> Outcome<ImageRecord> {
        each!(self, p => p.capture_image(c, resource, name).await)
    }
    async fn read_image(&self, name: &str) -> Outcome<Option<ImageRecord>> {
        each!(self, p => p.read_image(name).await)
    }
    async fn hydration(&self, c: &Computer, resource: &str) -> Outcome<bool> {
        each!(self, p => p.hydration(c, resource).await)
    }
    async fn delete_image(&self, name: &str) -> Outcome<bool> {
        each!(self, p => p.delete_image(name).await)
    }
}
