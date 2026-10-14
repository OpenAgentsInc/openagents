//! Content-addressed files and version histories under an explicit private root.

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use fs2::FileExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tempfile::NamedTempFile;

use crate::document::*;
use crate::{Error, Result, require};

pub use crate::version::{Note, Selection, Version};

#[derive(Clone, Debug)]
pub struct Store {
    root: PathBuf,
    project: String,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Project {
    schema: String,
    project: String,
}

impl Store {
    /// Bind a private file store to an existing project identified by the caller.
    /// No home directory or checkout path is inferred.
    pub fn open(root: impl AsRef<Path>, project: &str) -> Result<Self> {
        text(project, 256)?;
        require(
            root.as_ref().is_absolute(),
            "Choose an absolute folder for the lab's files.",
        )?;
        fs::create_dir_all(root.as_ref())?;
        let root = fs::canonicalize(root.as_ref())?;
        require(
            !root.ancestors().any(|p| p.join(".git").exists()),
            "Choose a lab folder outside the Git checkout.",
        )?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&root, fs::Permissions::from_mode(0o700))?;
        }
        let store = Self {
            root,
            project: project.into(),
        };
        for folder in ["assets", "versions", "documents", "notes", "selections"] {
            let path = store.root.join(folder);
            if path.exists() {
                require(
                    fs::symlink_metadata(&path)?.is_dir(),
                    "Choose a lab folder with ordinary subfolders.",
                )?;
            }
            fs::create_dir_all(path)?;
        }
        let _lock = store.lock()?;
        let path = store.root.join("project.json");
        if path.exists() {
            let saved: Project = store.read(&path, 4_096)?;
            require(
                saved.schema == "robot-lab.project.v1" && saved.project == project,
                "This lab folder belongs to a different project.",
            )?;
        } else {
            store.replace(
                &path,
                &Project {
                    schema: "robot-lab.project.v1".into(),
                    project: project.into(),
                },
            )?;
        }
        Ok(store)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
    pub fn project(&self) -> &str {
        &self.project
    }

    fn lock(&self) -> Result<File> {
        let path = self.root.join("write.lock");
        if path.exists() {
            regular(&path)?;
        }
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(path)?;
        FileExt::lock_exclusive(&file)?;
        Ok(file)
    }

