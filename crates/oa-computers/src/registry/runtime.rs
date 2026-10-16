//! Configured connections. Only credential environment-variable names appear
//! in configuration; credential values stay inside their HTTP clients.
use super::{Catalog, Placement, Refused, Registry, Target};
use computer_driver::{
    Artifact, ArtifactKind, Capabilities, CommandCursor, CommandRead, CommandSpec, CreateRequest,
    Driver, Exec, ExecOutput, Meter, Observation, Outcome, Shape,
};
use serde::Deserialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    time::Duration,
};

#[derive(Clone)]
pub enum RuntimeDriver {
    Boat(computer_driver::boat::BoatDriver),
    Fake(computer_driver::fake::FakeDriver),
    #[cfg(feature = "space")]
    Space(computer_driver::space::SpaceDriver),
}
macro_rules! forward {
    ($($name:ident($($arg:ident:$ty:ty),*)->$out:ty;)+)=>{$(
        async fn $name(&self,$($arg:$ty),*)->$out {
            match self {
                Self::Boat(driver)=>driver.$name($($arg),*).await,
                Self::Fake(driver)=>driver.$name($($arg),*).await,
                #[cfg(feature="space")]
                Self::Space(driver)=>driver.$name($($arg),*).await,
            }
        }
    )+};
}
impl Driver for RuntimeDriver {
    fn capabilities(&self) -> Capabilities {
        match self {
            Self::Boat(driver) => driver.capabilities(),
            Self::Fake(driver) => driver.capabilities(),
            #[cfg(feature = "space")]
            Self::Space(driver) => driver.capabilities(),
        }
    }
    forward! {
        create(request:&CreateRequest)->Outcome<String>;
        observe(computer:&str)->Outcome<Observation>;
        exec(computer:&str,exec:&Exec)->Outcome<ExecOutput>;
        start_command(computer:&str,spec:&CommandSpec)->Outcome<String>;
        read_command(computer:&str,id:&str,cursor:CommandCursor,max_bytes:u64)->Outcome<CommandRead>;
        stop_command(computer:&str,id:&str)->Outcome<String>;
        read_workspace_file(computer:&str,path:&str)->Result<Vec<u8>,computer_files::FileError>;
        write_workspace_file(computer:&str,path:&str,bytes:&[u8],overwrite:bool)->Result<computer_files::FileInfo,computer_files::FileError>;
        put_file(computer:&str,path:&str,bytes:&[u8])->Outcome<()>;
        get_file(computer:&str,path:&str)->Outcome<Vec<u8>>;
        meter(computer:&str)->Outcome<Meter>;
        delete(computer:&str)->Outcome<()>;
        suspend(computer:&str)->Outcome<String>;
        resume(computer:&str)->Outcome<String>;
        snapshot(computer:&str,name:&str)->Outcome<String>;
        snapshot_ready(name:&str)->Outcome<bool>;
        delete_snapshot(name:&str)->Outcome<()>;
        fork(computer:&str,operation:&str)->Outcome<String>;
    }
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Kind {
    Boat,
    Space,
    Fake,
}
impl Kind {
    fn driver(self) -> &'static str {
        match self {
            Self::Boat => "oa-boat",
            Self::Space => "space",
            Self::Fake => "fake",
        }
    }
    fn artifact(self) -> ArtifactKind {
        match self {
            Self::Boat => ArtifactKind::GceImage,
            Self::Space => ArtifactKind::SpaceTemplate,
            Self::Fake => ArtifactKind::Test,
        }
    }
    fn placement(self) -> Placement {
        match self {
            Self::Space => Placement::FastSandbox,
            _ => Placement::OurCloud,
        }
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Configuration {
    default: String,
    policy: String,
    deployments: Vec<Deployment>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Deployment {
    id: String,
    kind: Kind,
    #[serde(default)]
    api_url: Option<String>,
    #[serde(default)]
    data_plane_url: Option<String>,
    #[serde(default)]
    credential_env: Option<String>,
    #[serde(default)]
    template: Option<String>,
    #[serde(default)]
    fake_root: Option<std::path::PathBuf>,
    #[serde(default)]
    placement: Option<Placement>,
    #[serde(default)]
    priority: u32,
    #[serde(default = "yes")]
    enabled: bool,
    #[serde(default = "yes")]
    admit_new: bool,
    #[serde(default)]
    capacity: Option<u32>,
    #[serde(default)]
    rollout_accounts: Option<BTreeSet<String>>,
    #[serde(default)]
    legacy: bool,
    #[serde(default)]
    shapes: BTreeMap<String, Shape>,
    /// A non-secret provider tenant/installation name, when URLs alone are ambiguous.
    #[serde(default)]
    scope: Option<String>,
}
fn yes() -> bool {
    true
}
fn env(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.trim().is_empty())
}
fn settings() -> Refused {
    Refused::Configuration
}
fn credential(name: &str) -> Result<String, Refused> {
    if name.is_empty()
        || name.len() > 128
        || !name
            .bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
    {
        return Err(settings());
    }
    env(name).ok_or(Refused::Unavailable)
}
fn endpoint(value: &str) -> Result<String, Refused> {
    let url = url::Url::parse(value).map_err(|_| settings())?;
    let loopback = url.host_str().is_some_and(|host| {
        host == "localhost"
            || host
                .trim_matches(['[', ']'])
                .parse::<std::net::IpAddr>()
                .is_ok_and(|ip| ip.is_loopback())
    });
    if !(url.scheme() == "https" || url.scheme() == "http" && loopback)
        || url.host().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(settings());
    }
    Ok(url.as_str().trim_end_matches('/').into())
}

impl Configuration {
    fn legacy_environment() -> Result<Self, Refused> {
        let kind = match env("OA_COMPUTERS_DRIVER").as_deref().unwrap_or("boat") {
            "boat" => Kind::Boat,
            "space" => Kind::Space,
            "fake" => Kind::Fake,
            _ => return Err(settings()),
        };
        let id = format!("{}-primary", kind.driver());
        let legacy = match env("OA_COMPUTERS_LEGACY_DEPLOYMENT") {
            Some(value) if value == id => true,
            Some(_) => return Err(settings()),
            None => false,
        };
        Ok(Self {
            default: id.clone(),
            policy: "registry-v1".into(),
            deployments: vec![Deployment {
                id,
                kind,
                api_url: match kind {
                    Kind::Boat => {
                        Some(env("BOAT_API_BASE").unwrap_or_else(|| boat::BASE_URL.into()))
                    }
                    Kind::Space => env("SPACE_API_URL"),
                    Kind::Fake => None,
                },
                data_plane_url: env("SPACE_DATA_PLANE_URL"),
                credential_env: Some(
                    match kind {
                        Kind::Space => "SPACE_API_TOKEN",
                        _ => "BOAT_API_KEY",
                    }
                    .into(),
                ),
                template: env("SPACE_TEMPLATE"),
                fake_root: env("OA_COMPUTERS_FAKE_DIR").map(Into::into),
                placement: None,
                priority: 0,
                enabled: true,
                admit_new: true,
                capacity: None,
                rollout_accounts: None,
                legacy,
                shapes: BTreeMap::new(),
                scope: None,
            }],
        })
    }
}

async fn load(
    deployment: &Deployment,
    api: &str,
    data: Option<&str>,
) -> Result<RuntimeDriver, Refused> {
    match deployment.kind {
        Kind::Boat => {
            let key = boat::ApiKey::new(credential(
                deployment
                    .credential_env
                    .as_deref()
                    .unwrap_or("BOAT_API_KEY"),
            )?)
            .map_err(|_| settings())?;
            let mut builder = boat::Client::builder(key).base_url(api);
            if let Some(scope) = &deployment.scope {
                builder = builder.org(scope);
            }
            let client = builder.build().map_err(|_| settings())?;
            let driver = tokio::time::timeout(
                Duration::from_secs(10),
                computer_driver::boat::BoatDriver::connect(client),
            )
            .await
            .map_err(|_| Refused::Unavailable)?
            .map_err(|_| Refused::Unavailable)?;
            Ok(RuntimeDriver::Boat(driver))
        }
        Kind::Fake => {
            let root = deployment
                .fake_root
                .clone()
                .unwrap_or_else(|| std::env::temp_dir().join("oa-computers-fake"));
            std::fs::create_dir_all(&root).map_err(|_| settings())?;
            Ok(RuntimeDriver::Fake(computer_driver::fake::FakeDriver::new(
                root,
            )))
        }
        Kind::Space => {
            #[cfg(feature = "space")]
            {
                let token = computer_driver::Secret::new(credential(
                    deployment
                        .credential_env
                        .as_deref()
                        .unwrap_or("SPACE_API_TOKEN"),
                )?);
                let driver = computer_driver::space::SpaceDriver::new(
                    api,
                    data.ok_or_else(settings)?,
                    token,
                    deployment.template.as_deref().ok_or_else(settings)?,
                )
                .map_err(|_| settings())?;
                Ok(RuntimeDriver::Space(driver))
            }
            #[cfg(not(feature = "space"))]
            {
                let _ = (&deployment.template, data);
                Err(Refused::Unavailable)
            }
        }
    }
}

pub async fn from_environment() -> Result<Registry<RuntimeDriver>, Refused> {
    let config = if let Some(path) = env("OA_COMPUTERS_REGISTRY_FILE") {
        use std::io::Read;
        let mut bytes = Vec::new();
        std::fs::File::open(path)
            .map_err(|_| settings())?
            .take(256 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| settings())?;
        if bytes.len() > 256 * 1024 {
            return Err(settings());
        }
        serde_json::from_slice::<Configuration>(&bytes).map_err(|_| settings())?
    } else {
        Configuration::legacy_environment()?
    };
    if config.deployments.len() > 16 {
        return Err(settings());
    }
    let mut targets = Vec::new();
    let mut drivers = BTreeMap::new();
    for deployment in config.deployments {
        let api = match deployment.kind {
            Kind::Fake => {
                let root = deployment
                    .fake_root
                    .clone()
                    .unwrap_or_else(|| std::env::temp_dir().join("oa-computers-fake"));
                std::fs::create_dir_all(&root).map_err(|_| settings())?;
                format!(
                    "fake:{}",
                    super::digest(
                        &serde_json::json!({"root":std::fs::canonicalize(root).map_err(|_|settings())?})
                    )
                )
            }
            _ => endpoint(deployment.api_url.as_deref().ok_or_else(settings)?)?,
        };
        let data = deployment
            .data_plane_url
            .as_deref()
            .map(endpoint)
            .transpose()?;
        if deployment
            .scope
            .as_ref()
            .is_some_and(|scope| !super::identifier(scope))
        {
            return Err(settings());
        }
        let connection =
            serde_json::to_string(&(api.as_str(), data.as_deref(), deployment.scope.as_deref()))
                .expect("connection serializes");
        let loaded = if deployment.enabled {
            load(&deployment, &api, data.as_deref()).await.ok()
        } else {
            None
        };
        let mut caps = loaded
            .as_ref()
            .map(Driver::capabilities)
            .unwrap_or_else(|| Capabilities::new(deployment.kind.driver(), [], &[]));
        if matches!(deployment.kind, Kind::Fake) {
            // This backend simulates size classes; it creates directories, not VMs.
            caps.default_artifact = Some(Artifact {
                kind: ArtifactKind::Test,
                reference: "test:empty".into(),
                provider_identity: None,
                disk_gib: None,
            });
            for size in ["small", "default"] {
                caps.shapes.insert(
                    size.into(),
                    Shape {
                        vcpus: 1,
                        memory_mib: 1024,
                        min_disk_gib: None,
                    },
                );
            }
        }
        for (name, shape) in deployment.shapes {
            if caps
                .shapes
                .get(&name)
                .is_some_and(|declared| declared != &shape)
            {
                return Err(settings());
            }
            caps.shapes.insert(name, shape);
        }
        let mut target = Target::new(
            &deployment.id,
            &connection,
            caps,
            deployment.kind.artifact(),
            deployment
                .placement
                .unwrap_or_else(|| deployment.kind.placement()),
        )?;
        target.priority = deployment.priority;
        target.enabled = deployment.enabled;
        target.admit_new = deployment.admit_new;
        target.capacity = deployment.capacity;
        target.rollout_accounts = deployment.rollout_accounts;
        target.legacy_driver = deployment.legacy.then(|| target.caps.driver.clone());
        if let Some(driver) = loaded {
            drivers.insert(target.id.clone(), driver);
        }
        targets.push(target);
    }
    Registry::new(
        Catalog::new(&config.default, &config.policy, targets)?,
        drivers,
    )
}
