//! Local directory environments: a private directory per lease, owned by an
//! exact marker. A matching name or path never proves ownership.
use crate::{
    Result,
    provider::{Cost, Create, Lookup, Provider, Realized, Teardown},
};
use nostr::contracts::{digest_bytes, jcs};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use workspace_host::artifact::{MAX_BYTES, atomic, read, token};

const MARKER: &str = ".openagents-env.json";
const MAX_OUTPUTS: usize = 64;

pub struct LocalDirectories {
    root: PathBuf,
}

impl LocalDirectories {
    pub fn new(root: PathBuf) -> Result<Self> {
        let mut builder = std::fs::DirBuilder::new();
        builder.recursive(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder.create(&root).map_err(|e| e.to_string())?;
        Ok(Self { root })
    }
    fn name(account: &str, key: &str) -> String {
        digest_bytes(format!("{account}\n{key}").as_bytes())[7..39].to_string()
    }
    fn path(&self, resource: &str) -> Result<PathBuf> {
        if resource.len() != 32 || !resource.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("identity_mismatch: local resource identity".into());
        }
        Ok(self.root.join(resource))
    }
    fn marker(&self, resource: &str) -> Result<Option<(Value, Vec<u8>)>> {
        let path = self.path(resource)?.join(MARKER);
        if !path.exists() {
            return Ok(None);
        }
        let bytes = read(&path, 4096)?;
        let value = nostr::environment::parse(&bytes).map_err(|e| e.to_string())?;
        Ok(Some((value, bytes)))
    }
    fn realized(value: &Value, bytes: Vec<u8>) -> Result<Realized> {
        Ok(Realized {
            resource: value["resource"].as_str().ok_or("marker resource")?.into(),
            generation: value["generation"]
                .as_str()
                .ok_or("marker generation")?
                .into(),
            account: value["account"].as_str().ok_or("marker account")?.into(),
            ownership: bytes,
        })
    }
    /// The directory a realized resource occupies, for an attached worker.
    pub fn directory(&self, resource: &Realized) -> Result<PathBuf> {
        self.path(&resource.resource)
    }
    fn exact(&self, resource: &Realized) -> Result<PathBuf> {
        let Some((_, bytes)) = self.marker(&resource.resource)? else {
            return Err("unavailable: local resource is absent".into());
        };
        if bytes != resource.ownership {
            return Err("identity_mismatch: local ownership generation changed".into());
        }
        self.path(&resource.resource)
    }
}

fn receipt(action: &str, resource: &Realized, observed: &str) -> Vec<u8> {
    jcs(
        &json!({"provider":"local-directory","action":action,"resource":resource.resource,
        "generation":resource.generation,"observed":observed}),
    )
    .unwrap_or_default()
}

fn files(dir: &Path) -> Result<Vec<(String, Vec<u8>)>> {
    let mut out = Vec::new();
    if !dir.exists() {
        return Ok(out);
    }
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .map_err(|e| e.to_string())?
        .collect::<std::result::Result<_, _>>()
        .map_err(|e| e.to_string())?;
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        if out.len() >= MAX_OUTPUTS {
            return Err("limit_exceeded: too many retained outputs".into());
        }
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| "malformed: output name")?;
        if !entry.file_type().map_err(|e| e.to_string())?.is_file() {
            return Err("unsupported_feature: only regular output files are retained".into());
        }
        out.push((name, read(&entry.path(), MAX_BYTES)?));
    }
    Ok(out)
}

