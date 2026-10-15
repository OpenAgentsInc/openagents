//! Host-owned requests for waiting SDK callbacks. A reply is not a new turn.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use route_contract::{digest_of, observation::Scope, pending as wire};
use serde::{Deserialize, Serialize};

use super::{Error, Status, Store, owner};

pub use wire::{Answer, Authority, Form, Prompt, Receipt, Reply, Request, Resolution, Snapshot};

const SCHEMA: &str = "openagents.coder.pending-book.v1";
const MAX_BYTES: u64 = 8 * 1024 * 1024;
pub const TTL: u64 = 120;
pub const VIEWER_TTL: u64 = 10;

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Book {
    schema: String,
    authority: Authority,
    snapshot: Snapshot,
}

impl Book {
    fn new(scope: Scope, authority: Authority) -> Self {
        Self {
            schema: SCHEMA.into(),
            authority,
            snapshot: Snapshot {
                schema: wire::SNAPSHOT_SCHEMA.into(),
                scope,
                revision: 1,
                active: true,
                entries: Vec::new(),
            },
        }
    }

    fn expire(&mut self, now: u64) {
        for entry in &mut self.snapshot.entries {
            if matches!(entry.resolution, None | Some(Resolution::Answered))
                && entry.request.expires_at <= now
            {
                entry.resolution = Some(Resolution::Expired);
                self.snapshot.revision += 1;
            }
        }
    }

    fn open(&mut self, request: Request, now: u64) -> Result<Request, Error> {
        self.expire(now);
        if !self.snapshot.active
            || !request.valid()
            || request.scope != self.snapshot.scope
            || request.authority != self.authority
            || request.expires_at <= now
            || request.expires_at > now.saturating_add(TTL)
        {
            return Err(Error::InvalidTransition);
        }
        if let Some(existing) = self.snapshot.entries.iter_mut().find(|entry| {
            entry.request.native_call == request.native_call && entry.request.child == request.child
        }) {
            if existing.request.input_digest != request.input_digest
                || existing.request.form != request.form
                || existing.request.tool != request.tool
                || existing.request.prompt != request.prompt
            {
                if !matches!(existing.resolution, Some(Resolution::Consumed)) {
                    existing.resolution = Some(Resolution::Cancelled);
                    self.snapshot.revision += 1;
                }
                return Err(Error::Conflict);
            }
            return Ok(existing.request.clone());
        }
        if self.snapshot.entries.len() >= wire::MAX_REQUESTS {
            let Some(at) = self.snapshot.entries.iter().position(|entry| {
                entry
                    .resolution
                    .is_some_and(|state| state != Resolution::Answered)
            }) else {
                return Err(Error::LimitExceeded);
            };
            self.snapshot.entries.remove(at);
        }
        self.snapshot.entries.push(wire::Entry {
            request: request.clone(),
            resolution: None,
            answer: None,
        });
        self.snapshot.revision += 1;
        Ok(request)
    }

    fn reply(&mut self, reply: &Reply, now: u64) -> Result<Receipt, Error> {
        self.expire(now);
        if !self.snapshot.active || reply.scope != self.snapshot.scope {
            return Err(Error::InvalidTransition);
        }
        let entry = self
            .snapshot
            .entries
            .iter_mut()
            .find(|entry| entry.request.id == reply.request)
            .ok_or(Error::NotFound)?;
        if !entry.request.accepts(reply) {
            return Err(Error::InvalidCommand(
                "this answer does not match the request",
            ));
        }
        if let Some(state) = entry.resolution {
            return if matches!(
                state,
                Resolution::Answered | Resolution::Consumed | Resolution::Cancelled
            ) && entry.answer.as_ref() == Some(&reply.answer)
            {
                Ok(Receipt {
                    request: reply.request.clone(),
                    resolution: state,
                    duplicate: true,
                })
            } else {
                Err(Error::Conflict)
            };
        }
        let state = if reply.answer == Answer::Cancel {
            Resolution::Cancelled
        } else {
            Resolution::Answered
        };
        entry.resolution = Some(state);
        entry.answer = Some(reply.answer.clone());
        self.snapshot.revision += 1;
        Ok(Receipt {
            request: reply.request.clone(),
            resolution: state,
            duplicate: false,
        })
    }

