//! Existing CAD pages and engine calls over the shared version and artifact store.
//! Older documents stay in their existing layout until their reviewed copy runs.
use super::*;
use lab_kit::{ArtifactRole, AssetClass, Change, Reference, SpaceRef};
use lab_store::{ArtifactInput, CheckedDraft, LabStore};

struct ChatLegacy(Store);
#[async_trait::async_trait]
impl lab_store::cad_legacy::LegacySource for ChatLegacy {
    async fn read(&self, key: &str) -> lab_store::Result<Vec<u8>> {
        self.0
            .read_key(key)
            .await
            .map_err(lab_store::Error::storage)?
            .map(|(bytes, _)| bytes)
            .ok_or(lab_store::Error::NotFound)
    }
    async fn keys(&self, prefix: &str) -> lab_store::Result<Vec<String>> {
        let prefix = prefix.trim_end_matches('/');
        let index_key = format!("{prefix}/index.json");
        let index: lab_store::cad_legacy::LegacyIndex =
            serde_json::from_slice(&self.read(&index_key).await?)?;
        let mut keys = vec![index_key];
        for (id, doc) in index.docs {
            if !valid_doc(&id) || doc.versions > MAX_VERSIONS {
                return Err(lab_store::Error::Corrupt);
            }
            for number in 1..=doc.versions {
                for folder in [
                    format!("{prefix}/{id}/v{number}"),
                    format!("{prefix}/{id}/v{number}/out"),
                ] {
                    let remaining = 100_000usize
                        .checked_sub(keys.len())
                        .filter(|n| *n > 0)
                        .ok_or_else(|| {
                            lab_store::Error::Invalid("This CAD space has too many files.".into())
                        })?;
                    keys.extend(
                        self.0
                            .folder_keys_limit(&folder, remaining)
                            .await
                            .map_err(lab_store::Error::storage)?,
                    );
                }
            }
        }
        keys.sort();
        Ok(keys)
    }
}
pub(super) fn legacy(
    chat: &Store,
    scope: &Scope,
) -> Result<lab_store::cad_legacy::CadLegacy, String> {
    let source = Arc::new(ChatLegacy(chat.clone()));
    let key = scope
        .key("index.json")
        .map_err(|_| "This design's address couldn't be read.")?;
    let owner = key
        .split('/')
        .next()
        .ok_or("This design's address couldn't be read.")?;
    match &scope.project {
        Some(project) => lab_store::cad_legacy::CadLegacy::project(source, owner, project),
        None => lab_store::cad_legacy::CadLegacy::personal(source, owner),
    }
    .map_err(error)
}
pub(super) async fn legacy_index(chat: &Store, scope: &Scope) -> Result<Index, String> {
    let index = legacy(chat, scope)?.index().await.map_err(error)?;
    serde_json::from_value(
        serde_json::to_value(index).map_err(|_| "Your designs couldn't be read.")?,
    )
    .map_err(|_| "Your designs couldn't be read.".into())
}

fn error(_: lab_store::Error) -> String {
    "We couldn't read or save this design. Try again.".into()
}
pub(super) async fn store(chat: &Store, scope: &Scope) -> Result<Option<lab_store::Store>, String> {
    let Some(backend) = chat.labs() else {
        return Ok(None);
    };
    match &scope.project {
        Some(project) => lab_store::Store::new(
            backend,
            &SpaceRef::Project {
                project: project.clone(),
            },
        )
        .map(Some)
        .map_err(error),
        None => lab_store::Store::from_alias(backend, &scope.home)
            .await
            .map_err(error)?
            .map(Some)
            .ok_or_else(|| "We couldn't open your designs. Try again.".into()),
    }
}

pub(super) async fn reference(
    chat: &Store,
    scope: &Scope,
    doc: &str,
    number: u64,
) -> Result<Option<lab_api::ResourceReference>, String> {
    let Some(store) = store(chat, scope).await? else {
        return Ok(None);
    };
    if !store.knows_document(doc).await.map_err(error)? {
        return Ok(None);
    }
    let meta = store.document(doc).await.map_err(error)?;
    if meta.kind != "cad" {
        return Err("Choose a CAD design.".into());
    }
    let version = store.version(doc, number).await.map_err(error)?;
    Ok(Some(lab_api::ResourceReference {
        space: scope
            .project
            .as_ref()
            .map_or_else(|| "me".into(), |id| format!("project:{id}")),
        version: version.reference(),
    }))
}

