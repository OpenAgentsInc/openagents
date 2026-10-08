//! Explicit local owner custody. No paired observer or Studio right implies access.
use super::{Read, Snapshot, Source};
use coder::task::sales::{self, Store};
use std::path::PathBuf;

struct Reader {
    root: PathBuf,
    credential: PathBuf,
}
impl Reader {
    pub fn new(root: PathBuf, credential: PathBuf) -> Result<Self, String> {
        if !root.is_absolute() || !credential.is_absolute() {
            return Err("sales board source needs explicit absolute private paths".into());
        }
        Ok(Self { root, credential })
    }
}
impl Reader {
    fn read(&mut self) -> Result<Snapshot, String> {
        let mut store = Store::open(&self.root)?;
        let secret = Store::read_credential(&self.credential)?;
        let owner = store.authenticate(&secret)?;
        let pipeline = store.read_paul_pipeline(&owner)?;
        let view = store.sales_agent_owner_view(&owner)?;
        let paul = store.sales_agent_anchor(&owner, "paul")?;
        let practice = store.sales_roleplay_schedule(&owner, &paul, None, 64)?;
        let mut snapshot = Snapshot {
            pipeline: [0; 5],
            pending_drafts: 0,
            certificate_records: [0; 3],
            practice_records: practice.len(),
            proposals: Vec::new(),
            outbox_live: Vec::new(),
            outbox_fixture: Vec::new(),
            outbox_unknown: 0,
            idle: pipeline.idle,
            model_available: pipeline.model_available,
            rings: 0,
            shared: None,
        };
        store.ring_earned(&owner)?;
        snapshot.rings = store.earned_ledger(&owner)?.totals.rung;
        if let sales::earned::Shared::Available { aggregate, .. } = store.shared_aggregate()? {
            snapshot.shared = Some([
                aggregate.earned_sales,
                aggregate.net_usd_millionths_floor / 1_000_000,
            ]);
        }
        for row in pipeline.rows {
            let stage = match row.recorded_stage {
                sales::Stage::New => 0,
                sales::Stage::Qualified => 1,
                sales::Stage::Pilot => 2,
                sales::Stage::Active => 3,
                sales::Stage::Closed => 4,
            };
            snapshot.pipeline[stage] += 1;
            snapshot.pending_drafts = snapshot
                .pending_drafts
                .checked_add(row.pending_drafts)
                .ok_or("sales board count overflow")?;
            for meeting in row.meetings {
                if meeting.owner_confirmation_needed && meeting.slot_current {
                    snapshot.proposals.push(meeting.proposal_sha256);
                }
            }
        }
        snapshot.proposals.sort();
        snapshot.proposals.dedup();
        if snapshot.proposals.len() > 16 {
            return Err("sales board proposal bound exceeded".into());
        }
        let outbox = store.sales_outbox_projection(&owner)?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| "sales board clock is unavailable")?
            .as_secs();
        for record in outbox.records {
            if record.phase == sales::outbox::Phase::Unknown {
                snapshot.outbox_unknown += 1;
            }
            if record.phase != sales::outbox::Phase::Proposed
                || record.expires_at <= now
                || record.subject.is_none()
            {
                continue;
            }
            if record
                .subject
                .as_ref()
                .ok_or("original outbox subject missing")?
                .sha256()?
                != record.subject_sha256
            {
                return Err("original outbox proposal changed".into());
            }
            match record.mode {
                sales::outbox::Mode::Live => snapshot.outbox_live.push(record.subject_sha256),
                sales::outbox::Mode::Fixture => snapshot.outbox_fixture.push(record.subject_sha256),
            }
        }
        snapshot.outbox_live.sort();
        snapshot.outbox_live.dedup();
        snapshot.outbox_fixture.sort();
        snapshot.outbox_fixture.dedup();
        for record in view.certificates {
            let index = match record.certification.state {
                sales::agents::CertState::Qualified if record.measured_qualified => 0,
                sales::agents::CertState::Suspended => 2,
                _ => 1,
            };
            snapshot.certificate_records[index] += 1;
        }
        if !snapshot.valid() {
            return Err("sales board projection exceeded its bound".into());
        }
        Ok(snapshot)
    }
}

