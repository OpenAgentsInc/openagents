//! CAD API changes use the same bounded child process as chat tools.
use super::*;
use lab_api::{ApiError, Kind, Tool};
use lab_kit::{Artifact, Card, Change, JobSpec, JobState, JobStatus, LabKind, Version, Want};
use lab_store::{CheckedDraft, LabStore};

struct Working(PathBuf);
impl std::ops::Deref for Working {
    type Target = Path;
    fn deref(&self) -> &Path {
        &self.0
    }
}
impl Drop for Working {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

pub(crate) struct CadApi<'a> {
    engine: &'a Engine,
    prepared: Mutex<HashMap<String, Working>>,
}
impl<'a> CadApi<'a> {
    pub(crate) fn new(engine: &'a Engine) -> Self {
        Self {
            engine,
            prepared: Mutex::new(HashMap::new()),
        }
    }
    async fn prepare(
        &self,
        store: &lab_store::Store,
        content: &Value,
        base: Option<&Version>,
    ) -> lab_store::Result<(Working, Value)> {
        if base.is_some_and(Version::has_private_local) {
            return Err(lab_store::Error::Refused(lab_kit::Refusal::new(
                "host_required",
                "Use the computer that has this design's files.",
            )));
        }
        if !self.engine.admit(store.namespace()) {
            return Err(lab_store::Error::RateLimited);
        }
        let id = content["id"]
            .as_str()
            .filter(|v| valid_doc(v))
            .ok_or_else(|| {
                lab_store::Error::Invalid(
                    "Give this design a name using lowercase letters, digits, and hyphens.".into(),
                )
            })?;
        let dir = self
            .engine
            .cache
            .join("api")
            .join(format!("{:032x}", secp256k1::rand::random::<u128>()));
        tokio::fs::create_dir_all(&dir).await?;
        let dir = Working(dir);
        let result = async {
            if let Some(base) = base {
                imports::restore_shared(store, base, &dir)
                    .await
                    .map_err(invalid)?;
            }
            let _job = tokio::time::timeout(QUEUE_WAIT, self.engine.jobs.acquire())
                .await
                .map_err(|_| {
                    invalid("We're building another model right now. Try again in a minute.")
                })?
                .map_err(|_| invalid("The CAD engine stopped."))?;
            let result = self
                .engine
                .child(
                    &dir,
                    "cad_from_json",
                    &json!({"doc":id,"document":content,"summary":"Checked design"}),
                )
                .await
                .map_err(invalid)?;
            if let Some(message) = result["error"].as_str() {
                return Err(lab_store::Error::Refused(lab_kit::Refusal::new(
                    "cad_check",
                    message,
                )));
            }
            let at = dir.join(id).join("v1");
            let saved: Value =
                serde_json::from_slice(&tokio::fs::read(at.join("doc.json")).await?)?;
            if saved != *content {
                return Err(invalid(
                    "Save a complete part or assembly document before rebuilding it.",
                ));
            }
            let record: Value =
                serde_json::from_slice(&tokio::fs::read(at.join("version.json")).await?)?;
            Ok(cad::lab::measurements(content, record["report"].clone()))
        }
        .await;
        match result {
            Ok(measurements) => Ok((dir, measurements)),
            Err(error) => Err(error),
        }
    }
    fn remember(&self, store: &lab_store::Store, content: &Value, dir: Working) {
        let mut prepared = self.prepared.lock().unwrap_or_else(|e| e.into_inner());
        // Keep at most four checked models while their version saves finish.
        if prepared.len() >= 4 {
            if let Some(key) = prepared.keys().next().cloned() {
                prepared.remove(&key);
            }
        }
        prepared.insert(cache_key(store, content), dir);
    }
}
fn cache_key(store: &lab_store::Store, content: &Value) -> String {
    lab_kit::digest(&json!([store.namespace(), lab_kit::digest(content)]))
}
fn invalid(message: impl Into<String>) -> lab_store::Error {
    lab_store::Error::Invalid(message.into())
}
fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[async_trait::async_trait]
impl Kind for CadApi<'_> {
    fn manifest(&self) -> lab_kit::KindManifest {
        cad::lab::Cad::manifest()
    }
    async fn initial(
        &self,
        store: &lab_store::Store,
        document: &str,
        schema: &str,
        content: Value,
        summary: &str,
    ) -> lab_store::Result<CheckedDraft> {
        if content["id"].as_str() != Some(document) {
            return Err(invalid(
                "Use the same name for the design and its document.",
            ));
        }
        let (dir, measurements) = self.prepare(store, &content, None).await?;
        let draft = CheckedDraft::initial::<cad::lab::Cad>(
            schema,
            content.clone(),
            summary,
            &measurements,
        )?;
        self.remember(store, &content, dir);
        Ok(draft)
    }
    async fn change(
        &self,
        store: &lab_store::Store,
        base: &Version,
        change: &Change,
        summary: &str,
    ) -> lab_store::Result<CheckedDraft> {
        let content = cad::lab::changed_content(&base.content, change)?;
        let (dir, measurements) = self.prepare(store, &content, Some(base)).await?;
        let draft = CheckedDraft::change::<cad::lab::Cad>(base, change, summary, &measurements)?;
        self.remember(store, &content, dir);
        Ok(draft)
    }
    async fn saved(&self, store: &lab_store::Store, version: &Version) -> lab_store::Result<()> {
        let dir = self
            .prepared
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&cache_key(store, &version.content));
        let dir = match dir {
            Some(dir) => dir,
            None => {
                self.prepare(store, &version.content, Some(version))
                    .await?
                    .0
            }
        };
        let result = async {
            let id = version.content["id"]
                .as_str()
                .ok_or(lab_store::Error::Corrupt)?;
            let at = dir.join(id).join("v1");
            for name in FILES {
                if matches!(name, "doc.json" | "version.json") {
                    continue;
                }
                let path = at.join(name);
                if tokio::fs::metadata(&path)
                    .await
                    .is_ok_and(|m| m.len() > MAX_FILE_BYTES as u64)
                {
                    return Err(invalid("This design file is too large."));
                }
                if let Ok(bytes) = tokio::fs::read(path).await {
                    platform::attach(store, &version.reference(), name, bytes, now())
                        .await
                        .map_err(invalid)?;
                }
            }
            Ok(())
        }
        .await;
        drop(dir);
        result
    }
    fn jobs(&self, version: &Version, want: &Want) -> Vec<JobSpec> {
        cad::lab::Cad::jobs_for(version, want)
    }
    fn references(
        &self,
        content: &Value,
    ) -> Result<Vec<lab_api::ResourceReference>, lab_kit::Refusal> {
        if content.get("lab_references").is_none() {
            return Ok(vec![]);
        }
        let references: Vec<lab_api::ResourceReference> =
            serde_json::from_value(content["lab_references"].clone()).map_err(|_| {
                lab_kit::Refusal::new(
                    "reference",
                    "Choose a saved Lab version for each reference.",
                )
            })?;
        if references.len() > 64 {
            return Err(lab_kit::Refusal::new(
                "reference",
                "Use fewer Lab references in this design.",
            ));
        }
        Ok(references)
    }
    fn card(&self, version: &Version, jobs: &[JobState], artifacts: &[Artifact]) -> Card {
        cad::lab::Cad::card(version, jobs, artifacts)
    }
    fn tool(&self, name: &str, args: &Value) -> Result<Tool, ApiError> {
        let document = args["doc"]
            .as_str()
            .ok_or_else(|| ApiError::invalid("Choose a design."))?
            .to_owned();
        match name {
            "cad_from_json" => {
                let mut content = match &args["document"] {
                    Value::String(text) => serde_json::from_str(text)
                        .map_err(|_| ApiError::invalid("Choose a part or assembly document."))?,
                    value => value.clone(),
                };
                if !content.is_object() {
                    return Err(ApiError::invalid("Choose a part or assembly document."));
                }
                if content["kind"] == cad::doc::PART {
                    content["id"] = json!(document);
                }
                let schema = content["kind"]
                    .as_str()
                    .ok_or_else(|| ApiError::invalid("Choose a part or an assembly."))?
                    .into();
                Ok(Tool::Put {
                    title: content["title"].as_str().unwrap_or(&document).into(),
                    document,
                    schema,
                    content,
                    summary: args["summary"].as_str().unwrap_or("Saved design").into(),
                })
            }
            "cad_open" => Ok(Tool::Read { document }),
            "cad_drag_face" => Ok(Tool::Change {
                document,
                summary: "Pulled a face".into(),
                change: Change::Intent {
                    intent: json!({"intent":"pull_face","part":args["part"],"face":args["face"],"delta_mm":args["delta_mm"]}),
                },
            }),
            "cad_export" => Ok(Tool::Job {
                document,
                version: args["version"]
                    .as_u64()
                    .ok_or_else(|| ApiError::invalid("Choose a saved version."))?,
                want: Want::Job {
                    job_type: "export".into(),
                    params: json!({"formats":args["formats"]}),
                },
            }),
            _ => Err(ApiError::missing()),
        }
    }
    fn server_capabilities(&self) -> Option<lab_jobs::Capabilities> {
        Some(lab_jobs::Capabilities {
            worker: "cad-local".into(),
            placement: lab_jobs::Placement::Server,
            cpu_millis: 1000,
            memory_bytes: 640 * 1024 * 1024,
            gpu: false,
            tools: ["cad-lab".into()].into(),
            jobs: [
                "cad/rebuild".into(),
                "cad/export".into(),
                "cad/render".into(),
            ]
            .into(),
            local_assets: Default::default(),
            max_seconds: 75,
        })
    }
    async fn run_server_job(
        &self,
        store: lab_store::Store,
        job: JobState,
        execution: lab_api::ServerExecution,
    ) {
        let Ok(version) = store
            .version(&job.spec.version.document, job.spec.version.version)
            .await
        else {
            return;
        };
        let Ok(requirements) =
            lab_jobs::EffectiveRequirements::for_job(&job.spec, &version, &self.manifest())
        else {
            return;
        };
        if execution.worker.capabilities.placement != lab_jobs::Placement::Server
            || !requirements.eligible(&execution.worker.capabilities)
            || store.namespace()
                != lab_kit::digest(
                    &serde_json::to_value(&execution.worker.space).unwrap_or_default(),
                )
        {
            return;
        }
        if execution
            .authority
            .reserve(&execution.worker, &job.id, now())
            .await
            .is_err()
        {
            return;
        }
        let Ok(lease) = store
            .claim(&job.id, &execution.worker.id, now(), 180_000)
            .await
        else {
            return;
        };
        if execution
            .authority
            .assign(&execution.worker, lease.clone(), now())
            .await
            .is_err()
        {
            return;
        }
        let Ok(lease) = store.start(&lease, now()).await else {
            return;
        };
        let Ok(assignment) = execution
            .authority
            .started(&execution.worker, lease.clone(), now())
            .await
        else {
            return;
        };
        let result = tokio::time::timeout(DEADLINE, self.execute(&store, &job, &lease)).await;
        let (status, message, artifacts) = match result {
            Ok(Ok(artifacts)) => (JobStatus::Done, None, artifacts),
            Ok(Err(_)) => (
                JobStatus::Failed,
                Some("We couldn't finish this CAD step. Try again.".into()),
                vec![],
            ),
            Err(_) => (
                JobStatus::Uncertain,
                Some("This CAD step took too long. Check its files before trying again.".into()),
                vec![],
            ),
        };
        if let Ok(result) = store
            .finish(&lease, status, message, artifacts, now())
            .await
        {
            let fingerprint = lab_kit::digest(&serde_json::to_value(&result).unwrap_or_default());
            let _ = execution
                .authority
                .complete(&execution.worker, &assignment, result, &fingerprint, now())
                .await;
        }
    }
}