/// Operator-only copy after the source report's checksum has been approved.
/// The old objects remain unchanged, including their original JSON bytes.
pub(super) async fn migrate(chat: &Store, scope: &Scope, expected: &str) -> Result<Value, String> {
    let target = store(chat, scope)
        .await?
        .ok_or("Configure shared CAD storage before copying designs.")?;
    let source = legacy(chat, scope)?;
    let report = source.dry_run().await.map_err(error)?;
    if report.checksum().map_err(error)? != expected {
        return Err("The designs changed after review. Review them again before copying.".into());
    }
    source
        .copy(
            chat.labs()
                .ok_or("Configure shared CAD storage before copying designs.")?
                .as_ref(),
            expected,
        )
        .await
        .map_err(error)?;
    let index = source.index().await.map_err(error)?;
    for (id, entry) in &index.docs {
        for number in 1..=entry.versions {
            let (content, record) = source.version(id, number).await.map_err(error)?;
            let summary = record["summary"].as_str().unwrap_or("Saved design");
            let author = record["author"].as_str().unwrap_or(&scope.actor);
            let created = record["created"]
                .as_u64()
                .unwrap_or_default()
                .saturating_mul(1000);
            let measurements = cad::lab::measurements(&content, record["report"].clone());
            let version = match target.version(id, number).await {
                Ok(version) => {
                    if version.content_sha256 != lab_kit::digest(&content)
                        || version.summary != summary
                        || version.author != author
                        || version.created_unix_ms != created
                        || version.report.facts != record["report"]
                    {
                        return Err(
                            "A design with this name already exists in Labs. Nothing was replaced."
                                .into(),
                        );
                    }
                    version
                }
                Err(lab_store::Error::NotFound) => {
                    if number == 1 {
                        let schema = content["kind"]
                            .as_str()
                            .ok_or("Choose a part or an assembly.")?;
                        let draft = CheckedDraft::initial::<cad::lab::Cad>(
                            schema,
                            content.clone(),
                            summary,
                            &measurements,
                        )
                        .map_err(error)?;
                        target
                            .create(id, &entry.title, author, draft, created)
                            .await
                            .map_err(error)?
                    } else {
                        let base = target.version(id, number - 1).await.map_err(error)?;
                        let change = Change::Intent {
                            intent: json!({"intent":"replace","content":content}),
                        };
                        let draft = CheckedDraft::change::<cad::lab::Cad>(
                            &base,
                            &change,
                            summary,
                            &measurements,
                        )
                        .map_err(error)?;
                        target
                            .save(id, author, draft, created)
                            .await
                            .map_err(error)?
                    }
                }
                Err(problem) => return Err(error(problem)),
            };
            let prefix = format!(
                "{}/",
                scope
                    .key(&format!("{id}/v{number}"))
                    .map_err(|_| "This design's address couldn't be read.")?
            );
            for file in report
                .files
                .iter()
                .filter(|file| file.key.starts_with(&prefix))
            {
                let name = file
                    .key
                    .strip_prefix(&prefix)
                    .ok_or("This design's address couldn't be read.")?;
                if matches!(name, "doc.json" | "version.json") {
                    continue;
                }
                let bytes = chat
                    .read_key(&file.key)
                    .await
                    .map_err(|_| "A design file couldn't be read.")?
                    .ok_or("A design file is missing. Review the designs again.")?
                    .0;
                if format!("{:x}", Sha256::digest(&bytes)) != file.sha256
                    || bytes.len() as u64 != file.bytes
                {
                    return Err(
                        "A design file changed after review. Review the designs again.".into(),
                    );
                }
                attach(&target, &version.reference(), name, bytes, created).await?;
            }
        }
    }
    if source.dry_run().await.map_err(error)? != report {
        return Err(
            "The designs changed during the copy. Review them again before continuing.".into(),
        );
    }
    Ok(
        json!({"documents":report.documents,"versions":report.versions,"files":report.files.len(),"bytes":report.total_bytes,"review_sha256":expected,"source_unchanged":true}),
    )
}

