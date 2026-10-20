//! One bounded workspace reader for local viewers and admitted SDK tools.

use super::{Error, Store, local, pending};
use route_contract::digest_of;
pub use route_contract::workspace::*;
use std::path::{Path, PathBuf};

#[derive(Clone)]
pub struct Service {
    store: PathBuf,
    reference: Reference,
    agent: Option<pending::Service>,
}
impl Service {
    pub fn supported() -> bool {
        cfg!(unix)
    }
    pub fn reference(&self) -> &Reference {
        &self.reference
    }
    pub fn for_agent(store: &Path, agent: pending::Service) -> Self {
        let scope = agent.scope();
        let mut reference = Reference {
            schema: SCHEMA.into(),
            id: digest_of(&()),
            host: scope.host.clone(),
            task: scope.task.clone(),
            turn: scope.turn,
            authority: agent.authority().clone(),
        };
        reference.seal();
        Self {
            store: store.into(),
            reference,
            agent: Some(agent),
        }
    }
    pub fn for_viewer(store: &Path, task: &str) -> Result<Self, Error> {
        // Local ownership is established by the existing local-run record.
        let record = local::record(store, task).ok_or(Error::NotFound)?;
        let task = Store::open(store)?.show(task)?;
        let run = task.run.as_ref().ok_or(Error::InvalidTransition)?;
        if Path::new(&record.worktree).canonicalize().ok().as_deref()
            != Some(run.admission.workspace.as_path())
        {
            return Err(Error::UnsafePath);
        }
        let mut reference = Reference {
            schema: SCHEMA.into(),
            id: digest_of(&()),
            host: local::LOCAL_HOST.into(),
            task: task.task_id.clone(),
            turn: task.turn(),
            authority: route_contract::pending::Authority {
                intent: task.intent_digest.clone(),
                grant: run.admission.grant_digest.clone(),
                checkout: run.admission.workspace.display().to_string(),
                target: record
                    .workspace_target
                    .clone()
                    .ok_or(Error::InvalidTransition)?,
            },
        };
        if pending::target_identity(&run.admission.workspace)? != reference.authority.target {
            return Err(Error::UnsafePath);
        }
        reference.seal();
        Ok(Self {
            store: store.into(),
            reference,
            agent: None,
        })
    }
    pub fn read(&self, request: &Request) -> Result<Output, Error> {
        if !Self::supported()
            || !request.reference.valid()
            || request.reference != self.reference
            || request.reference.host != local::LOCAL_HOST
            || !request.operation.valid()
        {
            return Err(Error::UnsafePath);
        }
        if let Some(agent) = &self.agent {
            agent.with_active(|| self.perform(request))
        } else {
            // Keep the task locked until the bounded read finishes: a next turn
            // cannot change the grant while a viewer reads this one.
            let store = Store::open(&self.store)?;
            let task = store.show(&self.reference.task)?;
            let run = task.run.as_ref().ok_or(Error::InvalidTransition)?;
            let authority = &self.reference.authority;
            if task.turn() != self.reference.turn
                || task.intent_digest != authority.intent
                || run.admission.grant_digest != authority.grant
                || run.admission.workspace.to_string_lossy() != authority.checkout
            {
                return Err(Error::InvalidTransition);
            }
            self.perform(request)
        }
    }
    #[cfg(unix)]
    fn perform(&self, request: &Request) -> Result<Output, Error> {
        unix::perform(request, self.agent.is_some())
    }
    #[cfg(not(unix))]
    fn perform(&self, _request: &Request) -> Result<Output, Error> {
        Err(Error::UnsupportedSchema)
    }
}
impl coder_sdk::workspace::Service for Service {
    fn execute(&self, request: &Request) -> Result<Output, String> {
        self.read(request).map_err(|error| match error {
            Error::UnsupportedSchema => "This file isn't readable text. Choose another file.".into(),
            _ => "This workspace changed or cannot be read. Refresh it and choose a file inside the task's folder.".into(),
        })
    }
}

