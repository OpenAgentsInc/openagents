//! Atomic local journals and immutable, verified generation directories.
use super::*;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};
const DOCUMENT_LIMIT: usize = 2 * 1024 * 1024;
const JOURNAL_LIMIT: usize = 16 * 1024 * 1024;
const HISTORY: usize = 32;
pub(crate) fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn io(path: &Path, error: impl std::fmt::Display) -> Diagnostic {
    Diagnostic::at(path.display().to_string(), "$", error.to_string())
}
pub(crate) fn read(path: &Path, limit: usize) -> Result<Vec<u8>> {
    let metadata = std::fs::symlink_metadata(path).map_err(|e| io(path, e))?;
    if !metadata.is_file() || metadata.len() > limit as u64 {
        return Err(io(
            path,
            "Expected a bounded regular file; symbolic links are refused",
        ));
    }
    let file = File::open(path).map_err(|e| io(path, e))?;
    let mut bytes = vec![];
    file.take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| io(path, e))?;
    if bytes.len() > limit {
        return Err(io(path, "Input grew beyond its byte budget"));
    }
    Ok(bytes)
}
fn bytes(value: &impl Serialize, limit: usize) -> Result<Vec<u8>> {
    let value = serde_json::to_vec_pretty(value)
        .map_err(|e| Diagnostic::at("document.json", "$", e.to_string()))?;
    if value.len() > limit {
        return Err(Diagnostic::at(
            "document.json",
            "$",
            "Serialized document or undo journal exceeds its byte budget",
        ));
    }
    Ok(value)
}
fn sync_dir(path: &Path) -> Result<()> {
    File::open(path)
        .and_then(|f| f.sync_all())
        .map_err(|e| io(path, e))
}
fn create(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path).map_err(|e| io(path, e))?;
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|e| io(path, e))
}
pub(crate) fn write_preview(path: &Path, data: &[u8]) -> Result<()> {
    ancestors(path)?;
    atomic(path, data)
}
fn atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let temp = path.with_extension("pending");
    if temp.exists() {
        std::fs::remove_file(&temp).map_err(|e| io(&temp, e))?;
    }
    create(&temp, bytes)?;
    std::fs::rename(&temp, path).map_err(|e| io(path, e))?;
    sync_dir(
        path.parent()
            .ok_or_else(|| io(path, "Missing destination parent"))?,
    )
}
fn ancestors(path: &Path) -> Result<()> {
    for ancestor in path.ancestors() {
        if ancestor.as_os_str().is_empty() {
            continue;
        }
        if ancestor
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            return Err(io(path, "Parent traversal is refused"));
        }
        match std::fs::symlink_metadata(ancestor) {
            Ok(m) if m.file_type().is_symlink() => {
                return Err(io(ancestor, "Symbolic links are refused"));
            }
            Ok(_) => (),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
            Err(e) => return Err(io(ancestor, e)),
        }
    }
    Ok(())
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Build {
    pub schema: String,
    pub generation: String,
    pub content: [u8; 32],
    pub path: PathBuf,
    pub reused: bool,
    pub asset_files_reused: usize,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Seal {
    schema: String,
    generation: String,
    asset_digest: String,
    files: BTreeMap<String, String>,
    content: [u8; 32],
}
/// One exclusively held workspace. A dropped process releases the OS lock.
pub struct Workspace {
    root: PathBuf,
    _lock: File,
    pub(crate) journal: Journal,
    pub(crate) base: Pack,
}
impl Workspace {
    /// Copies an admitted compiled asset bundle once. It never edits the input.
    pub fn init(input: &Path, destination: &Path, zone: String) -> Result<Self> {
        ancestors(input)?;
        ancestors(destination)?;
        if destination.exists() {
            return Err(io(destination, "Authoring destination must not exist"));
        }
        let pack_bytes = read(&input.join("pack.json"), 128 * 1024 * 1024)?;
        let pack: Pack = parse("pack.json", &pack_bytes, 128 * 1024 * 1024)?;
        let scene: Scene = parse(
            "scene.json",
            &read(&input.join("scene.json"), 1024 * 1024)?,
            1024 * 1024,
        )?;
        let social: Option<verse_world::play::social::Profile> =
            if input.join("profile.json").exists() {
                parse(
                    "profile.json",
                    &read(&input.join("profile.json"), DOCUMENT_LIMIT)?,
                    DOCUMENT_LIMIT,
                )?
            } else {
                None
            };
        let doc = Document::from_scene(zone, scene.clone(), pack.placements.clone(), social);
        admit(&doc, &pack)?;
        checked(
            "assets",
            crate::remote_content::identity(&pack, &scene, input),
        )?;
        std::fs::create_dir_all(destination).map_err(|e| io(destination, e))?;
        let base_dir = destination.join("assets");
        std::fs::create_dir(&base_dir).map_err(|e| io(&base_dir, e))?;
        create(&base_dir.join("pack.json"), &pack_bytes)?;
        let mut total = 0usize;
        for texture in &pack.textures {
            let source = read(&input.join(&texture.file), 64 * 1024 * 1024)?;
            total += source.len();
            if total > 512 * 1024 * 1024 || hash(&source) != texture.sha256 {
                return Err(io(
                    &input.join(&texture.file),
                    "Texture changed during source snapshot",
                ));
            }
            create(&base_dir.join(&texture.file), &source)?;
        }
        sync_dir(&base_dir)?;
        let journal = Journal {
            schema: "verse.author.journal.v1".into(),
            asset_digest: hash(&pack_bytes),
            revision: 1,
            document: doc,
            undo: vec![],
            redo: vec![],
            last_label: "Initialize workspace".into(),
        };
        create(
            &destination.join("journal.json"),
            &bytes(&journal, JOURNAL_LIMIT)?,
        )?;
        sync_dir(destination)?;
        Self::open(destination)
    }
    pub fn open(root: &Path) -> Result<Self> {
        ancestors(root)?;
        let root = root.canonicalize().map_err(|e| io(root, e))?;
        let lock_path = root.join("author.lock");
        if lock_path.exists()
            && !std::fs::symlink_metadata(&lock_path)
                .map_err(|e| io(&lock_path, e))?
                .is_file()
        {
            return Err(io(&lock_path, "Lock must be a regular file"));
        }
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let lock = options.open(&lock_path).map_err(|e| io(&lock_path, e))?;
        lock.try_lock()
            .map_err(|_| io(&lock_path, "Another author holds this workspace"))?;
        let journal: Journal = parse(
            "journal.json",
            &read(&root.join("journal.json"), JOURNAL_LIMIT)?,
            JOURNAL_LIMIT,
        )?;
        if journal.schema != "verse.author.journal.v1"
            || journal.revision == 0
            || journal.undo.len() > HISTORY
            || journal.redo.len() > HISTORY
        {
            return Err(io(
                &root.join("journal.json"),
                "Unsupported journal schema, revision, or history budget",
            ));
        }
        ancestors(&root.join("assets"))?;
        let base_bytes = read(&root.join("assets/pack.json"), 128 * 1024 * 1024)?;
        if hash(&base_bytes) != journal.asset_digest {
            return Err(io(
                &root.join("assets/pack.json"),
                "Source asset snapshot changed; initialize a new workspace",
            ));
        }
        let base = parse("assets/pack.json", &base_bytes, 128 * 1024 * 1024)?;
        let workspace = Self {
            root,
            _lock: lock,
            journal,
            base,
        };
        workspace.validate_current()?;
        Ok(workspace)
    }
    pub fn document(&self) -> &Document {
        &self.journal.document
    }
    pub fn revision(&self) -> u64 {
        self.journal.revision
    }
    pub fn journal(&self) -> &Journal {
        &self.journal
    }
    pub fn validate_current(&self) -> Result<()> {
        bytes(self.document(), DOCUMENT_LIMIT)?;
        let (pack, scene, _) = admit(self.document(), &self.base)?;
        checked(
            "assets",
            crate::remote_content::identity(&pack, &scene, &self.root.join("assets")),
        )?;
        Ok(())
    }
    fn save(&mut self, candidate: Journal) -> Result<()> {
        bytes(&candidate.document, DOCUMENT_LIMIT)?;
        admit(&candidate.document, &self.base)?;
        let encoded = bytes(&candidate, JOURNAL_LIMIT)?;
        atomic(&self.root.join("journal.json"), &encoded)?;
        self.journal = candidate;
        Ok(())
    }
    pub fn transact(&mut self, tx: &Transaction) -> Result<u64> {
        if tx.expected_revision != self.revision() {
            return Err(Diagnostic::at(
                "transaction.json",
                "expected_revision",
                format!(
                    "Reload revision {}; this transaction is stale",
                    self.revision()
                ),
            ));
        }
        if tx.edits.is_empty()
            || tx.edits.len() > 64
            || tx.label.trim().is_empty()
            || tx.label.len() > 128
            || tx.label.chars().any(char::is_control)
        {
            return Err(Diagnostic::at(
                "transaction.json",
                "edits",
                "Use 1..64 edits and a printable 1..128 byte label",
            ));
        }
        let mut next = self.journal.clone();
        for (index, edit) in tx.edits.iter().enumerate() {
            edit.apply(&mut next.document)
                .map_err(|e| Diagnostic::at("transaction.json", format!("edits[{index}]"), e))?;
        }
        next.revision = self
            .revision()
            .checked_add(1)
            .ok_or_else(|| Diagnostic::at("journal.json", "revision", "Revision exhausted"))?;
        next.undo.push(self.document().clone());
        if next.undo.len() > HISTORY {
            next.undo.remove(0);
        }
        next.redo.clear();
        next.last_label = tx.label.clone();
        self.save(next)?;
        Ok(self.revision())
    }
    pub fn undo(&mut self, expected: u64) -> Result<u64> {
        self.history(expected, false)
    }
    pub fn redo(&mut self, expected: u64) -> Result<u64> {
        self.history(expected, true)
    }
    fn history(&mut self, expected: u64, redo: bool) -> Result<u64> {
        if expected != self.revision() {
            return Err(Diagnostic::at(
                "command",
                "expected_revision",
                "Undo or redo revision is stale",
            ));
        }
        let mut next = self.journal.clone();
        let target = if redo {
            next.redo.pop()
        } else {
            next.undo.pop()
        }
        .ok_or_else(|| Diagnostic::at("command", "history", "No matching history entry"))?;
        if redo {
            next.undo.push(next.document);
        } else {
            next.redo.push(next.document);
        }
        next.document = target;
        next.revision = self
            .revision()
            .checked_add(1)
            .ok_or_else(|| Diagnostic::at("journal.json", "revision", "Revision exhausted"))?;
        next.last_label = if redo { "Redo" } else { "Undo" }.into();
        self.save(next)?;
        Ok(self.revision())
    }
    pub fn inspect(&self) -> Result<serde_json::Value> {
        let (pack, _, _) = admit(self.document(), &self.base)?;
        let models: Vec<_> = pack.models.iter().map(|(key, model)| serde_json::json!({
            "key": key, "source": model.source, "source_sha256": model.source_sha256,
            "vertices": model.surfaces.iter().map(|s| s.vertices.len()).sum::<usize>(),
            "surfaces": model.surfaces.iter().map(|s| &s.material).collect::<Vec<_>>(),
            "clips": model.clips.iter().map(|c| (c.id, c.duration)).collect::<Vec<_>>(),
            "states": model.states, "sockets": model.attachments.iter().map(|a| a.id).collect::<Vec<_>>()
        })).collect();
        Ok(
            serde_json::json!({"schema":"verse.author.inspection.v1", "revision":self.revision(), "zone":self.document().zone,
            "models":models, "textures":pack.textures, "inventory":pack.inventory, "document":self.document(),
            "undo":self.journal.undo.len(), "redo":self.journal.redo.len()}),
        )
    }
    pub fn preview(&self) -> Result<Preview> {
        Preview::new(self.document(), &self.base, &self.root.join("assets"))
    }
    pub fn reload_preview(&self, preview: &mut Preview) -> Result<()> {
        preview.reload(self.document(), &self.base, &self.root.join("assets"))
    }
    pub fn build(&self) -> Result<Build> {
        let (pack, scene, _) = admit(self.document(), &self.base)?;
        let mut preview = self.preview()?;
        let report = preview.step(1)?;
        let document = bytes(self.document(), DOCUMENT_LIMIT)?;
        let generation = hash(&[self.journal.asset_digest.as_bytes(), &document].concat());
        let generations = self.root.join("generations");
        ancestors(&generations)?;
        std::fs::create_dir_all(&generations).map_err(|e| io(&generations, e))?;
        let path = generations.join(&generation);
        let reused = path.exists();
        if !reused {
            let staging = generations.join("building");
            // A previous interruption stays reviewable instead of becoming an admitted generation.
            if staging.exists() {
                return Err(io(
                    &staging,
                    "Interrupted build exists; inspect it and remove it before retrying",
                ));
            }
            std::fs::create_dir(&staging).map_err(|e| io(&staging, e))?;
            let mut files = BTreeMap::new();
            let mut write = |name: &str, data: Vec<u8>| -> Result<()> {
                create(&staging.join(name), &data)?;
                files.insert(name.into(), hash(&data));
                Ok(())
            };
            write("document.json", document)?;
            write("pack.json", bytes(&pack, 128 * 1024 * 1024)?)?;
            write("scene.json", bytes(&scene, 1024 * 1024)?)?;
            write("preview.json", bytes(&report, DOCUMENT_LIMIT)?)?;
            write("preview.svg", preview.svg(None)?.into_bytes())?;
            for texture in &pack.textures {
                write(
                    &texture.file,
                    read(
                        &self.root.join("assets").join(&texture.file),
                        64 * 1024 * 1024,
                    )?,
                )?;
            }
            let absolute = path.clone();
            let host = serde_json::json!({"listen":"127.0.0.1:7777", "instance":1,
                "pack":absolute.join("pack.json"), "scene":absolute.join("scene.json"),
                "certificate_der":"cert.der", "private_key_der":"key.der",
                "guests":{"cap":8,"ring":[0.0,0.0,0.0],"radius":2.0},
                "authored_combat_health":self.document().social_profile.is_none(), "authored":self.document().authored,
                "social_profile":self.document().social_profile,"rewards":self.document().rewards,
                "progression":self.document().progression,"items":self.document().items,
                "outfits":self.document().outfits,"equipment":self.document().equipment});
            write("host-template.json", bytes(&host, 64 * 1024)?)?;
            let seal = Seal {
                schema: "verse.author.generation.v1".into(),
                generation: generation.clone(),
                asset_digest: self.journal.asset_digest.clone(),
                files,
                content: preview.content(),
            };
            create(
                &staging.join("generation.json"),
                &bytes(&seal, DOCUMENT_LIMIT)?,
            )?;
            sync_dir(&staging)?;
            std::fs::rename(&staging, &path).map_err(|e| io(&path, e))?;
            sync_dir(&generations)?;
        }
        let content = self.verify_generation(&path, &generation)?;
        let result = Build {
            schema: "verse.author.build.v1".into(),
            generation,
            content,
            path,
            reused,
            asset_files_reused: pack.textures.len() + 1,
        };
        atomic(
            &self.root.join("current.json"),
            &bytes(&result, DOCUMENT_LIMIT)?,
        )?;
        Ok(result)
    }
    fn verify_generation(&self, path: &Path, generation: &str) -> Result<[u8; 32]> {
        ancestors(path)?;
        let seal: Seal = parse(
            "generation.json",
            &read(&path.join("generation.json"), DOCUMENT_LIMIT)?,
            DOCUMENT_LIMIT,
        )?;
        if seal.schema != "verse.author.generation.v1"
            || seal.generation != generation
            || seal.asset_digest != self.journal.asset_digest
            || seal.files.len() > 520
        {
            return Err(io(path, "Generation seal identity or budget differs"));
        }
        let mut allowed: std::collections::BTreeSet<String> = [
            "document.json",
            "pack.json",
            "scene.json",
            "preview.json",
            "preview.svg",
            "host-template.json",
        ]
        .into_iter()
        .map(String::from)
        .collect();
        allowed.extend(self.base.textures.iter().map(|t| t.file.clone()));
        if seal
            .files
            .keys()
            .cloned()
            .collect::<std::collections::BTreeSet<_>>()
            != allowed
        {
            return Err(io(
                path,
                "Generation file set differs from its admitted asset bundle",
            ));
        }
        let mut total = 0usize;
        for (name, digest) in &seal.files {
            if Path::new(name).components().count() != 1
                || !matches!(
                    Path::new(name).components().next(),
                    Some(std::path::Component::Normal(_))
                )
            {
                return Err(io(path, "Generation file path escapes its directory"));
            }
            let limit = if name == "pack.json" {
                128 * 1024 * 1024
            } else if name.ends_with(".png") {
                64 * 1024 * 1024
            } else {
                DOCUMENT_LIMIT
            };
            let data = read(&path.join(name), limit)?;
            total = total
                .checked_add(data.len())
                .ok_or_else(|| io(path, "Generation byte budget exceeded"))?;
            if total > 640 * 1024 * 1024 {
                return Err(io(path, "Generation byte budget exceeded"));
            }
            if hash(&data) != *digest {
                return Err(io(
                    &path.join(name),
                    "Generation file differs from its seal",
                ));
            }
        }
        let actual: std::collections::BTreeSet<_> = std::fs::read_dir(path)
            .map_err(|e| io(path, e))?
            .map(|entry| entry.map(|e| e.file_name()))
            .collect::<std::io::Result<_>>()
            .map_err(|e| io(path, e))?;
        let expected: std::collections::BTreeSet<_> = seal
            .files
            .keys()
            .map(std::ffi::OsString::from)
            .chain(std::iter::once("generation.json".into()))
            .collect();
        if actual != expected {
            return Err(io(path, "Generation contains unsealed or missing files"));
        }
        let document: Document = parse(
            "document.json",
            &read(&path.join("document.json"), DOCUMENT_LIMIT)?,
            DOCUMENT_LIMIT,
        )?;
        if hash(
            &[
                seal.asset_digest.as_bytes(),
                &bytes(&document, DOCUMENT_LIMIT)?,
            ]
            .concat(),
        ) != generation
        {
            return Err(io(path, "Generation does not match its source document"));
        }
        let (pack, scene, _) = admit(&document, &self.base)?;
        if read(&path.join("pack.json"), 128 * 1024 * 1024)? != bytes(&pack, 128 * 1024 * 1024)?
            || read(&path.join("scene.json"), 1024 * 1024)? != bytes(&scene, 1024 * 1024)?
        {
            return Err(io(path, "Published values differ from runtime compilation"));
        }
        let content = Preview::new(&document, &self.base, path)?.content();
        if content != seal.content {
            return Err(io(path, "Published content identity differs"));
        }
        Ok(content)
    }
}