    fn consume(&mut self, request: &Request, now: u64) -> Result<Option<Answer>, Error> {
        self.expire(now);
        if !self.snapshot.active || !request.valid() || request.scope != self.snapshot.scope {
            return Err(Error::InvalidTransition);
        }
        let entry = self
            .snapshot
            .entries
            .iter_mut()
            .find(|entry| entry.request.id == request.id)
            .ok_or(Error::NotFound)?;
        if entry.request != *request {
            return Err(Error::Conflict);
        }
        match entry.resolution {
            Some(Resolution::Answered) => {
                entry.resolution = Some(Resolution::Consumed);
                self.snapshot.revision += 1;
                Ok(entry.answer.clone())
            }
            None => Ok(None),
            Some(Resolution::Consumed) if matches!(request.form, Form::Questions { .. }) => {
                Ok(entry.answer.clone())
            }
            Some(Resolution::Cancelled | Resolution::Expired)
                if matches!(entry.answer, Some(Answer::Deny | Answer::Cancel)) =>
            {
                Ok(entry.answer.clone())
            }
            _ => Err(Error::InvalidTransition),
        }
    }

    fn close(&mut self, request: Option<&route_contract::Digest>, resolution: Resolution) {
        let mut changed = request.is_none() && self.snapshot.active;
        if request.is_none() {
            self.snapshot.active = false;
        }
        for entry in &mut self.snapshot.entries {
            if request.is_none_or(|id| id == &entry.request.id)
                && matches!(entry.resolution, None | Some(Resolution::Answered))
            {
                entry.resolution = Some(resolution);
                changed = true;
            }
        }
        if changed {
            self.snapshot.revision += 1;
        }
    }
}

/// One provider attempt's registry. Clones keep the same bound authority.
#[derive(Clone, Debug)]
pub struct Service {
    store: PathBuf,
    scope: Scope,
    authority: Authority,
}

impl Service {
    pub(crate) fn begin(store: &Path, scope: Scope, authority: Authority) -> Result<Self, Error> {
        let service = Self {
            store: store.into(),
            scope,
            authority,
        };
        service.update(|book| {
            if let Some(old) = book {
                if old.snapshot.scope == service.scope
                    && old.authority == service.authority
                    && old.snapshot.active
                {
                    return Ok(());
                }
                if (old.snapshot.scope.turn, old.snapshot.scope.generation)
                    >= (service.scope.turn, service.scope.generation)
                {
                    return Err(Error::InvalidTransition);
                }
            }
            *book = Some(Book::new(service.scope.clone(), service.authority.clone()));
            Ok(())
        })?;
        Ok(service)
    }

    pub fn scope(&self) -> &Scope {
        &self.scope
    }
    pub fn authority(&self) -> &Authority {
        &self.authority
    }

    pub fn active(&self) -> bool {
        self.current().is_ok() && self.snapshot().is_ok_and(|snapshot| snapshot.active)
    }

    pub fn open(
        &self,
        native_call: &str,
        child: Option<String>,
        tool: &str,
        input: serde_json::Value,
        prompt: Prompt,
        form: Form,
        now: u64,
    ) -> Result<Request, Error> {
        let mut request = Request {
            schema: wire::SCHEMA.into(),
            id: digest_of(&()),
            scope: self.scope.clone(),
            authority: self.authority.clone(),
            native_call: native_call.into(),
            child,
            tool: tool.into(),
            input,
            input_digest: digest_of(&()),
            prompt,
            form,
            expires_at: now.saturating_add(TTL),
        };
        request.seal();
        self.with_active(|| self.bound(|book| book.open(request, now)))
    }

    pub fn snapshot(&self) -> Result<Snapshot, Error> {
        self.bound(|book| {
            book.expire(super::autostart::unix_now());
            Ok(book.snapshot.clone())
        })
    }

    pub fn close(&self) -> Result<(), Error> {
        self.bound(|book| {
            book.close(None, Resolution::Cancelled);
            Ok(())
        })
    }