/// A background reader keeps the sales lock and filesystem off the frame thread.
pub struct LocalOwner {
    request: std::sync::mpsc::SyncSender<()>,
    result: std::sync::mpsc::Receiver<(std::time::Instant, Result<Snapshot, String>)>,
    pending: bool,
    next: std::time::Instant,
}
impl LocalOwner {
    pub fn new(root: PathBuf, credential: PathBuf) -> Result<Self, String> {
        let mut reader = Reader::new(root, credential)?;
        let meta = std::fs::symlink_metadata(&reader.root)
            .map_err(|_| "sales board root is unavailable")?;
        if !meta.is_dir() || meta.file_type().is_symlink() {
            return Err("sales board root must be an existing private directory".into());
        }
        let (request, incoming) = std::sync::mpsc::sync_channel(1);
        let (finished, result) = std::sync::mpsc::channel();
        std::thread::Builder::new()
            .name("sales-board-owner-read".into())
            .spawn(move || {
                while incoming.recv().is_ok() {
                    let value = reader.read();
                    if finished.send((std::time::Instant::now(), value)).is_err() {
                        break;
                    }
                }
            })
            .map_err(|_| "sales board reader is unavailable")?;
        Ok(Self {
            request,
            result,
            pending: false,
            next: std::time::Instant::now(),
        })
    }
    pub fn from_env() -> Result<Self, String> {
        Self::new(
            std::env::var_os("OPENAGENTS_SALES_BOARD_ROOT")
                .map(PathBuf::from)
                .ok_or("sales board source is unconfigured")?,
            std::env::var_os("OPENAGENTS_SALES_BOARD_CREDENTIAL")
                .map(PathBuf::from)
                .ok_or("sales board credential is unconfigured")?,
        )
    }
}
impl Source for LocalOwner {
    fn read(&mut self) -> Read {
        match self.result.try_recv() {
            Ok((at, value)) => {
                self.pending = false;
                self.next = std::time::Instant::now() + std::time::Duration::from_secs(1);
                return match value {
                    Ok(s) if at.elapsed() <= std::time::Duration::from_secs(2) => {
                        Read::Ready(s, at.elapsed().as_secs_f32())
                    }
                    _ => Read::Unavailable,
                };
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => return Read::Unavailable,
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
        }
        if !self.pending && std::time::Instant::now() >= self.next {
            if self.request.try_send(()).is_err() {
                return Read::Unavailable;
            }
            self.pending = true;
        }
        Read::Pending
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use coder::task::{agent, agent_key::FileKeys};
    fn now() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }
    #[test]
    fn canonical_private_owner_read_reopens_and_refuses_changed_custody() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("private-host");
        let credential = dir.path().join("private-owner-credential");
        let mut store = Store::open(&root).unwrap();
        store
            .initialize("private-company-message-canary", &credential)
            .unwrap();
        let owner = store
            .authenticate(&Store::read_credential(&credential).unwrap())
            .unwrap();
        let native = agent::Store::with_keys(&root, "paul", std::sync::Arc::new(FileKeys)).unwrap();
        let record = native.open(dir.path(), now()).unwrap();
        let record = native.ensure_key(record, now()).unwrap();
        let key = agent::parse_secret(&"11".repeat(32)).unwrap();
        native.attest(record, &key, now() + 3600, now()).unwrap();
        let anchor = store.sales_agent_anchor(&owner, "paul").unwrap();
        let binding = sales::paul::Binding {
            schema: sales::paul::SCHEMA.into(),
            revision: 1,
            anchor,
            owner_credential: credential.clone(),
            assignments: vec![],
            permitted_requesters: vec!["owner".into()],
        };
        store
            .configure_paul(&owner, &binding, &binding.sha256().unwrap())
            .unwrap();
        let member_credential = dir.path().join("private-member-credential");
        store
            .issue(&owner, "member", sales::Role::Writer, &member_credential)
            .unwrap();
        drop(store);
        let mut member = Reader::new(root.clone(), member_credential).unwrap();
        assert!(member.read().is_err());
        let mut reader = Reader::new(root.clone(), credential.clone()).unwrap();
        let first = reader.read().unwrap();
        assert!(first.idle);
        assert_eq!(first.pipeline, [0; 5]);
        assert!(!first.model_available);
        assert_eq!(first, reader.read().unwrap());
        let mut floor = super::super::Floor::default();
        struct One(Option<Snapshot>);
        impl Source for One {
            fn read(&mut self) -> Read {
                self.0
                    .take()
                    .map(|s| Read::Ready(s, 0.0))
                    .unwrap_or(Read::Unavailable)
            }
        }
        floor.set_source(Some(Box::new(One(Some(first)))));
        floor.poll(true, 0.1);
        assert!(
            !floor
                .lines()
                .join(" ")
                .contains("private-company-message-canary")
        );
        let mut record = native.load().unwrap().unwrap();
        record.state = agent::State::Paused;
        native.save(&record).unwrap();
        assert!(reader.read().is_err());
        record.state = agent::State::Active;
        native.save(&record).unwrap();
        assert!(reader.read().unwrap().idle);
        std::fs::remove_file(credential).unwrap();
        assert!(reader.read().is_err());
        floor.poll(true, 0.1);
        assert!(floor.snapshot().is_none());
    }
}