    fn read<T: for<'de> Deserialize<'de>>(&self, path: &Path, limit: usize) -> Result<T> {
        let bytes = read_bounded(path, limit)?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    fn replace<T: Serialize>(&self, path: &Path, value: &T) -> Result<()> {
        if path.exists() {
            regular(path)?;
        }
        let bytes = serde_json::to_vec(value)?;
        require(
            bytes.len() <= MAX_DOCUMENT_BYTES + 16_384,
            "This saved item is too large.",
        )?;
        let parent = path.parent().ok_or(Error::Corrupt)?;
        let mut file = NamedTempFile::new_in(parent)?;
        file.write_all(&bytes)?;
        file.as_file().sync_all()?;
        file.persist(path).map_err(|e| e.error)?;
        sync_directory(parent)?;
        Ok(())
    }

    /// Stream an asset into the store. Hashes name files; input names never
    /// become paths. A failed stream leaves no visible asset.
    pub fn put_asset(
        &self,
        mut input: impl Read,
        media_type: &str,
        max_bytes: u64,
    ) -> Result<Asset> {
        text(media_type, 128)?;
        require(
            (1..=4 << 30).contains(&max_bytes),
            "Set a file size limit between 1 byte and 4 GB.",
        )?;
        let mut file = NamedTempFile::new_in(self.root.join("assets"))?;
        let mut hasher = Sha256::new();
        let mut total = 0_u64;
        let mut buffer = [0; 65_536];
        loop {
            let n = input.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            total += n as u64;
            require(
                total <= max_bytes,
                "This file is larger than the lab allows.",
            )?;
            hasher.update(&buffer[..n]);
            file.write_all(&buffer[..n])?;
        }
        require(total > 0, "Choose a file with content.")?;
        let asset = Asset {
            sha256: format!("{:x}", hasher.finalize()),
            bytes: total,
            media_type: media_type.into(),
        };
        file.as_file().sync_all()?;
        let path = self.root.join("assets").join(&asset.sha256);
        match file.persist_noclobber(&path) {
            Ok(_) => sync_directory(&self.root.join("assets"))?,
            Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => {
                self.verify_asset(&asset)?
            }
            Err(error) => return Err(error.error.into()),
        }
        Ok(asset)
    }

    pub fn verify_asset(&self, asset: &Asset) -> Result<()> {
        asset.validate()?;
        let path = self.root.join("assets").join(&asset.sha256);
        regular(&path)?;
        let mut input = File::open(path)?;
        require(
            input.metadata()?.len() == asset.bytes,
            "The saved file's size has changed.",
        )?;
        let mut hasher = Sha256::new();
        let mut buffer = [0; 65_536];
        let mut total = 0_u64;
        loop {
            let n = input.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            total += n as u64;
            require(total <= asset.bytes, "The saved file's size has changed.")?;
            hasher.update(&buffer[..n]);
        }
        require(
            total == asset.bytes && format!("{:x}", hasher.finalize()) == asset.sha256,
            "The saved file has changed. Import it again.",
        )
    }

    pub fn asset_bytes(&self, asset: &Asset, max_bytes: usize) -> Result<Vec<u8>> {
        asset.validate()?;
        require(
            asset.bytes <= max_bytes as u64,
            "This file is too large to open here.",
        )?;
        let bytes = read_bounded(&self.root.join("assets").join(&asset.sha256), max_bytes)?;
        require(
            bytes.len() as u64 == asset.bytes && digest(&bytes) == asset.sha256,
            "The saved file has changed. Import it again.",
        )?;
        Ok(bytes)
    }

    pub fn get(&self, reference: &Reference) -> Result<Version> {
        reference.validate()?;
        let path = self
            .root
            .join("versions")
            .join(format!("{}.json", reference.version));
        let version: Version = self.read(&path, MAX_DOCUMENT_BYTES + 16_384)?;
        if version.schema != "robot-lab.version.v1"
            || version.document != reference.document
            || version.number == 0
            || version.reference()? != *reference
            || digest(&serde_json::to_vec(&version.content)?) != version.content_sha256
        {
            return Err(Error::Corrupt);
        }
        identifier(&version.document)?;
        text(&version.author, 256)?;
        text(&version.summary, 1_024)?;
        if let Some(parent) = &version.parent {
            parent.validate()?;
            require(
                parent.document == version.document && version.number > 1,
                "This saved version has an invalid parent.",
            )?;
        } else {
            require(
                version.number == 1,
                "This saved version is missing its parent.",
            )?;
        }
        version.content.validate()?;
        Ok(version)
    }

    pub fn latest(&self, document: &str) -> Result<Reference> {
        identifier(document)?;
        let reference: Reference = self.read(
            &self.root.join("documents").join(format!("{document}.json")),
            4_096,
        )?;
        require(
            reference.document == document,
            "This saved item has the wrong name.",
        )?;
        self.get(&reference)?;
        Ok(reference)
    }

    pub fn list(&self) -> Result<Vec<Reference>> {
        let mut result = Vec::new();
        for entry in fs::read_dir(self.root.join("documents"))? {
            let path = entry?.path();
            if path.extension().is_some_and(|e| e == "json") {
                let name = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .ok_or(Error::Corrupt)?;
                result.push(self.latest(name)?);
                require(
                    result.len() <= 10_000,
                    "This lab has too many items to list at once.",
                )?;
            }
        }
        result.sort();
        Ok(result)
    }

    pub fn versions(&self, document: &str) -> Result<Vec<Version>> {
        let mut at = Some(self.latest(document)?);
        let mut versions = Vec::new();
        while let Some(reference) = at {
            let version = self.get(&reference)?;
            if let Some(previous) = versions.last() {
                let previous: &Version = previous;
                require(
                    version.number.checked_add(1) == Some(previous.number),
                    "This saved history is damaged.",
                )?;
            }
            at = version.parent.clone();
            versions.push(version);
            require(
                versions.len() <= 10_000,
                "This item has too many versions to list at once.",
            )?;
        }
        versions.reverse();
        Ok(versions)
    }

    /// `base = None` creates an item; an edit must name its exact latest version.
    /// Version bytes are synced before the head is replaced atomically. An
    /// interrupted save can leave an unreachable version, never a partial head.
    pub fn save(
        &self,
        document: &str,
        base: Option<&Reference>,
        content: Document,
        author: &str,
        summary: &str,
        created_unix_ms: u64,
    ) -> Result<Reference> {
        identifier(document)?;
        text(author, 256)?;
        text(summary, 1_024)?;
        content.validate()?;
        let _lock = self.lock()?;
        let latest = match self.latest(document) {
            Ok(value) => Some(value),
            Err(Error::Missing) => None,
            Err(error) => return Err(error),
        };
        if latest.as_ref() != base {
            return Err(Error::Conflict);
        }
        let number = if let Some(reference) = base {
            let old = self.get(reference)?;
            require(
                old.content.kind() == content.kind(),
                "Keep the same item type when editing.",
            )?;
            old.number.checked_add(1).ok_or(Error::Corrupt)?
        } else {
            1
        };
        self.validate_dependencies(&content)?;
        let version = Version {
            schema: "robot-lab.version.v1".into(),
            document: document.into(),
            number,
            parent: base.cloned(),
            author: author.into(),
            summary: summary.into(),
            created_unix_ms,
            content_sha256: digest(&serde_json::to_vec(&content)?),
            content,
        };
        let reference = version.reference()?;
        let path = self
            .root
            .join("versions")
            .join(format!("{}.json", reference.version));
        if path.exists() {
            require(
                self.get(&reference)? == version,
                "This saved version has changed.",
            )?;
        } else {
            self.replace(&path, &version)?;
        }
        self.replace(
            &self.root.join("documents").join(format!("{document}.json")),
            &reference,
        )?;
        Ok(reference)
    }

    fn robot(&self, reference: &Reference) -> Result<Robot> {
        match self.get(reference)?.content {
            Document::Robot(robot) => Ok(robot),
            _ => Err(Error::Invalid("Choose a robot definition.".into())),
        }
    }

    fn scene(&self, reference: &Reference) -> Result<Scene> {
        match self.get(reference)?.content {
            Document::Scene(scene) => Ok(scene),
            _ => Err(Error::Invalid("Choose a robot scene.".into())),
        }
    }

    fn experiment(&self, reference: &Reference) -> Result<Experiment> {
        match self.get(reference)?.content {
            Document::Experiment(experiment) => Ok(experiment),
            _ => Err(Error::Invalid("Choose an experiment.".into())),
        }
    }

    pub fn validate_dependencies(&self, document: &Document) -> Result<()> {
        for asset in document.assets() {
            self.verify_asset(asset)?;
        }
        match document {
            Document::Robot(_) => {}
            Document::Scene(scene) => {
                for instance in &scene.robots {
                    self.robot(&instance.robot)?
                        .validate_pose(&instance.joints)?;
                }
                for camera in &scene.cameras {
                    if let Some((id, link)) = &camera.robot_link {
                        let instance = scene
                            .robots
                            .iter()
                            .find(|r| r.id == *id)
                            .ok_or(Error::Corrupt)?;
                        require(
                            self.robot(&instance.robot)?
                                .links
                                .iter()
                                .any(|l| l.name == *link),
                            "Attach the camera to a link in this robot.",
                        )?;
                    }
                }
            }
            Document::Experiment(experiment) => {
                let scene = self.scene(&experiment.scene)?;
                let instance = scene
                    .robots
                    .iter()
                    .find(|r| r.id == experiment.controller.robot())
                    .ok_or_else(|| {
                        Error::Invalid("Choose a controller for a robot in this scene.".into())
                    })?;
                let robot = self.robot(&instance.robot)?;
                require(
                    robot.capabilities.contains(&Capability::Simulate),
                    "This robot doesn't support simulation.",
                )?;
                for joint in experiment.controller.joints() {
                    require(
                        robot
                            .joints
                            .iter()
                            .any(|j| j.name == *joint && j.kind != JointKind::Fixed),
                        "Map the controller to movable joints in this robot.",
                    )?;
                }
                require(
                    scene.objects.iter().any(|o| {
                        o.id == experiment.task.object && o.mass > 0.0 && o.collider.is_some()
                    }),
                    "Choose a moving object with a collision shape for the task.",
                )?;
                if let Controller::Policy { cameras, .. } = &experiment.controller {
                    for camera in cameras {
                        require(
                            scene.cameras.iter().any(|c| c.id == *camera),
                            "This policy needs a camera that isn't in the scene.",
                        )?;
                    }
                }
            }
            Document::Episode(episode) => {
                let experiment = self.experiment(&episode.experiment)?;
                require(
                    experiment.seeds.contains(&episode.seed)
                        && episode.steps.len() <= experiment.limits.max_steps as usize,
                    "Keep the episode inside its experiment's seeds and run length.",
                )?;
                require(
                    episode.wall_seconds <= f64::from(experiment.limits.wall_seconds)
                        || matches!(
                            episode.outcome,
                            Outcome::Failed { .. } | Outcome::Stopped { .. }
                        ),
                    "The episode exceeds its experiment's time limit.",
                )?;
                let scene = self.scene(&experiment.scene)?;
                let instance = scene
                    .robots
                    .iter()
                    .find(|r| r.id == experiment.controller.robot())
                    .ok_or(Error::Corrupt)?;
                let robot = self.robot(&instance.robot)?;
                let mut asset_bytes = 0_u64;
                let mut seen_assets = std::collections::BTreeSet::new();
                for step in &episode.steps {
                    robot.validate_state(&step.joints)?;
                    for (name, value) in &step.joints {
                        let joint = robot
                            .joints
                            .iter()
                            .find(|j| j.name == *name)
                            .ok_or(Error::Corrupt)?;
                        if joint
                            .limits
                            .is_some_and(|limits| *value < limits.lower || *value > limits.upper)
                        {
                            require(
                                step.limit_violations.contains(name),
                                "Record each joint position that exceeds its limits.",
                            )?;
                        }
                    }
                    robot.validate_pose(&step.actions)?;
                    require(
                        step.actions
                            .keys()
                            .all(|j| experiment.controller.joints().contains(j)),
                        "The episode uses an unmapped action joint.",
                    )?;
                    require(
                        step.objects
                            .keys()
                            .all(|id| scene.objects.iter().any(|o| o.id == *id)),
                        "The episode names an object outside its scene.",
                    )?;
                    require(
                        step.sensors
                            .keys()
                            .all(|id| scene.cameras.iter().any(|c| c.id == *id)),
                        "The episode names a camera outside its scene.",
                    )?;
                    if episode.mode == EpisodeMode::Simulation {
                        require(
                            (step.time_seconds
                                - f64::from(step.index) * experiment.timestep_seconds)
                                .abs()
                                <= experiment.timestep_seconds * 1e-6,
                            "Keep simulation times on the experiment's fixed step.",
                        )?;
                    }
                    for sensor in step.sensors.values() {
                        if seen_assets.insert(&sensor.asset.sha256) {
                            asset_bytes = asset_bytes
                                .checked_add(sensor.asset.bytes)
                                .ok_or(Error::Corrupt)?;
                        }
                    }
                }
                require(
                    asset_bytes <= experiment.limits.max_artifact_bytes,
                    "The episode's camera files exceed its storage limit.",
                )?;
            }
        }
        Ok(())
    }

    pub fn add_note(&self, note: Note) -> Result<()> {
        text(&note.author, 256)?;
        text(&note.text, 4_096)?;
        let subject = self.get(&note.subject)?;
        if let Some(step) = note.episode_step {
            require(
                matches!(&subject.content, Document::Episode(e) if (step as usize) < e.steps.len()),
                "Choose a step from this saved episode.",
            )?;
        }
        let _lock = self.lock()?;
        let path = self
            .root
            .join("notes")
            .join(format!("{}.json", note.subject.version));
        let mut notes = self.notes(&note.subject)?;
        require(notes.len() < 1_000, "This version already has 1000 notes.")?;
        if !notes.contains(&note) {
            notes.push(note);
        }
        self.replace(&path, &notes)
    }

    pub fn notes(&self, subject: &Reference) -> Result<Vec<Note>> {
        self.get(subject)?;
        let path = self
            .root
            .join("notes")
            .join(format!("{}.json", subject.version));
        let notes: Vec<Note> = match self.read(&path, 8 << 20) {
            Ok(notes) => notes,
            Err(Error::Missing) => Vec::new(),
            Err(error) => return Err(error),
        };
        require(
            notes.len() <= 1_000 && notes.iter().all(|n| n.subject == *subject),
            "This version's notes are damaged.",
        )?;
        Ok(notes)
    }

    pub fn select(&self, chat: &str, selection: &Selection) -> Result<()> {
        self.select_checked(chat, selection, None)
    }

    /// Compare the chat selection under the writer lock before replacing it.
    /// `None` as the expected value requires a previously unselected chat.
    pub fn select_if_unchanged(
        &self,
        chat: &str,
        selection: &Selection,
        expected: Option<&str>,
    ) -> Result<()> {
        self.select_checked(chat, selection, Some(expected))
    }
    fn select_checked(
        &self,
        chat: &str,
        selection: &Selection,
        expected: Option<Option<&str>>,
    ) -> Result<()> {
        identifier(chat)?;
        let scene = self.scene(&selection.scene)?;
        require(
            selection.camera == "orbit" || scene.cameras.iter().any(|c| c.id == selection.camera),
            "Choose a camera from this scene.",
        )?;
        if let Some(reference) = &selection.episode {
            let Document::Episode(episode) = self.get(reference)?.content else {
                return Err(Error::Invalid("Choose a recorded episode.".into()));
            };
            require(
                self.experiment(&episode.experiment)?.scene == selection.scene
                    && selection
                        .step
                        .is_some_and(|step| (step as usize) < episode.steps.len()),
                "Choose a step from an episode of this scene version.",
            )?;
        } else {
            require(
                selection.step.is_none(),
                "Choose an episode before choosing a step.",
            )?;
        }
        if let Some(other) = &selection.comparison {
            match (selection.episode.is_some(), self.get(other)?.content) {
                (false, Document::Scene(_)) => (),
                (true, Document::Episode(episode)) => {
                    self.experiment(&episode.experiment)?;
                }
                _ => {
                    return Err(Error::Invalid(
                        "Compare two scenes or two recordings.".into(),
                    ));
                }
            }
        }
        let _lock = self.lock()?;
        if let Some(expected) = expected {
            let current = self.selection(chat)?;
            let digest = current.map(|value| value.sha256()).transpose()?;
            if digest.as_deref() != expected {
                return Err(Error::Conflict);
            }
        }
        self.replace(
            &self.root.join("selections").join(format!("{chat}.json")),
            selection,
        )
    }

    pub fn selection(&self, chat: &str) -> Result<Option<Selection>> {
        identifier(chat)?;
        let value: Option<Selection> = match self.read(
            &self.root.join("selections").join(format!("{chat}.json")),
            16_384,
        ) {
            Ok(value) => Some(value),
            Err(Error::Missing) => None,
            Err(error) => return Err(error),
        };
        if let Some(selection) = &value {
            self.get(&selection.scene)?;
            if let Some(episode) = &selection.episode {
                self.get(episode)?;
            }
            if let Some(other) = &selection.comparison {
                self.get(other)?;
            }
        }
        Ok(value)
    }
}

pub fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn regular(path: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            Error::Missing
        } else {
            Error::Io(error)
        }
    })?;
    require(
        metadata.file_type().is_file(),
        "Use an ordinary saved file, without a shortcut or link.",
    )
}

fn read_bounded(path: &Path, limit: usize) -> Result<Vec<u8>> {
    regular(path)?;
    let mut file = File::open(path)?.take(limit as u64 + 1);
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    require(
        bytes.len() <= limit,
        "This saved file is too large to open.",
    )?;
    Ok(bytes)
}

fn sync_directory(path: &Path) -> Result<()> {
    #[cfg(unix)]
    File::open(path)?.sync_all()?;
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}