    /// Wait for a committed answer. Missing viewers and abandoned waits deny.
    pub async fn wait(&self, request: &Request) -> Result<Answer, Error> {
        let attached = Instant::now();
        let mut seen_viewer = false;
        loop {
            let now = super::autostart::unix_now();
            let answer = self.with_active(|| self.bound(|book| book.consume(request, now)));
            match answer {
                Ok(Some(answer)) => return Ok(answer),
                Ok(None) => {}
                Err(error) => {
                    let _ = self.bound(|book| {
                        book.close(Some(&request.id), Resolution::Cancelled);
                        Ok(())
                    });
                    return Err(error);
                }
            }
            let viewer = viewer(&self.store, &self.scope.task, now).unwrap_or(false);
            seen_viewer |= viewer;
            if !viewer && (seen_viewer || attached.elapsed() >= Duration::from_secs(2)) {
                self.bound(|book| {
                    book.close(Some(&request.id), Resolution::Cancelled);
                    Ok(())
                })?;
                return Err(Error::InvalidTransition);
            }
            if attached.elapsed() >= Duration::from_secs(TTL) {
                self.bound(|book| {
                    book.close(Some(&request.id), Resolution::Expired);
                    Ok(())
                })?;
                return Err(Error::InvalidTransition);
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    /// Commit while the task's current grant and checkout are locked.
    pub fn with_active<T>(&self, apply: impl FnOnce() -> Result<T, Error>) -> Result<T, Error> {
        with_current(&self.store, &self.scope, &self.authority, || {
            if !self.bound(|book| Ok(book.snapshot.active))? {
                return Err(Error::InvalidTransition);
            }
            apply()
        })
    }

    fn current(&self) -> Result<(), Error> {
        current(&self.store, &self.scope, &self.authority)
    }

    fn bound<T>(&self, apply: impl FnOnce(&mut Book) -> Result<T, Error>) -> Result<T, Error> {
        self.update(|book| {
            let book = book.as_mut().ok_or(Error::NotFound)?;
            if book.snapshot.scope != self.scope || book.authority != self.authority {
                return Err(Error::InvalidTransition);
            }
            apply(book)
        })
    }

    fn update<T>(
        &self,
        apply: impl FnOnce(&mut Option<Book>) -> Result<T, Error>,
    ) -> Result<T, Error> {
        update(&self.store, &self.scope.task, apply)
    }
}

/// Answer only on the local task owner's existing authority boundary.
pub fn reply(store: &Path, task: &str, reply: &Reply) -> Result<Receipt, Error> {
    Store::open(store)?.show(task)?;
    if reply.scope.task != task {
        return Err(Error::InvalidTransition);
    }
    let authority = update(store, task, |book| {
        Ok(book.as_ref().ok_or(Error::NotFound)?.authority.clone())
    })?;
    with_current(store, &reply.scope, &authority, || {
        update(store, task, |book| {
            let book = book.as_mut().ok_or(Error::NotFound)?;
            if book.authority != authority {
                return Err(Error::InvalidTransition);
            }
            book.reply(reply, super::autostart::unix_now())
        })
    })
}

fn current(store: &Path, scope: &Scope, authority: &Authority) -> Result<(), Error> {
    with_current(store, scope, authority, || Ok(()))
}

fn with_current<T>(
    store: &Path,
    scope: &Scope,
    authority: &Authority,
    apply: impl FnOnce() -> Result<T, Error>,
) -> Result<T, Error> {
    let store = Store::open(store)?;
    let task = store.show(&scope.task)?;
    let run = task.run.as_ref().ok_or(Error::InvalidTransition)?;
    if task.status != Status::Running
        || run.result.is_some()
        || task.turn() != scope.turn
        || task.intent_digest != authority.intent
        || run.admission.grant_digest != authority.grant
        || run.admission.workspace.to_string_lossy() != authority.checkout
        || target_identity(&run.admission.workspace)? != authority.target
        || scope.host != super::local::LOCAL_HOST
    {
        return Err(Error::InvalidTransition);
    }
    match owner::Owner::acquire(&store, &scope.task) {
        Err(Error::Busy) => apply(),
        _ => Err(Error::InvalidTransition),
    }
}

pub(crate) fn target_identity(path: &Path) -> Result<String, Error> {
    let metadata = std::fs::symlink_metadata(path)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(Error::UnsafePath);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        Ok(format!("{}:{}", metadata.dev(), metadata.ino()))
    }
    #[cfg(windows)]
    {
        let (identity, _) = private_fs::identity_of(path)?;
        Ok(format!("{}:{}", identity.volume, identity.index))
    }
    #[cfg(not(any(unix, windows)))]
    {
        Err(Error::UnsupportedSchema)
    }
}

fn directory(store: &Path, task: &str) -> Result<PathBuf, Error> {
    let root = store.join("pending");
    super::prepare_directory(&root)?;
    let digest = route_contract::Digest::of_bytes(task.as_bytes());
    let dir = root.join(digest.as_str().trim_start_matches("sha256:"));
    super::prepare_directory(&dir)?;
    Ok(dir)
}

fn update<T>(
    store: &Path,
    task: &str,
    apply: impl FnOnce(&mut Option<Book>) -> Result<T, Error>,
) -> Result<T, Error> {
    let dir = directory(store, task)?;
    let lock_path = dir.join("book.lock");
    let lock = super::open_lock(&lock_path)?;
    super::take_lock(&lock, Duration::from_secs(1))?;
    super::verify_same_file(&lock_path, &lock)?;
    let path = dir.join("book.json");
    let mut original = None;
    let mut book = if super::regular_or_absent(&path)? {
        let mut bytes = Vec::new();
        super::private_open(&path, false, false)?
            .take(MAX_BYTES + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() as u64 > MAX_BYTES {
            return Err(Error::LimitExceeded);
        }
        original = Some(bytes.clone());
        let book: Book = serde_json::from_slice(&bytes)
            .map_err(|_| Error::Corrupt("the pending requests could not be read"))?;
        if book.schema != SCHEMA || !book.snapshot.valid() {
            return Err(Error::UnsupportedSchema);
        }
        Some(book)
    } else {
        None
    };
    // Save even on refusal: invalidation and expiry must survive the caller.
    let result = apply(&mut book);
    if let Some(book) = &book {
        let bytes = serde_json::to_vec(book).map_err(|_| Error::UnsupportedSchema)?;
        if bytes.len() as u64 > MAX_BYTES {
            return Err(Error::LimitExceeded);
        }
        if original.as_deref() != Some(bytes.as_slice()) {
            let temporary = dir.join("book.next");
            super::write_new(&temporary, &bytes)?;
            super::regular_or_absent(&path)?;
            std::fs::rename(temporary, path)?;
            super::sync_directory(&dir)?;
        }
    }
    result
}

/// Read a current owner's requests and renew this native-capable viewer.
pub fn view(store: &Path, task: &str) -> Result<Option<Snapshot>, Error> {
    Store::open(store)?.show(task)?;
    let bound = update(store, task, |book| {
        Ok(book
            .as_ref()
            .map(|book| (book.snapshot.scope.clone(), book.authority.clone())))
    })?;
    let Some((scope, authority)) = bound else {
        return Ok(None);
    };
    let service = Service {
        store: store.into(),
        scope,
        authority,
    };
    if service.current().is_err() {
        service.close()?;
    } else {
        observe(store, task, super::autostart::unix_now())?;
    }
    service.snapshot().map(Some)
}

/// Only a client that implements native replies renews this lease.
pub(crate) fn observe(store: &Path, task: &str, now: u64) -> Result<(), Error> {
    let dir = directory(store, task)?;
    let lock = super::open_lock(&dir.join("viewer.lock"))?;
    super::take_lock(&lock, Duration::from_secs(1))?;
    let temporary = dir.join("viewer.next");
    super::write_new(
        &temporary,
        now.saturating_add(VIEWER_TTL).to_string().as_bytes(),
    )?;
    let path = dir.join("viewer");
    super::regular_or_absent(&path)?;
    std::fs::rename(temporary, path)?;
    super::sync_directory(&dir)
}

fn viewer(store: &Path, task: &str, now: u64) -> Result<bool, Error> {
    let path = directory(store, task)?.join("viewer");
    if !super::regular_or_absent(&path)? {
        return Ok(false);
    }
    let mut bytes = String::new();
    super::private_open(&path, false, false)?
        .take(32)
        .read_to_string(&mut bytes)?;
    Ok(bytes
        .parse::<u64>()
        .is_ok_and(|until| until > now && until <= now.saturating_add(VIEWER_TTL)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn request(generation: u64) -> Request {
        let mut request = Request {
            schema: wire::SCHEMA.into(),
            id: digest_of(&()),
            scope: Scope {
                host: "local".into(),
                task: "task".into(),
                turn: 1,
                generation,
            },
            authority: Authority {
                intent: "intent".into(),
                grant: "grant".into(),
                checkout: "/checkout".into(),
                target: "target".into(),
            },
            native_call: "call".into(),
            child: None,
            tool: "Bash".into(),
            input: json!({"command":"make check"}),
            input_digest: digest_of(&()),
            prompt: Prompt {
                command: "make check".into(),
                reason: String::new(),
                risk: wire::Risk::High,
            },
            form: Form::Approval,
            expires_at: 110,
        };
        request.seal();
        request
    }
    fn answer(request: &Request, answer: Answer) -> Reply {
        Reply {
            request: request.id.clone(),
            scope: request.scope.clone(),
            input_digest: request.input_digest.clone(),
            answer,
        }
    }

    #[test]
    fn expired_answer_and_close_cannot_be_consumed_or_revived() {
        let request = request(1);
        let mut book = Book::new(request.scope.clone(), request.authority.clone());
        book.open(request.clone(), 100).unwrap();
        book.reply(&answer(&request, Answer::AllowOnce), 101)
            .unwrap();
        assert!(book.consume(&request, 110).is_err());
        assert_eq!(
            book.snapshot.entries[0].resolution,
            Some(Resolution::Expired)
        );
        assert!(
            book.reply(&answer(&request, Answer::AllowOnce), 111)
                .is_err()
        );
        book.close(None, Resolution::Cancelled);
        let revision = book.snapshot.revision;
        book.close(None, Resolution::Cancelled);
        assert_eq!(book.snapshot.revision, revision);
        assert!(book.snapshot.valid());
        assert!(book.open(request, 100).is_err());
    }

    #[test]
    fn a_native_viewer_lease_expires_without_becoming_permission() {
        let root = tempfile::tempdir().unwrap();
        assert!(!viewer(root.path(), "task", 100).unwrap());
        observe(root.path(), "task", 100).unwrap();
        assert!(viewer(root.path(), "task", 101).unwrap());
        assert!(!viewer(root.path(), "task", 111).unwrap());
        assert!(!viewer(root.path(), "another-task", 101).unwrap());
    }

    #[test]
    fn records_survive_reopen_and_wrong_scope_or_final_input_cannot_resolve_them() {
        let root = tempfile::tempdir().unwrap();
        let request = request(1);
        update(root.path(), "task", |book| {
            let mut created = Book::new(request.scope.clone(), request.authority.clone());
            created.open(request.clone(), 100)?;
            *book = Some(created);
            Ok(())
        })
        .unwrap();
        update(root.path(), "task", |book| {
            book.as_mut()
                .unwrap()
                .reply(&answer(&request, Answer::AllowOnce), 101)?;
            Ok(())
        })
        .unwrap();
        update(root.path(), "task", |book| {
            let book = book.as_mut().unwrap();
            let mut stale = answer(&request, Answer::AllowOnce);
            stale.scope.generation += 1;
            assert!(book.reply(&stale, 102).is_err());
            let mut changed = request.clone();
            changed.input = json!({"command":"deploy"});
            changed.seal();
            assert!(book.consume(&changed, 102).is_err());
            assert_eq!(book.consume(&request, 102)?, Some(Answer::AllowOnce));
            assert!(book.consume(&request, 102).is_err());
            assert!(
                book.reply(&answer(&request, Answer::AllowOnce), 102)?
                    .duplicate
            );
            Ok(())
        })
        .unwrap();
    }
}