pub(super) async fn overlay_index(
    chat: &Store,
    scope: &Scope,
    index: &mut Index,
) -> Result<(), String> {
    let Some(store) = store(chat, scope).await? else {
        return Ok(());
    };
    for id in store.deleted_documents("cad").await.map_err(error)? {
        index.docs.remove(&id);
    }
    let mut after = 0;
    loop {
        let page = store
            .documents(Some("cad"), after, 100)
            .await
            .map_err(error)?;
        for doc in page.items {
            let mut history = vec![];
            let mut after = 0;
            loop {
                let page = store.versions(&doc.id, after, 100).await.map_err(error)?;
                history.extend(page.items.iter().map(|v| Version {
                    n: v.number,
                    summary: v.summary.clone(),
                    author: v.author.clone(),
                    created: v.created_unix_ms / 1000,
                    mass_kg: v.report.facts["mass"]["mass_kg"].as_f64(),
                }));
                match page.next {
                    Some(next) => after = next,
                    None => break,
                }
            }
            index.docs.insert(
                doc.id,
                Entry {
                    title: doc.title,
                    kind: doc.schema,
                    versions: doc.latest,
                    history,
                },
            );
        }
        match page.next {
            Some(next) => after = next,
            None => break,
        }
    }
    Ok(())
}

pub(super) async fn persist(
    chat: &Store,
    scope: &Scope,
    dir: &Path,
    result: &Value,
) -> Result<bool, String> {
    let Some(store) = store(chat, scope).await? else {
        return Ok(false);
    };
    let doc = result["doc"]
        .as_str()
        .filter(|d| valid_doc(d))
        .ok_or("The engine saved no document.")?;
    let known = store.knows_document(doc).await.map_err(error)?;
    // Copying a preexisting document requires its reviewed migration report.
    if !known
        && read_legacy_index(chat, scope)
            .await?
            .0
            .docs
            .contains_key(doc)
    {
        return Ok(false);
    }
    let n = result["version"]
        .as_u64()
        .filter(|n| *n > 0)
        .ok_or("The engine saved no version.")?;
    let at = dir.join(doc).join(format!("v{n}"));
    let bytes = tokio::fs::read(at.join("doc.json"))
        .await
        .map_err(|_| "The new design couldn't be read.")?;
    if bytes.len() > MAX_DOC_BYTES {
        return Err("This design is too large to save.".into());
    }
    let content: Value =
        serde_json::from_slice(&bytes).map_err(|_| "The new design couldn't be read.")?;
    let record: Value = serde_json::from_slice(
        &tokio::fs::read(at.join("version.json"))
            .await
            .map_err(|_| "The new version couldn't be read.")?,
    )
    .map_err(|_| "The new version couldn't be read.")?;
    let summary = record["summary"].as_str().unwrap_or("Updated design");
    let author = record["author"]
        .as_str()
        .filter(|a| !a.is_empty())
        .unwrap_or(&scope.actor);
    let now = record["created"]
        .as_u64()
        .unwrap_or_default()
        .saturating_mul(1000);
    let m = cad::lab::measurements(&content, record["report"].clone());
    let version = if !known {
        if n != 1 {
            return Err("Reload this design before saving it.".into());
        }
        let schema = content["kind"]
            .as_str()
            .ok_or("Choose a part or an assembly.")?;
        let draft = CheckedDraft::initial::<cad::lab::Cad>(schema, content.clone(), summary, &m)
            .map_err(error)?;
        store
            .create(
                doc,
                content["title"].as_str().unwrap_or(doc),
                author,
                draft,
                now,
            )
            .await
            .map_err(error)?
    } else {
        let base = store.version(doc, n - 1).await.map_err(error)?;
        let change = Change::Intent {
            intent: json!({"intent":"replace","content":content}),
        };
        let draft =
            CheckedDraft::change::<cad::lab::Cad>(&base, &change, summary, &m).map_err(error)?;
        store.save(doc, author, draft, now).await.map_err(error)?
    };
    if version.number != n {
        return Err("Reload this design before saving it.".into());
    }
    for name in FILES {
        if let Ok(bytes) = tokio::fs::read(at.join(name)).await {
            attach(&store, &version.reference(), name, bytes, now).await?;
        }
    }
    imports::persist(chat, scope, doc, n, &at).await?;
    Ok(true)
}