impl Provider for LocalDirectories {
    fn binding(&self) -> String {
        "environment/local-directory".into()
    }
    fn create(&mut self, account: &str, key: &str) -> Create {
        let name = Self::name(account, key);
        match self.marker(&name) {
            Ok(Some((value, bytes))) if value["key"] == key && value["account"] == account => {
                return match Self::realized(&value, bytes) {
                    Ok(realized) => Create::Created(realized, Cost::Measured(0)),
                    Err(e) => Create::Unknown(e),
                };
            }
            Ok(Some(_)) => return Create::Unknown("conflicting local marker".into()),
            Ok(None) => {}
            Err(e) => return Create::Unknown(e),
        }
        let marker = json!({"v":"openagents.environment-ownership.v1","requires":[],
            "provider":self.binding(),"account":account,"resource":name,"generation":token(),
            "key":key,"observed_at":crate::now()});
        let result = (|| -> Result<Realized> {
            let path = self.path(&name)?;
            let mut builder = std::fs::DirBuilder::new();
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt;
                builder.mode(0o700);
            }
            match builder.create(&path) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    return Err(
                        "unknown: an unmarked resource already occupies this identity".into(),
                    );
                }
                Err(e) => return Err(e.to_string()),
            }
            let bytes = jcs(&marker).map_err(|e| e.to_string())?;
            atomic(&path.join(MARKER), &bytes)?;
            Self::realized(&marker, bytes)
        })();
        match result {
            Ok(realized) => Create::Created(realized, Cost::Measured(0)),
            Err(e) => Create::Unknown(e),
        }
    }
    fn lookup(&mut self, account: &str, key: &str) -> Lookup {
        let name = Self::name(account, key);
        match self.marker(&name) {
            Ok(Some((value, bytes))) if value["key"] == key && value["account"] == account => {
                match Self::realized(&value, bytes) {
                    Ok(realized) => Lookup::Found(realized),
                    Err(e) => Lookup::Unknown(e),
                }
            }
            Ok(Some(_)) => Lookup::Unknown("conflicting local marker".into()),
            Ok(None) => match self.path(&name) {
                // A directory without a marker may be a partial create.
                Ok(path) if path.exists() => Lookup::Unknown("unmarked partial resource".into()),
                Ok(_) => Lookup::Absent,
                Err(e) => Lookup::Unknown(e),
            },
            Err(e) => Lookup::Unknown(e),
        }
    }
    fn adopt(
        &mut self,
        account: &str,
        resource: &str,
        generation: &str,
        ownership: &[u8],
    ) -> Result<Realized> {
        let Some((value, bytes)) = self.marker(resource)? else {
            return Err("not_admitted: no ownership record for that resource".into());
        };
        if bytes != ownership
            || value["account"] != account
            || value["generation"] != generation
            || value["resource"] != resource
            || value["provider"] != self.binding().as_str()
        {
            return Err("identity_mismatch: ownership evidence does not match".into());
        }
        Self::realized(&value, bytes)
    }
    fn materialize(
        &mut self,
        resource: &Realized,
        components: &[(String, Vec<u8>)],
    ) -> Result<Vec<(String, String)>> {
        let dir = self.exact(resource)?.join("components");
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let mut observed = Vec::new();
        for (id, bytes) in components {
            atomic(&dir.join(id), bytes)?;
            observed.push((id.clone(), digest_bytes(&read(&dir.join(id), MAX_BYTES)?)));
        }
        Ok(observed)
    }
    fn collect(&mut self, resource: &Realized) -> Result<Vec<(String, Vec<u8>)>> {
        files(&self.exact(resource)?.join("out"))
    }
    fn destroy(&mut self, resource: &Realized) -> Teardown {
        let path = match self.exact(resource) {
            Ok(path) => path,
            Err(e) if e.starts_with("unavailable") => {
                return Teardown::Unknown("resource already absent; ownership unproven".into());
            }
            Err(e) => return Teardown::Failed(e),
        };
        if let Err(e) = std::fs::remove_dir_all(&path) {
            return Teardown::Unknown(e.to_string());
        }
        if path.exists() {
            return Teardown::Unknown("resource still present".into());
        }
        Teardown::Confirmed(receipt("destroy", resource, "absent"), Cost::Measured(0))
    }
    fn reconcile_teardown(&mut self, resource: &Realized, action: &str) -> Teardown {
        if action == "detach" {
            return self.detach(resource);
        }
        match self.path(&resource.resource) {
            Ok(path) if !path.exists() => Teardown::Confirmed(
                receipt("destroy", resource, "observed-absent"),
                Cost::Measured(0),
            ),
            Ok(_) => {
                Teardown::Unknown("the original resource has not been confirmed absent".into())
            }
            Err(e) => Teardown::Unknown(e),
        }
    }
    fn detach(&mut self, resource: &Realized) -> Teardown {
        match self.exact(resource) {
            Ok(_) => Teardown::Confirmed(receipt("detach", resource, "running"), Cost::Measured(0)),
            Err(e) => Teardown::Failed(e),
        }
    }
}