impl local::Local {
    pub fn workspace_reference(&self, task: &str) -> Result<Reference, String> {
        if !Service::supported() {
            return Err("Workspace browsing is not available on this computer.".into());
        }
        Service::for_viewer(self.store(), task)
            .map(|service| service.reference)
            .map_err(|_| "This task's workspace is unavailable.".into())
    }
    pub fn read_workspace(&self, task: &str, request: &Request) -> Result<Output, String> {
        if request.reference.task != task {
            return Err("This file belongs to a different task.".into());
        }
        use coder_sdk::workspace::Service as _;
        let service = Service::for_viewer(self.store(), task)
            .map_err(|_| "This task's workspace is unavailable.")?;
        service.execute(request)
    }
}

#[cfg(unix)]
mod unix {
    use super::*;
    use std::fs::{File, OpenOptions};
    use std::io::Read;
    use std::os::fd::{AsRawFd, FromRawFd};
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
    use std::time::{Duration, Instant};
    const SCAN: usize = 4096;
    const BYTES: usize = 8 * 1024 * 1024;

    fn directory(root: &File, path: &Path) -> std::io::Result<File> {
        let mut dir = root.try_clone()?;
        for component in path.components() {
            let std::path::Component::Normal(name) = component else {
                return Err(std::io::Error::other("invalid workspace path"));
            };
            let name = std::ffi::CString::new(name.as_bytes())?;
            // SAFETY: the directory handle is live; name is one NUL-terminated
            // component. Links are refused, and the returned handle is owned.
            let fd = unsafe {
                libc::openat(
                    dir.as_raw_fd(),
                    name.as_ptr(),
                    libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                )
            };
            if fd < 0 {
                return Err(std::io::Error::last_os_error());
            }
            // SAFETY: openat returned a new descriptor owned by this File.
            dir = unsafe { File::from_raw_fd(fd) };
        }
        Ok(dir)
    }
    fn names(
        root: &File,
        path: &str,
        limit: usize,
        began: Instant,
    ) -> std::io::Result<(Vec<(String, Kind)>, bool, usize)> {
        let dir = directory(root, Path::new(path))?;
        // /dev/fd refers to this opened directory on macOS and Linux. All
        // descendant opens use handles; a renamed parent cannot redirect them.
        let entries = std::fs::read_dir(format!("/dev/fd/{}", dir.as_raw_fd()))?;
        let mut out = Vec::new();
        let mut truncated = false;
        let mut visited = 0;
        for (index, entry) in entries.enumerate() {
            if index >= limit || began.elapsed() > Duration::from_millis(500) {
                truncated = true;
                break;
            }
            visited += 1;
            let entry = entry?;
            let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            if name == ".git" || name.chars().any(char::is_control) {
                continue;
            }
            let kind = entry.file_type()?;
            let kind = if kind.is_symlink() {
                continue;
            } else if kind.is_dir() {
                Kind::Directory
            } else if kind.is_file() {
                Kind::File
            } else {
                continue;
            };
            let path = if path.is_empty() {
                name
            } else {
                format!("{path}/{name}")
            };
            if path.len() <= MAX_PATH && (Operation::Read { path: path.clone() }).valid() {
                out.push((path, kind));
            } else {
                truncated = true;
            }
        }
        out.sort_by(|a, b| a.0.cmp(&b.0));
        Ok((out, truncated, visited))
    }
    fn bytes(root: &File, path: &str, limit: usize) -> std::io::Result<(Vec<u8>, bool)> {
        let file = super::super::artifact::confined_at(root.try_clone()?, Path::new(path))?;
        let mut bytes = Vec::new();
        file.take(limit as u64 + 1).read_to_end(&mut bytes)?;
        let truncated = bytes.len() > limit;
        bytes.truncate(limit);
        Ok((bytes, truncated))
    }
    fn text(bytes: Vec<u8>, truncated: bool) -> Result<String, Error> {
        if bytes.contains(&0) {
            return Err(Error::UnsupportedSchema);
        }
        match String::from_utf8(bytes) {
            Ok(text) => Ok(text),
            Err(error) if truncated && error.utf8_error().error_len().is_none() => Ok(
                String::from_utf8(error.as_bytes()[..error.utf8_error().valid_up_to()].to_vec())
                    .map_err(|_| Error::UnsupportedSchema)?,
            ),
            Err(_) => Err(Error::UnsupportedSchema),
        }
    }
    fn snippet(value: &str) -> String {
        let mut end = value.len().min(1024);
        while !value.is_char_boundary(end) {
            end -= 1;
        }
        value[..end].into()
    }
    fn denied(request: &Request, path: &str) -> bool {
        let path = Path::new(&request.reference.authority.checkout).join(path);
        coder_sdk::floors::classify("Read", &serde_json::json!({"file_path":path})).is_some()
    }
    pub(super) fn perform(request: &Request, agent: bool) -> Result<Output, Error> {
        if agent && denied(request, request.operation.path()) {
            return Err(Error::UnsafePath);
        }
        let root = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(&request.reference.authority.checkout)?;
        let meta = root.metadata()?;
        if format!("{}:{}", meta.dev(), meta.ino()) != request.reference.authority.target {
            return Err(Error::UnsafePath);
        }
        let mut output = Output {
            reference: request.reference.id.clone(),
            operation: request.operation.id(),
            rows: Vec::new(),
            text: None,
            content: None,
            truncated: false,
        };
        match &request.operation {
            Operation::Read { path } => {
                let (bytes, truncated) = bytes(&root, path, MAX_TEXT)?;
                let text = text(bytes, truncated)?;
                output.content = Some(route_contract::Digest::of_bytes(text.as_bytes()));
                output.text = Some(text);
                output.truncated = truncated;
            }
            Operation::List { path } => {
                let (rows, truncated, _) = names(&root, path, SCAN, Instant::now())?;
                output.truncated = truncated || rows.len() > MAX_ROWS;
                output.rows = rows
                    .into_iter()
                    .take(MAX_ROWS)
                    .map(|(path, kind)| Row::new(path, kind, None, String::new()))
                    .collect();
            }
            Operation::Search { path, query } => {
                let began = Instant::now();
                let mut folders = std::collections::VecDeque::from([(path.clone(), 0)]);
                let mut scanned = 0;
                let mut read = 0;
                'folders: while let Some((folder, depth)) = folders.pop_front() {
                    if began.elapsed() > Duration::from_millis(500)
                        || scanned >= SCAN
                        || read >= BYTES
                    {
                        output.truncated = true;
                        break;
                    }
                    let (entries, truncated, visited) =
                        names(&root, &folder, SCAN - scanned, began)?;
                    scanned += visited;
                    output.truncated |= truncated;
                    for (path, kind) in entries {
                        if read >= BYTES || began.elapsed() > Duration::from_millis(500) {
                            output.truncated = true;
                            break 'folders;
                        }
                        if kind == Kind::Directory {
                            if depth < 16 {
                                folders.push_back((path, depth + 1));
                            } else {
                                output.truncated = true;
                            }
                            continue;
                        }
                        if agent && denied(request, &path) {
                            continue;
                        }
                        let (bytes, truncated) =
                            match bytes(&root, &path, (BYTES - read).min(MAX_TEXT)) {
                                Ok(value) => value,
                                Err(_) => {
                                    output.truncated = true;
                                    continue;
                                }
                            };
                        read += bytes.len();
                        output.truncated |= truncated;
                        let Ok(text) = text(bytes, truncated) else {
                            continue;
                        };
                        for (line, value) in text.lines().enumerate() {
                            if value.contains(query) {
                                if output.rows.len() == MAX_ROWS {
                                    output.truncated = true;
                                    break 'folders;
                                }
                                output.rows.push(Row::new(
                                    path.clone(),
                                    Kind::Match,
                                    Some(line + 1),
                                    snippet(value),
                                ));
                            }
                        }
                    }
                }
            }
        }
        Ok(output)
    }
}
