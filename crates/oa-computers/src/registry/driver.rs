use super::{Binding, Catalog, Refused};
use crate::store::Entry;
use computer_driver::{
    Capabilities, CommandCursor, CommandRead, CommandSpec, CreateRequest, Driver, Exec, ExecOutput,
    ImageRecord, Meter, Observation, Outcome, Profile, StorageObservation,
};
use std::collections::BTreeMap;

/// Resolve the stored computer before constructing an effect-capable driver.
pub trait Drivers: Clone {
    type Bound: Driver + Clone;
    fn resolve(&self, entry: &Entry) -> Result<Self::Bound, Refused>;
    fn legacy_catalog(&self) -> Option<&Catalog> {
        None
    }
}

/// Compatibility for the existing single-driver Rust helpers. These helpers
/// cannot operate on a row assigned to a registry deployment.
impl<D: Driver + Clone> Drivers for D {
    type Bound = BoundDriver<D>;
    fn resolve(&self, entry: &Entry) -> Result<Self::Bound, Refused> {
        let caps = self.capabilities();
        if entry.driver != caps.driver || entry.binding.is_some() {
            return Err(Refused::Unavailable);
        }
        Ok(BoundDriver {
            inner: self.clone(),
            caps,
            binding: None,
            deleting: entry.desired == crate::store::Desired::Deleted,
        })
    }
}

#[derive(Clone)]
pub struct Registry<D> {
    catalog: Catalog,
    drivers: BTreeMap<String, D>,
}
impl<D: Driver + Clone> Registry<D> {
    pub fn new(mut catalog: Catalog, drivers: BTreeMap<String, D>) -> Result<Self, Refused> {
        for (id, target) in &mut catalog.targets {
            let Some(driver) = drivers.get(id) else {
                target.enabled = false;
                continue;
            };
            let actual = driver.capabilities();
            if actual.driver != target.caps.driver
                || actual.workspace_attachment != target.caps.workspace_attachment
                || !target.caps.profiles.is_subset(&actual.profiles)
                || target.caps.memory_suspend && !actual.memory_suspend
                || target.caps.command_sessions && !actual.command_sessions
                || target.caps.environment_artifacts && !actual.environment_artifacts
                || !actual.sizes.is_empty()
                    && target
                        .caps
                        .sizes
                        .iter()
                        .any(|size| !actual.sizes.contains(size))
                || target.caps.shapes.iter().any(|(name, shape)| {
                    actual
                        .shapes
                        .get(name)
                        .is_some_and(|declared| declared != shape)
                })
                || actual
                    .default_artifact
                    .as_ref()
                    .is_some_and(|artifact| artifact.kind != target.artifact_kind)
                || target
                    .caps
                    .workspace_file_bytes
                    .is_some_and(|n| actual.workspace_file_bytes.is_none_or(|limit| n > limit))
            {
                return Err(Refused::Configuration);
            }
        }
        if drivers.keys().any(|id| !catalog.targets.contains_key(id)) {
            return Err(Refused::Configuration);
        }
        Ok(Self { catalog, drivers })
    }
    #[cfg(feature = "space")]
    pub(crate) fn drivers_mut(&mut self) -> impl Iterator<Item = &mut D> {
        self.drivers.values_mut()
    }

    pub fn catalog(&self) -> &Catalog {
        &self.catalog
    }
}
impl<D: Driver + Clone> Drivers for Registry<D> {
    type Bound = BoundDriver<D>;
    fn legacy_catalog(&self) -> Option<&Catalog> {
        Some(&self.catalog)
    }
    fn resolve(&self, entry: &Entry) -> Result<Self::Bound, Refused> {
        let target = self.catalog.for_entry(entry)?;
        let binding = entry.binding.as_ref().ok_or(Refused::Unavailable)?;
        let driver = self.drivers.get(&target.id).ok_or(Refused::Unavailable)?;
        Ok(BoundDriver {
            inner: driver.clone(),
            caps: target.caps.clone(),
            binding: Some(binding.clone()),
            deleting: entry.desired == crate::store::Desired::Deleted,
        })
    }
}