impl CadApi<'_> {
    async fn execute(
        &self,
        store: &lab_store::Store,
        job: &JobState,
        lease: &lab_store::Lease,
    ) -> lab_store::Result<Vec<String>> {
        let version = store
            .version(&job.spec.version.document, job.spec.version.version)
            .await?;
        let (dir, _) = self
            .prepare(store, &version.content, Some(&version))
            .await?;
        let result = async {
            let id = version.content["id"]
                .as_str()
                .ok_or(lab_store::Error::Corrupt)?;
            let name = match job.spec.job_type.as_str() {
                "rebuild" => None,
                "render" => Some("cad_render"),
                "export" => Some("cad_export"),
                _ => return Err(invalid("Choose a supported CAD job.")),
            };
            if let Some(name) = name {
                let mut args = json!({"doc":id,"version":1});
                if name == "cad_export" {
                    let formats = job.spec.params["formats"]
                        .as_array()
                        .filter(|f| !f.is_empty() && f.len() <= 4)
                        .ok_or_else(|| invalid("Choose STEP, STL, glTF, or 3MF for the export."))?;
                    if formats
                        .iter()
                        .any(|f| !matches!(f.as_str(), Some("step" | "stl" | "glb" | "3mf")))
                    {
                        return Err(invalid("Choose STEP, STL, glTF, or 3MF for the export."));
                    }
                    args["formats"] = json!(formats);
                } else {
                    args["width"] = json!(
                        job.spec.params["width"]
                            .as_u64()
                            .unwrap_or(1200)
                            .clamp(64, 1600)
                    );
                    args["height"] = json!(
                        job.spec.params["height"]
                            .as_u64()
                            .unwrap_or(800)
                            .clamp(64, 1000)
                    );
                }
                let _job = tokio::time::timeout(QUEUE_WAIT, self.engine.jobs.acquire())
                    .await
                    .map_err(|_| invalid("Try this CAD step again in a minute."))?
                    .map_err(|_| invalid("The CAD engine stopped."))?;
                let answer = self
                    .engine
                    .child(&dir, name, &args)
                    .await
                    .map_err(invalid)?;
                if let Some(message) = answer["error"].as_str() {
                    return Err(invalid(message));
                }
            }
            let at = dir.join(id).join("v1");
            for name in FILES {
                if matches!(name, "doc.json" | "version.json") {
                    continue;
                }
                let path = at.join(name);
                if tokio::fs::metadata(&path)
                    .await
                    .is_ok_and(|m| m.len() > MAX_FILE_BYTES as u64)
                {
                    return Err(invalid("This design file is too large."));
                }
                if let Ok(bytes) = tokio::fs::read(path).await {
                    platform::attach_job(store, lease, &version.reference(), name, bytes, now())
                        .await
                        .map_err(invalid)?;
                }
            }
            if let Ok(mut files) = tokio::fs::read_dir(at.join("out")).await {
                while let Some(file) = files.next_entry().await? {
                    let name = file.file_name().to_string_lossy().into_owned();
                    if !valid_out(&name) || !file.file_type().await?.is_file() {
                        continue;
                    }
                    if file.metadata().await?.len() > MAX_FILE_BYTES as u64 {
                        return Err(invalid("This design file is too large."));
                    }
                    let bytes = tokio::fs::read(file.path()).await?;
                    let name = name.strip_prefix(&format!("{id}-v1.")).map_or_else(
                        || name.clone(),
                        |suffix| format!("{id}-v{}.{suffix}", version.number),
                    );
                    platform::attach_job(
                        store,
                        lease,
                        &version.reference(),
                        &format!("out/{name}"),
                        bytes,
                        now(),
                    )
                    .await
                    .map_err(invalid)?;
                }
            }
            Ok(store
                .version_artifacts(&version.reference())
                .await?
                .into_iter()
                .map(|a| a.id)
                .collect())
        }
        .await;
        drop(dir);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    // This child fixture checks process, persistence, and job behavior. The CAD
    // crate separately tests these documents against the geometry engine.
    const CHILD: &str = r#"#!/usr/bin/python3
import json, os, pathlib, sys
root = pathlib.Path(os.environ['CAD_LAB_DIR'])
args = json.load(sys.stdin)
doc = args['doc']
at = root / doc / 'v1'
at.mkdir(parents=True, exist_ok=True)
out = at / 'out'
out.mkdir(exist_ok=True)
if sys.argv[2] == 'cad_from_json':
    content = args['document']
    (at / 'doc.json').write_text(json.dumps(content))
    (at / 'version.json').write_text(json.dumps({'report': {'checks': {'rebuilt': True}, 'mass': {'mass_kg': 0.05}}}))
    (at / 'assets.json').write_text('[]')
    (at / 'model.glb').write_bytes(b'Synthetic glTF')
    (at / 'hero.png').write_bytes(b'Synthetic poster')
elif sys.argv[2] == 'cad_export':
    (out / 'fan-v1.step').write_bytes(b'ISO-10303-21; Synthetic fixture; END-ISO-10303-21;')
print(json.dumps({'doc': doc, 'version': 1, 'saved': sys.argv[2] == 'cad_from_json'}))
"#;

    #[tokio::test]
    async fn checked_child_versions_cards_and_exports_share_the_store() {
        let root = tempfile::tempdir().unwrap();
        let program = root.path().join("child");
        std::fs::write(&program, CHILD).unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        let mut engine = Engine::new(program);
        engine.cache = root.path().join("cache");
        let api = CadApi::new(&engine);
        let backend =
            Arc::new(lab_store::backend::FsBackend::open(root.path().join("store")).unwrap());
        let store = lab_store::Store::new(
            backend.clone(),
            &lab_kit::SpaceRef::Personal {
                account: "synthetic-alice".into(),
            },
        )
        .unwrap();
        let foreign = lab_store::Store::new(
            backend.clone(),
            &lab_kit::SpaceRef::Personal {
                account: "synthetic-bob".into(),
            },
        )
        .unwrap();
        let mut content =
            cad::doc::new_part("fan", "Fan bracket", cad::doc::DEFAULT_MATERIAL, None);
        content["features"] =
            json!([{"id":"body","op":"solid.box","params":{"length":40,"width":40,"height":3}}]);
        let draft = api
            .initial(
                &store,
                "fan",
                cad::doc::PART,
                content.clone(),
                "Fan bracket",
            )
            .await
            .unwrap();
        let first = store
            .create("fan", "Fan bracket", "alice", draft, now())
            .await
            .unwrap();
        api.saved(&store, &first).await.unwrap();
        assert_eq!(first.content_sha256, lab_kit::digest(&content));
        let change = Change::Intent {
            intent: json!({"intent":"pull_face","part":"fan","face":"body:+z","delta_mm":5.0}),
        };
        let expected = cad::lab::changed_content(&content, &change).unwrap();
        let draft = api
            .change(&store, &first, &change, "Five millimeters thicker")
            .await
            .unwrap();
        let second = store
            .save("fan", "alice", draft.clone(), now())
            .await
            .unwrap();
        api.saved(&store, &second).await.unwrap();
        assert_eq!(second.number, 2);
        assert_eq!(second.content, expected);
        assert!(matches!(
            store.save("fan", "alice", draft, now()).await,
            Err(lab_store::Error::Conflict)
        ));
        assert!(matches!(
            foreign.document("fan").await,
            Err(lab_store::Error::NotFound)
        ));
        let artifacts = store.version_artifacts(&second.reference()).await.unwrap();
        assert!(artifacts.iter().all(|a| a.no_training));
        let card = api.card(&second, &[], &artifacts);
        assert_eq!(card.view, lab_kit::View::Model3d);
        assert!(store.jobs("fan", 0, 100).await.unwrap().items.is_empty());
        let spec = api
            .jobs(
                &second,
                &Want::Job {
                    job_type: "export".into(),
                    params: json!({"formats":["step"]}),
                },
            )
            .remove(0);
        let job = store
            .create_job(spec.clone(), &api.manifest(), now())
            .await
            .unwrap();
        let authority = lab_jobs::authority::Authority::new(backend.clone());
        let issued = authority
            .enroll(
                "synthetic-alice",
                lab_kit::SpaceRef::Personal {
                    account: "synthetic-alice".into(),
                },
                api.server_capabilities().unwrap(),
                now(),
                60000,
            )
            .await
            .unwrap();
        api.run_server_job(
            store.clone(),
            job.clone(),
            lab_api::ServerExecution {
                worker: issued.worker.clone(),
                authority: authority.clone(),
            },
        )
        .await;
        let usage = authority
            .usage(&issued.worker.space, &job.id)
            .await
            .unwrap();
        assert_eq!(usage.len(), 1);
        assert_eq!(usage[0].placement, lab_jobs::Placement::Server);
        assert_eq!(usage[0].charge, None);
        assert_eq!(store.job(&job.id).await.unwrap().status, JobStatus::Done);
        let step = store
            .named_artifact(&second.reference(), "out/fan-v2.step")
            .await
            .unwrap();
        assert!(
            store
                .artifact_bytes(&step.id, None)
                .await
                .unwrap()
                .starts_with(b"ISO-10303-21;")
        );
        assert_eq!(
            store
                .create_job(spec, &api.manifest(), now())
                .await
                .unwrap()
                .id,
            job.id
        );
        assert!(api.prepared.lock().unwrap().is_empty());
        assert_eq!(
            std::fs::read_dir(engine.cache.join("api")).unwrap().count(),
            0
        );
        store.delete("fan", now()).await.unwrap();
        assert!(matches!(
            store.artifact(&step.id).await,
            Err(lab_store::Error::NotFound)
        ));
    }
}
