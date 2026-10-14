//! Private, restart-safe jobs. Recovery saves the last completed step; it never
//! silently restarts a simulator from an incomplete physical state.

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::PathBuf;

use fs2::FileExt;
use robot_lab::document::{EpisodeEvent, EpisodeEventKind, Outcome, Reference};
use robot_lab::{Document, Store};
use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;

use crate::runner::{Backend, Controller, Event, Mode, Progress, Recording, Runner};
use crate::{Result, require};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Queued,
    Running,
    Paused,
    Finished,
    Interrupted,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Job {
    pub schema: String,
    pub id: String,
    pub experiment: Reference,
    pub seed: u64,
    pub mode: Mode,
    pub status: Status,
    pub recording: Option<Recording>,
    pub episode: Option<Reference>,
}

#[derive(Clone)]
pub struct Book {
    store: Store,
    root: PathBuf,
}

pub struct Worker {
    book: Book,
    job: Job,
    // The OS releases this lock on a crash. Another process can then recover.
    _lock: File,
    checkpoint_pending: bool,
}

impl Book {
    pub fn open(store: Store) -> Result<Self> {
        let root = store.root().join("jobs");
        if root.exists() {
            require(
                fs::symlink_metadata(&root)?.is_dir(),
                "Use an ordinary folder for robot runs.",
            )?;
        }
        fs::create_dir_all(&root)?;
        Ok(Self { store, root })
    }

    pub fn create(&self, id: &str, experiment: Reference, seed: u64, mode: Mode) -> Result<Job> {
        let _lock = self.lock(id)?;
        require(
            !self.path(id, "json").exists(),
            "Choose a different name for this run.",
        )?;
        let Document::Experiment(value) = self.store.get(&experiment)?.content else {
            return Err(crate::Error::Invalid("Choose an experiment to run.".into()));
        };
        require(
            value.seeds.contains(&seed),
            "Choose a seed from this experiment.",
        )?;
        let job = Job {
            schema: "robot-job.v1".into(),
            id: id.into(),
            experiment,
            seed,
            mode,
            status: Status::Queued,
            recording: None,
            episode: None,
        };
        self.write(&job)?;
        Ok(job)
    }

    pub fn get(&self, id: &str) -> Result<Job> {
        valid_id(id)?;
        let path = self.path(id, "json");
        require(
            fs::symlink_metadata(&path)?.file_type().is_file(),
            "Choose an ordinary saved run file.",
        )?;
        let mut bytes = Vec::new();
        File::open(path)?
            .take((robot_lab::document::MAX_DOCUMENT_BYTES + (2 << 20) + 1) as u64)
            .read_to_end(&mut bytes)?;
        require(
            bytes.len() <= robot_lab::document::MAX_DOCUMENT_BYTES + (2 << 20),
            "This saved run is too large.",
        )?;
        let job: Job = serde_json::from_slice(&bytes)?;
        require(
            job.schema == "robot-job.v1" && job.id == id,
            "This run file couldn't be opened.",
        )?;
        require(
            job.status == Status::Queued || job.recording.is_some(),
            "This run is missing its saved motion.",
        )?;
        job.experiment.validate()?;
        if let Some(recording) = &job.recording {
            require(
                recording.schema == "robot-run-recording.v1"
                    && recording.episode.experiment == job.experiment
                    && recording.episode.seed == job.seed,
                "This run file has mismatched experiment settings.",
            )?;
            Document::Episode(recording.episode.clone()).validate()?;
        }
        Ok(job)
    }