#[derive(Clone)]
pub struct BoundDriver<D> {
    inner: D,
    caps: Capabilities,
    binding: Option<Binding>,
    /// Delete explicitly discards saved programs; ordinary stop must keep them.
    deleting: bool,
}
macro_rules! forward {
    ($($name:ident($($arg:ident:$ty:ty),*) -> $out:ty;)+)=>{$(
        async fn $name(&self,$($arg:$ty),*)->$out {self.inner.$name($($arg),*).await}
    )+};
}
impl<D: Driver> Driver for BoundDriver<D> {
    fn capabilities(&self) -> Capabilities {
        self.caps.clone()
    }
    async fn create(&self, request: &CreateRequest) -> Outcome<String> {
        if let Some(binding) = &self.binding {
            if !binding.migrated && !super::supports_needs(&self.caps, &binding.needs) {
                return Outcome::failed("This computer can't start with its saved settings.");
            }
            // A size's meaning may have changed while this create was pending.
            // Existing resources can still be observed or deleted below.
            if !binding.migrated && self.caps.shapes.get(&binding.size) != binding.shape.as_ref() {
                return Outcome::failed("This computer's size is no longer available.");
            }
            if !binding.migrated {
                let image = binding.artifact.as_ref().and_then(|artifact| {
                    (artifact.kind != computer_driver::ArtifactKind::Test
                        || artifact.reference != "test:empty")
                        .then_some(artifact.reference.as_str())
                });
                if request.size.as_deref() != Some(binding.size.as_str())
                    || request.image.as_deref() != image
                {
                    return Outcome::failed(
                        "This computer's saved settings do not match the request.",
                    );
                }
            }
            let mut request = request.clone();
            request.operation = binding.operation(&request.operation);
            if self.caps.workspace_attachment {
                request.ttl_seconds = None;
            }
            return self.inner.create(&request).await;
        }
        self.inner.create(request).await
    }
    async fn fork(&self, computer: &str, operation: &str) -> Outcome<String> {
        if !self.caps.supports(Profile::Fork) {
            return Outcome::failed("unsupported: This computer cannot be copied.");
        }
        let operation = self
            .binding
            .as_ref()
            .map_or_else(|| operation.into(), |binding| binding.operation(operation));
        self.inner.fork(computer, &operation).await
    }
    async fn start_command(&self, computer: &str, spec: &CommandSpec) -> Outcome<String> {
        if self
            .binding
            .as_ref()
            .is_some_and(|binding| !binding.migrated)
            && !self.caps.command_sessions
        {
            return Outcome::failed("unsupported: Commands are unavailable on this computer.");
        }
        self.inner.start_command(computer, spec).await
    }
    async fn suspend(&self, computer: &str) -> Outcome<String> {
        if !self.deleting
            && self
                .binding
                .as_ref()
                .is_some_and(|binding| binding.needs.memory_suspend)
            && !self.caps.memory_suspend
        {
            return Outcome::failed(
                "unsupported: This computer cannot keep running programs when stopped.",
            );
        }
        if !self.caps.supports(Profile::Suspend) {
            return Outcome::failed("unsupported: This computer cannot be stopped and resumed.");
        }
        self.inner.suspend(computer).await
    }
    async fn resume(&self, computer: &str) -> Outcome<String> {
        if self
            .binding
            .as_ref()
            .is_some_and(|binding| binding.needs.memory_suspend)
            && !self.caps.memory_suspend
        {
            return Outcome::failed(
                "unsupported: This computer cannot restore its running programs.",
            );
        }
        if !self.caps.supports(Profile::Suspend) {
            return Outcome::failed("unsupported: This computer cannot be stopped and resumed.");
        }
        self.inner.resume(computer).await
    }
    async fn suspend_operation(&self, computer: &str, operation: &str) -> Outcome<String> {
        if !self.caps.supports(Profile::Suspend)
            || !self.deleting
                && self
                    .binding
                    .as_ref()
                    .is_some_and(|b| b.needs.memory_suspend)
                && !self.caps.memory_suspend
        {
            return Outcome::failed(
                "unsupported: This computer cannot keep running programs when stopped.",
            );
        }
        let operation = self
            .binding
            .as_ref()
            .map_or_else(|| operation.into(), |b| b.operation(operation));
        self.inner.suspend_operation(computer, &operation).await
    }
    async fn resume_operation(&self, computer: &str, operation: &str) -> Outcome<String> {
        if !self.caps.supports(Profile::Suspend)
            || self
                .binding
                .as_ref()
                .is_some_and(|b| b.needs.memory_suspend)
                && !self.caps.memory_suspend
        {
            return Outcome::failed(
                "unsupported: This computer cannot restore its running programs.",
            );
        }
        let operation = self
            .binding
            .as_ref()
            .map_or_else(|| operation.into(), |b| b.operation(operation));
        self.inner.resume_operation(computer, &operation).await
    }
    async fn snapshot(&self, computer: &str, name: &str) -> Outcome<String> {
        if !self.caps.supports(Profile::Snapshot) {
            return Outcome::failed("unsupported: This computer cannot save disk images.");
        }
        let name = self
            .binding
            .as_ref()
            .filter(|binding| !binding.migrated)
            .map(|binding| {
                format!(
                    "oa-{}",
                    &binding.operation(&format!("snapshot:{name}"))[..60]
                )
            })
            .unwrap_or_else(|| name.into());
        self.inner.snapshot(computer, &name).await
    }
    async fn snapshot_ready(&self, name: &str) -> Outcome<bool> {
        if !self.caps.supports(Profile::Snapshot) {
            return Outcome::failed("unsupported: This computer cannot save disk images.");
        }
        self.inner.snapshot_ready(name).await
    }
    async fn delete_snapshot(&self, name: &str) -> Outcome<()> {
        if !self.caps.supports(Profile::Snapshot) {
            return Outcome::failed("unsupported: This computer cannot save disk images.");
        }
        self.inner.delete_snapshot(name).await
    }
    forward! {
        capture_environment_image(computer:&str,name:&str)->Outcome<ImageRecord>;
        read_environment_image(name:&str)->Outcome<Option<ImageRecord>>;
        delete_environment_image(name:&str)->Outcome<bool>;
        environment_image_hydrated(computer:&str)->Outcome<bool>;

        observe(computer:&str)->Outcome<Observation>;
        exec(computer:&str,exec:&Exec)->Outcome<ExecOutput>;
        read_command(computer:&str,id:&str,cursor:CommandCursor,max_bytes:u64)->Outcome<CommandRead>;
        stop_command(computer:&str,id:&str)->Outcome<String>;
        read_workspace_file(computer:&str,path:&str)->Result<Vec<u8>,computer_files::FileError>;
        write_workspace_file(computer:&str,path:&str,bytes:&[u8],overwrite:bool)->Result<computer_files::FileInfo,computer_files::FileError>;
        put_file(computer:&str,path:&str,bytes:&[u8])->Outcome<()>;
        get_file(computer:&str,path:&str)->Outcome<Vec<u8>>;
        meter(computer:&str)->Outcome<Meter>;
        storage(computer:&str)->Outcome<StorageObservation>;
        delete(computer:&str)->Outcome<()>;
    }
}
