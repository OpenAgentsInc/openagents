use crate::{Error, Event, Lease, Result, require};
use lab_kit::{Artifact, DocumentMeta, JobState, JobStatus, Note, Reference};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeMap;

pub(crate) const SCHEMA: &str = "openagents.lab-store.space.v1";

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct State {
    pub schema: String,
    #[serde(default)]
    pub requests: BTreeMap<String, String>,
    pub documents: BTreeMap<String, DocumentRecord>,
    pub artifacts: BTreeMap<String, Artifact>,
    pub jobs: BTreeMap<String, JobRecord>,
    pub notes: Vec<Note>,
    pub events: Vec<Event>,
    pub cursor: u64,
}
#[derive(Serialize, Deserialize)]
pub(crate) struct DocumentRecord {
    pub meta: DocumentMeta,
    pub deleted: bool,
    pub versions: BTreeMap<u64, String>,
}
#[derive(Serialize, Deserialize)]
pub(crate) struct JobRecord {
    pub state: JobState,
    pub created: u64,
    pub attempt: u64,
    pub lease: Option<Lease>,
    pub settled: Option<Lease>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            schema: SCHEMA.into(),
            requests: Default::default(),
            documents: Default::default(),
            artifacts: Default::default(),
            jobs: Default::default(),
            notes: vec![],
            events: vec![],
            cursor: 0,
        }
    }
}
impl State {
    pub fn document(&self, id: &str) -> Result<&DocumentRecord> {
        self.documents
            .get(id)
            .filter(|d| !d.deleted)
            .ok_or(Error::NotFound)
    }
    pub fn document_mut(&mut self, id: &str) -> Result<&mut DocumentRecord> {
        self.documents
            .get_mut(id)
            .filter(|d| !d.deleted)
            .ok_or(Error::NotFound)
    }
    pub fn reference(&self, r: &Reference) -> Result<()> {
        if self
            .document(&r.document)?
            .versions
            .contains_key(&r.version)
        {
            Ok(())
        } else {
            Err(Error::NotFound)
        }
    }
    pub fn event(&mut self, document: &str, now: u64, change: serde_json::Value) -> Result<()> {
        require(
            self.events.len() < 100_000,
            "This space has reached its activity limit.",
        )?;
        self.cursor = self.cursor.checked_add(1).ok_or(Error::Corrupt)?;
        self.events.push(Event {
            cursor: self.cursor,
            document: document.into(),
            at_unix_ms: now,
            change,
        });
        Ok(())
    }
    pub fn leased(&mut self, lease: &Lease, now: u64) -> Result<&mut JobRecord> {
        self.document(&lease.job.spec.version.document)?;
        let j = self.jobs.get_mut(&lease.job.id).ok_or(Error::NotFound)?;
        let l = j.lease.as_ref().ok_or(Error::LeaseLost)?;
        if j.state.status != JobStatus::Running
            || l.worker != lease.worker
            || l.attempt != lease.attempt
            || now >= l.expires_unix_ms
        {
            return Err(Error::LeaseLost);
        }
        Ok(j)
    }
    pub fn expire(&mut self, now: u64) -> Result<usize> {
        let mut events = vec![];
        for (id, j) in &mut self.jobs {
            if let Some(l) = &j.lease {
                if now >= l.expires_unix_ms {
                    j.state.status = if l.started {
                        JobStatus::Uncertain
                    } else {
                        JobStatus::Queued
                    };
                    j.lease = None;
                    events.push((
                        j.state.spec.version.document.clone(),
                        json!({"type":"job","id":id,"status":j.state.status}),
                    ));
                }
            }
        }
        let count = events.len();
        for (doc, change) in events {
            self.event(&doc, now, change)?;
        }
        Ok(count)
    }
}