    /// A stop request is durable and does not wait for a controller to return.
    pub fn stop(&self, id: &str) -> Result<()> {
        self.get(id)?;
        let path = self.path(id, "stop");
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => {
                file.sync_all()?;
                File::open(&self.root)?.sync_all()?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                require(
                    fs::symlink_metadata(path)?.file_type().is_file(),
                    "Use an ordinary stop request file.",
                )?;
            }
            Err(error) => return Err(error.into()),
        }
        Ok(())
    }

    pub fn start<B: Backend, C: Controller>(
        &self,
        id: &str,
        runner: &Runner<B, C>,
    ) -> Result<Worker> {
        let lock = self.lock(id)?;
        let mut job = self.get(id)?;
        require(
            job.status == Status::Queued,
            "This run has already started. Create another run to retry it.",
        )?;
        let Document::Experiment(saved) = self.store.get(&job.experiment)?.content else {
            return Err(crate::Error::Invalid("Choose a saved experiment.".into()));
        };
        require(
            runner.recording().episode.experiment == job.experiment
                && runner.recording().episode.seed == job.seed
                && runner.mode() == job.mode
                && runner.experiment() == &saved
                && runner.recording().episode.steps.len() == 1,
            "Start this run with its saved experiment and seed.",
        )?;
        job.status = if runner.paused() {
            Status::Paused
        } else {
            Status::Running
        };
        job.recording = Some(runner.recording().clone());
        self.write(&job)?;
        Ok(Worker {
            book: self.clone(),
            job,
            _lock: lock,
            checkpoint_pending: false,
        })
    }

    /// Recover only after acquiring the departed worker's lock. Finished runs
    /// remain finished; active workers cannot be mistaken for crashed workers.
    pub fn recover(&self, id: &str, author: &str, now_unix_ms: u64) -> Result<Job> {
        let _lock = self.lock(id)?;
        let mut job = self.get(id)?;
        if matches!(job.status, Status::Running | Status::Paused) {
            let recording = job.recording.as_mut().unwrap();
            if matches!(recording.episode.outcome, Outcome::InProgress) {
                let step = recording.episode.steps.last().unwrap().index;
                recording.episode.outcome = Outcome::Stopped {
                    reason: "The computer stopped during this run".into(),
                };
                recording.events.push(Event::Stopped { step });
                recording.episode.events.push(EpisodeEvent {
                    step,
                    kind: EpisodeEventKind::Stopped,
                    reason: "The computer stopped during this run".into(),
                });
                job.status = Status::Interrupted;
            } else {
                job.status = Status::Finished;
            }
            self.save_episode(&mut job, author, now_unix_ms)?;
            self.write(&job)?;
        }
        if job.status == Status::Finished && job.episode.is_none() {
            self.save_episode(&mut job, author, now_unix_ms)?;
            self.write(&job)?;
        }
        Ok(job)
    }

    fn save_episode(&self, job: &mut Job, author: &str, now_unix_ms: u64) -> Result<()> {
        if job.episode.is_some() {
            return Ok(());
        }
        let episode = &job.recording.as_ref().unwrap().episode;
        let document = format!("run-{}", job.id);
        // Recover a crash after saving the episode but before saving the job.
        let reference = match self.store.latest(&document) {
            Ok(reference) => Some(reference),
            Err(robot_lab::Error::Missing) => None,
            Err(error) => return Err(error.into()),
        };
        if let Some(reference) = reference {
            require(
                self.store.get(&reference)?.content == Document::Episode(episode.clone()),
                "This saved attempt belongs to a different run.",
            )?;
            job.episode = Some(reference);
        } else {
            job.episode = Some(self.store.save(
                &document,
                None,
                Document::Episode(episode.clone()),
                author,
                "Saved robot attempt",
                now_unix_ms,
            )?);
        }
        Ok(())
    }

    fn lock(&self, id: &str) -> Result<File> {
        valid_id(id)?;
        let path = self.path(id, "lock");
        if path.exists() {
            require(
                fs::symlink_metadata(&path)?.file_type().is_file(),
                "Use an ordinary run lock file.",
            )?;
        }
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)?;
        require(
            file.try_lock_exclusive().is_ok(),
            "This run is still open on another worker.",
        )?;
        Ok(file)
    }
    fn path(&self, id: &str, extension: &str) -> PathBuf {
        self.root.join(format!("{id}.{extension}"))
    }
    fn write(&self, job: &Job) -> Result<()> {
        let bytes = serde_json::to_vec(job)?;
        require(
            bytes.len() <= robot_lab::document::MAX_DOCUMENT_BYTES + (2 << 20),
            "This saved run is too large.",
        )?;
        let mut file = NamedTempFile::new_in(&self.root)?;
        file.write_all(&bytes)?;
        file.as_file().sync_all()?;
        file.persist(self.path(&job.id, "json"))
            .map_err(|e| e.error)?;
        File::open(&self.root)?.sync_all()?;
        Ok(())
    }
}

impl Worker {
    pub fn job(&self) -> &Job {
        &self.job
    }

    /// Save every completed step before admitting another step. Closing a batch
    /// view does not call this worker's stop operation.
    pub fn tick<B: Backend, C: Controller>(
        &mut self,
        runner: &mut Runner<B, C>,
        single_step: bool,
        author: &str,
        now_unix_ms: u64,
    ) -> Result<Progress> {
        require(
            runner.recording().episode.experiment == self.job.experiment
                && runner.recording().episode.seed == self.job.seed
                && runner.mode() == self.job.mode
                && self
                    .job
                    .recording
                    .as_ref()
                    .is_some_and(|saved| saved.episode.steps == runner.recording().episode.steps),
            "Use this worker's experiment and seed.",
        )?;
        if self.checkpoint_pending {
            self.book.write(&self.job)?;
            self.checkpoint_pending = false;
        }
        if self.book.path(&self.job.id, "stop").exists() {
            runner.stop();
        }
        let progress = if single_step {
            runner.single_step()?
        } else {
            runner.tick()?
        };
        self.job.recording = Some(runner.recording().clone());
        self.job.status = if runner.finished() {
            Status::Finished
        } else if runner.paused() {
            Status::Paused
        } else {
            Status::Running
        };
        // Write the terminal checkpoint before the episode, so a crash between
        // these writes preserves its actual outcome during recovery.
        self.checkpoint_pending = true;
        self.book.write(&self.job)?;
        self.checkpoint_pending = false;
        if runner.finished() {
            self.book.save_episode(&mut self.job, author, now_unix_ms)?;
            self.book.write(&self.job)?;
        }
        Ok(progress)
    }
}

fn valid_id(id: &str) -> Result<()> {
    require(
        !id.is_empty()
            && id.len() <= 40
            && id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'),
        "Use a run name with up to 40 letters, numbers, dashes, or underscores.",
    )
}