pub(super) async fn attach(
    store: &lab_store::Store,
    version: &Reference,
    name: &str,
    bytes: Vec<u8>,
    now: u64,
) -> Result<(), String> {
    if bytes.len() > MAX_FILE_BYTES {
        return Err("This design file is too large.".into());
    }
    let role = match name {
        "hero.png" => ArtifactRole::Poster,
        "model.glb" => ArtifactRole::Build,
        name if name.starts_with("out/") => ArtifactRole::Export,
        _ => ArtifactRole::Source,
    };
    let artifact = store
        .put_artifact(
            ArtifactInput {
                version: version.clone(),
                role,
                media_type: content_type(name).into(),
                // A design can contain an attached model. Preserve the stricter
                // policy for every file derived from it.
                class: AssetClass::NoTraining,
                no_training: true,
            },
            bytes,
            MAX_FILE_BYTES as u64,
            now,
        )
        .await
        .map_err(error)?;
    store
        .name_artifact(version, name, &artifact.id)
        .await
        .map_err(error)
}

pub(super) async fn read_file(
    chat: &Store,
    scope: &Scope,
    doc: &str,
    number: u64,
    name: &str,
) -> Result<Option<Vec<u8>>, String> {
    let Some(store) = store(chat, scope).await? else {
        return Ok(None);
    };
    if !store.knows_document(doc).await.map_err(error)? {
        return Ok(None);
    }
    let version = store.version(doc, number).await.map_err(error)?;
    if name == "doc.json" {
        return serde_json::to_vec(&version.content)
            .map(Some)
            .map_err(|_| "This design couldn't be read.".into());
    }
    if name == "version.json" {
        let old_digest = Sha256::digest(version.content.to_string().as_bytes());
        let old_digest = old_digest
            .iter()
            .take(8)
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        return serde_json::to_vec(&json!({"n":version.number,"summary":version.summary,
            "author":version.author,"created":version.created_unix_ms / 1000,"base":version.parent,
            "digest":old_digest,"report":version.report.facts}))
        .map(Some)
        .map_err(|_| "This version couldn't be read.".into());
    }
    let name = if !FILES.contains(&name) && !name.starts_with("out/") {
        format!("out/{name}")
    } else {
        name.into()
    };
    let artifact = store
        .named_artifact(&version.reference(), &name)
        .await
        .map_err(error)?;
    store
        .artifact_bytes(&artifact.id, None)
        .await
        .map(Some)
        .map_err(error)
}

pub(super) async fn delete(chat: &Store, scope: &Scope, doc: &str) -> Result<(), String> {
    let Some(store) = store(chat, scope).await? else {
        return Ok(());
    };
    if store.knows_document(doc).await.map_err(error)? {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        store.delete(doc, now).await.map_err(error)?;
    }
    Ok(())
}

pub(super) async fn output(
    chat: &Store,
    scope: &Scope,
    doc: &str,
    number: u64,
    name: &str,
    bytes: &[u8],
) -> Result<bool, String> {
    let Some(store) = store(chat, scope).await? else {
        return Ok(false);
    };
    if !store.knows_document(doc).await.map_err(error)? {
        return Ok(false);
    }
    let version = store.version(doc, number).await.map_err(error)?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    attach(
        &store,
        &version.reference(),
        &format!("out/{name}"),
        bytes.to_vec(),
        now,
    )
    .await?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn legacy_review_includes_version_files_and_exports_without_changing_them() {
        let root = tempfile::tempdir().unwrap();
        let chat = Store::local(root.path().join("chats"));
        let scope = Scope::personal(&crate::chat_store::account_owner("synthetic-owner")).unwrap();
        let files = [
            ("index.json", lab_kit::canonical_json(&json!({"schema":INDEX_SCHEMA,"docs":{"plate":{"title":"Plate","kind":"part.v1","versions":1,"history":[]}}})).into_bytes()),
            ("plate/v1/doc.json", lab_kit::canonical_json(&json!({"kind":"part.v1","id":"plate"})).into_bytes()),
            ("plate/v1/version.json", lab_kit::canonical_json(&json!({"n":1,"digest":"synthetic"})).into_bytes()),
            ("plate/v1/out/plate-v1.step", b"Synthetic STEP fixture".to_vec()),
        ];
        for (name, bytes) in &files {
            chat.write_key(&scope.key(name).unwrap(), bytes.clone(), None)
                .await
                .unwrap();
        }
        let source = legacy(&chat, &scope).unwrap();
        let report = source.dry_run().await.unwrap();
        // The Python operator review produces the same checksum for this fixture.
        assert_eq!(
            report.checksum().unwrap(),
            "560ba44c2485030f3e57d8931f7af704d4f44a221cf035a87b98c7403e66cab0"
        );
        assert_eq!(
            (report.documents, report.versions, report.files.len()),
            (1, 1, 4)
        );
        let target = lab_store::backend::FsBackend::open(root.path().join("copy")).unwrap();
        assert_eq!(
            source
                .copy(&target, &report.checksum().unwrap())
                .await
                .unwrap(),
            report
        );
        for (name, bytes) in files {
            assert_eq!(
                chat.read_key(&scope.key(name).unwrap())
                    .await
                    .unwrap()
                    .unwrap()
                    .0,
                bytes
            );
        }
        assert_eq!(source.dry_run().await.unwrap(), report);
    }

    async fn fixture(dir: &Path, n: u64, height: f64) -> (Value, Value) {
        let mut doc = cad::doc::new_part(
            "plate",
            "Fan mounting plate",
            cad::doc::DEFAULT_MATERIAL,
            None,
        );
        doc["features"] = json!([{"id":"body","op":"solid.box","params":{"length":40,"width":40,"height":height}}]);
        let report = cad::evaluate_with(&doc, &dir.join(".parts"))
            .unwrap()
            .report;
        let record = json!({"n":n,"summary":"Mounting plate","author":"alice","created":1792194000u64,"base":if n==1 {Value::Null} else {json!(n-1)},
            "report":report});
        let at = dir.join("plate").join(format!("v{n}"));
        tokio::fs::create_dir_all(&at).await.unwrap();
        tokio::fs::write(at.join("doc.json"), serde_json::to_vec(&doc).unwrap())
            .await
            .unwrap();
        tokio::fs::write(
            at.join("version.json"),
            serde_json::to_vec(&record).unwrap(),
        )
        .await
        .unwrap();
        tokio::fs::write(at.join("model.glb"), b"Synthetic model bytes")
            .await
            .unwrap();
        (doc, record)
    }

    #[tokio::test]
    async fn reviewed_history_copy_keeps_numbers_digests_and_original_files_and_can_resume() {
        let root = tempfile::tempdir().unwrap();
        let backend =
            Arc::new(lab_store::backend::FsBackend::open(root.path().join("labs")).unwrap());
        let chat = Store::local(root.path().join("chats")).with_labs(Some(backend));
        let scope =
            Scope::personal(&crate::chat_store::account_owner("synthetic-account")).unwrap();
        let target = lab_store::Store::new(
            chat.labs().unwrap(),
            &SpaceRef::Personal {
                account: "synthetic-account".into(),
            },
        )
        .unwrap();
        target.bind_alias(&scope.home).await.unwrap();
        let index = json!({"schema":INDEX_SCHEMA,"docs":{"plate":{"title":"Fan mounting plate","kind":"part.v1","versions":2,"history":[]}}});
        chat.write_key(
            &scope.key("index.json").unwrap(),
            serde_json::to_vec(&index).unwrap(),
            None,
        )
        .await
        .unwrap();
        let working = root.path().join("working");
        let mut originals = vec![];
        for (number, height) in [(1, 3.0), (2, 8.0)] {
            let (content, record) = fixture(&working, number, height).await;
            for (name, bytes) in [
                ("doc.json", serde_json::to_vec(&content).unwrap()),
                ("version.json", serde_json::to_vec(&record).unwrap()),
                ("model.glb", b"Synthetic glTF".to_vec()),
            ] {
                let key = scope.key(&format!("plate/v{number}/{name}")).unwrap();
                chat.write_key(&key, bytes.clone(), None).await.unwrap();
                originals.push((key, bytes));
            }
        }
        let report = legacy(&chat, &scope).unwrap().dry_run().await.unwrap();
        assert!(migrate(&chat, &scope, &"0".repeat(64)).await.is_err());
        assert!(
            target
                .documents(Some("cad"), 0, 100)
                .await
                .unwrap()
                .items
                .is_empty()
        );
        for _ in 0..2 {
            assert_eq!(
                migrate(&chat, &scope, &report.checksum().unwrap())
                    .await
                    .unwrap()["versions"],
                2
            );
        }
        for (number, height) in [(1, 3.0), (2, 8.0)] {
            let version = target.version("plate", number).await.unwrap();
            assert_eq!(version.content["features"][0]["params"]["height"], height);
            assert_eq!(version.content_sha256, lab_kit::digest(&version.content));
            assert_eq!(
                read_file(&chat, &scope, "plate", number, "model.glb")
                    .await
                    .unwrap()
                    .unwrap(),
                b"Synthetic glTF"
            );
        }
        for (key, bytes) in originals {
            assert_eq!(chat.read_key(&key).await.unwrap().unwrap().0, bytes);
        }
        assert_eq!(
            legacy(&chat, &scope).unwrap().dry_run().await.unwrap(),
            report
        );
    }

    #[tokio::test]
    async fn shared_versions_files_exports_and_deletion_keep_the_existing_page_contract() {
        let root = tempfile::tempdir().unwrap();
        let backend =
            Arc::new(lab_store::backend::FsBackend::open(root.path().join("private")).unwrap());
        let chat = Store::local(root.path().join("chats")).with_labs(Some(backend));
        let scope = Scope::personal(&crate::chat_store::account_owner("alice")).unwrap();
        lab_store::Store::new(
            chat.labs().unwrap(),
            &SpaceRef::Personal {
                account: "alice".into(),
            },
        )
        .unwrap()
        .bind_alias(&scope.home)
        .await
        .unwrap();
        let working = root.path().join("working");
        let (doc, record) = fixture(&working, 1, 3.0).await;
        assert!(
            persist(&chat, &scope, &working, &json!({"doc":"plate","version":1}))
                .await
                .unwrap()
        );
        let shared = store(&chat, &scope).await.unwrap().unwrap();
        assert_eq!(
            shared.version("plate", 1).await.unwrap().content_sha256,
            lab_kit::digest(&doc)
        );
        assert!(
            read_legacy_index(&chat, &scope)
                .await
                .unwrap()
                .0
                .docs
                .is_empty()
        );
        let index = load_index(&chat, &scope).await.unwrap();
        assert_eq!(index.docs["plate"].versions, 1);
        let old: Value = serde_json::from_slice(
            &super::super::read_file(&chat, &scope, "plate", 1, "version.json")
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(old["report"], record["report"]);
        assert_eq!(old["n"], 1);
        assert_eq!(
            super::super::read_file(&chat, &scope, "plate", 1, "model.glb")
                .await
                .unwrap(),
            b"Synthetic model bytes"
        );
        assert!(
            super::super::read_file(
                &chat,
                &Scope::personal(&crate::chat_store::account_owner("bob")).unwrap(),
                "plate",
                1,
                "model.glb"
            )
            .await
            .is_none()
        );
        fixture(&working, 2, 8.0).await;
        persist(&chat, &scope, &working, &json!({"doc":"plate","version":2}))
            .await
            .unwrap();
        assert_eq!(
            load_index(&chat, &scope).await.unwrap().docs["plate"]
                .history
                .len(),
            2
        );
        assert!(
            output(
                &chat,
                &scope,
                "plate",
                2,
                "plate-v2.step",
                b"Synthetic STEP bytes"
            )
            .await
            .unwrap()
        );
        assert_eq!(
            super::super::read_file(&chat, &scope, "plate", 2, "plate-v2.step")
                .await
                .unwrap(),
            b"Synthetic STEP bytes"
        );
        assert!(
            super::super::read_file(&chat, &scope, "plate", 1, "plate-v2.step")
                .await
                .is_none()
        );
        assert!(
            shared
                .name_artifact(
                    &Reference {
                        document: "plate".into(),
                        version: 2
                    },
                    "../escape",
                    "none"
                )
                .await
                .is_err()
        );
        delete(&chat, &scope, "plate").await.unwrap();
        assert!(load_index(&chat, &scope).await.unwrap().docs.is_empty());
        assert!(
            super::super::read_file(&chat, &scope, "plate", 1, "doc.json")
                .await
                .is_none()
        );
        assert!(shared.knows_document("plate").await.unwrap());
    }
}
