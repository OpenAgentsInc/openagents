//! Explicit repository snapshots and retained remote result artifacts.
use crate::{Lease, Record, Result};
use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Read,
    path::{Component, Path, PathBuf},
    process::{Command, Stdio},
};
const MAX_INPUT: usize = 128 * 1024 * 1024;
const MAX_TREE: u64 = 512 * 1024 * 1024;
pub fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Snapshot {
    pub revision: String,
    pub source_root: PathBuf,
    pub caller_revision: String,
    pub working_directory: PathBuf,
    pub paths: Vec<String>,
    pub included: Vec<String>,
    pub input_digest: String,
    pub caller_changes_digest: String,
    pub input_path: PathBuf,
}
fn git(root: &Path, args: &[&str]) -> Result<Vec<u8>> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .args(args)
        .stdin(Stdio::null())
        .output()
        .map_err(|_| "Cannot start Git for workspace transfer.")?;
    if !output.status.success() {
        return Err("Git refused the workspace operation.".into());
    }
    if output.stdout.len() > MAX_INPUT {
        return Err("The workspace operation exceeds its transfer limit.".into());
    }
    Ok(output.stdout)
}
pub fn validate_path(name: &str) -> Result<()> {
    if name.is_empty()
        || name.contains('\0')
        || !Path::new(name)
            .components()
            .all(|c| matches!(c, Component::Normal(_)))
    {
        return Err(
            "Workspace paths must be relative and cannot contain parent components.".into(),
        );
    }
    if Path::new(name).components().any(|p| {
        matches!(
            p.as_os_str().to_str(),
            Some(".git" | ".env" | "auth.json" | "credentials.json" | "google-services.json")
        )
    }) {
        return Err("Credential files and Git control files cannot be transferred.".into());
    }
    Ok(())
}
fn included(root: &Path, names: &[String]) -> Result<Vec<Value>> {
    let mut files = vec![];
    for name in names {
        validate_path(name)?;
        let mut current = root.to_path_buf();
        for part in Path::new(name).components() {
            current.push(part);
            if fs::symlink_metadata(&current)
                .map_err(|_| "Cannot inspect an admitted workspace file.")?
                .file_type()
                .is_symlink()
            {
                return Err("Admitted workspace files cannot traverse symlinks.".into());
            }
        }
        if !fs::metadata(&current)
            .map_err(|_| "Cannot inspect an admitted workspace file.")?
            .is_file()
        {
            return Err("Use --include for individual regular files.".into());
        }
        let bytes = read_bounded(&current, 16 * 1024 * 1024)?;
        #[cfg(unix)]
        let executable = {
            use std::os::unix::fs::PermissionsExt;
            fs::metadata(&current).unwrap().permissions().mode() & 0o111 != 0
        };
        #[cfg(not(unix))]
        let executable = false;
        files.push(json!({"path":name,"content":base64::engine::general_purpose::STANDARD.encode(bytes),"executable":executable}));
    }
    Ok(files)
}
fn changes(
    root: &Path,
    revision: &str,
    paths: &[String],
    names: &[String],
) -> Result<(Vec<u8>, Vec<Value>, String)> {
    let mut args = vec![
        "diff",
        "--no-ext-diff",
        "--no-textconv",
        "--binary",
        revision,
        "--",
    ];
    args.extend(paths.iter().map(String::as_str));
    let patch = git(root, &args)?;
    let files = included(root, names)?;
    let fingerprint=digest(&serde_json::to_vec(&json!({"patch":base64::engine::general_purpose::STANDARD.encode(&patch),"included":files})).unwrap());
    Ok((patch, files, fingerprint))
}
/// Read the exact configured source revision and selected local changes.
/// This does not create a cloud job, transfer files, or write repository state.
pub fn source_identity(
    cwd: &Path,
    revision: &str,
    paths: &[String],
    names: &[String],
) -> Result<String> {
    if revision.len() != 40 || !revision.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("The operator source must pin an exact Git commit.".into());
    }
    let root = String::from_utf8(git(cwd, &["rev-parse", "--show-toplevel"])?)
        .map_err(|_| "Invalid operator source.")?;
    let root = Path::new(root.trim());
    let head = String::from_utf8(git(root, &["rev-parse", "HEAD"])?)
        .map_err(|_| "Invalid operator source revision.")?;
    if head.trim() != revision {
        return Err("The operator source revision changed.".into());
    }
    let (_, _, changes) = changes(root, revision, paths, names)?;
    Ok(format!(
        "sha256:{}",
        digest(
            &serde_json::to_vec(&json!({"revision":revision,"changes":changes}))
                .map_err(|_| "Cannot encode operator source identity.")?
        )
    ))
}
pub fn capture(
    lease: &Lease,
    cwd: &Path,
    revision: Option<&str>,
    paths: Vec<String>,
    names: Vec<String>,
) -> Result<Snapshot> {
    for p in paths.iter().chain(&names) {
        validate_path(p)?;
    }
    let root = String::from_utf8(git(cwd, &["rev-parse", "--show-toplevel"])?)
        .map_err(|_| "Invalid repository path.")?;
    let root = PathBuf::from(root.trim());
    let cwd = fs::canonicalize(cwd).map_err(|_| "Cannot resolve the workspace directory.")?;
    let relative = cwd
        .strip_prefix(&root)
        .map_err(|_| "The workspace is outside its repository.")?
        .to_path_buf();
    let revision = revision.unwrap_or("HEAD");
    if revision.starts_with('-')
        || !revision
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"/_.-".contains(&c))
    {
        return Err("Invalid workspace revision.".into());
    }
    let revision = String::from_utf8(git(
        &root,
        &["rev-parse", "--verify", &format!("{revision}^{{commit}}")],
    )?)
    .map_err(|_| "Invalid workspace revision.")?
    .trim()
    .to_string();
    let mut args = vec!["ls-tree", "-rlz", &revision, "--"];
    args.extend(paths.iter().map(String::as_str));
    let listing = git(&root, &args)?;
    let mut size = 0u64;
    let mut count = 0;
    for row in listing.split(|b| *b == 0).filter(|r| !r.is_empty()) {
        let row = std::str::from_utf8(row).map_err(|_| "Workspace paths must be UTF-8.")?;
        let (meta, name) = row.split_once('\t').ok_or("Invalid Git tree listing.")?;
        validate_path(name)?;
        let fields: Vec<_> = meta.split_whitespace().collect();
        if fields.get(1) != Some(&"blob") {
            return Err("Admit submodule contents separately before cloud transfer.".into());
        }
        size += fields
            .get(3)
            .and_then(|s| s.parse::<u64>().ok())
            .ok_or("Invalid Git object size.")?;
        count += 1;
    }
    if count == 0 {
        return Err("The workspace selection contains no repository files.".into());
    }
    if size > MAX_TREE || count > 30000 {
        return Err("The repository is too large for an interactive transfer. Use --workspace-path PATH to select the task's files.".into());
    }
    let (patch, files, fingerprint) = changes(&root, &revision, &paths, &names)?;
    let mut args = vec!["archive", "--format=tar.gz", &revision, "--"];
    args.extend(paths.iter().map(String::as_str));
    let archive = git(&root, &args)?;
    let input = json!({"schema":"openagents.coder.workspace.v1","revision":revision,"archive":base64::engine::general_purpose::STANDARD.encode(archive),"patch":base64::engine::general_purpose::STANDARD.encode(patch),"included":files});
    let input = serde_json::to_vec(&input).map_err(|_| "Cannot encode the workspace input.")?;
    if input.len() > MAX_INPUT {
        return Err("The workspace input exceeds 128 MiB. Select fewer paths.".into());
    }
    let input_path = lease.file("input.json")?;
    write_private(&input_path, &input)?;
    Ok(Snapshot {
        revision,
        caller_revision: String::from_utf8(git(&root, &["rev-parse", "HEAD"])?)
            .map_err(|_| "Invalid caller revision.")?
            .trim()
            .into(),
        source_root: root,
        working_directory: relative,
        paths,
        included: names,
        input_digest: digest(&input),
        caller_changes_digest: fingerprint,
        input_path,
    })
}
impl Snapshot {
    pub fn input(&self) -> Result<Vec<u8>> {
        let bytes = read_bounded(&self.input_path, MAX_INPUT)?;
        if digest(&bytes) != self.input_digest {
            return Err("The retained workspace input has changed.".into());
        }
        Ok(bytes)
    }
    pub fn verify_caller(&self, cwd: &Path) -> Result<()> {
        let root = String::from_utf8(git(cwd, &["rev-parse", "--show-toplevel"])?)
            .map_err(|_| "Invalid repository path.")?;
        if Path::new(root.trim()) != self.source_root {
            return Err("The result belongs to another repository.".into());
        }
        let head = String::from_utf8(git(&self.source_root, &["rev-parse", "HEAD"])?)
            .map_err(|_| "Invalid repository revision.")?;
        if head.trim() != self.caller_revision {
            return Err(
                "The caller revision changed. Review the patch before applying it manually.".into(),
            );
        }
        let (_, _, fingerprint) = changes(
            &self.source_root,
            &self.revision,
            &self.paths,
            &self.included,
        )?;
        if fingerprint != self.caller_changes_digest {
            return Err("The caller's admitted files changed. Review the patch before applying it manually.".into());
        }
        Ok(())
    }
}
pub fn read_bounded(path: &Path, limit: usize) -> Result<Vec<u8>> {
    crate::regular_or_missing(path)?;
    let mut bytes = vec![];
    fs::File::open(path)
        .and_then(|f| f.take(limit as u64 + 1).read_to_end(&mut bytes))
        .map_err(|_| "Cannot read a retained workspace file.")?;
    if bytes.len() > limit {
        return Err("A workspace artifact exceeds its size limit.".into());
    }
    Ok(bytes)
}
fn write_private(path: &Path, bytes: &[u8]) -> Result<()> {
    use std::io::Write;
    crate::regular_or_missing(path)?;
    let mut f = crate::private_options()
        .write(true)
        .create(true)
        .truncate(true)
        .open(path)
        .map_err(|_| "Cannot create a private workspace file.")?;
    f.write_all(bytes)
        .and_then(|_| f.sync_all())
        .map_err(|_| "Cannot retain a workspace file.".into())
}
/// The extraction command checks identity before touching an existing workspace.
pub fn restore_script(r: &Record, dir: &str) -> Result<String> {
    let s = r.workspace.as_ref().ok_or("Missing workspace input.")?;
    Ok(format!(
        r#"python3 - {dir} {digest} {revision} <<'PY'
import pathlib,json,hashlib,base64,tarfile,io,os,posixpath,subprocess
p=pathlib.Path(__import__('sys').argv[1]);expected=__import__('sys').argv[2];revision=__import__('sys').argv[3]
raw=(p/'input.json').read_bytes()
if hashlib.sha256(raw).hexdigest()!=expected:raise SystemExit('Workspace digest mismatch')
v=json.loads(raw)
if v['schema']!='openagents.coder.workspace.v1' or v['revision']!=revision:raise SystemExit('Workspace revision mismatch')
marker=p/'workspace-input'
if marker.exists():
 if marker.read_text()!=expected:raise SystemExit('Existing workspace identity mismatch')
 raise SystemExit(0)
w=p/'workspace';w.mkdir(exist_ok=True)
def safe(name):
 parts=pathlib.PurePosixPath(name).parts
 if not parts or name.startswith('/') or '..' in parts or '.git' in parts:raise SystemExit('Unsafe workspace path')
 q=w.joinpath(*parts)
 if any(x.is_symlink() for x in [q,*q.parents] if x!=w):raise SystemExit('Workspace path traverses a symlink')
 return q
archive=tarfile.open(fileobj=io.BytesIO(base64.b64decode(v['archive'])),mode='r:gz');total=0
for m in archive:
 q=safe(m.name);total+=m.size
 if total>512*1024*1024:raise SystemExit('Workspace exceeds its size limit')
 if m.isdir():q.mkdir(parents=True,exist_ok=True)
 elif m.isfile():
  q.parent.mkdir(parents=True,exist_ok=True)
  with q.open('wb') as f:f.write(archive.extractfile(m).read())
  q.chmod(m.mode&0o777)
 elif m.issym():
  if m.linkname.startswith('/') or posixpath.normpath(posixpath.join(posixpath.dirname(m.name),m.linkname)).startswith('../'):raise SystemExit('Unsafe workspace symlink')
  q.parent.mkdir(parents=True,exist_ok=True);q.symlink_to(m.linkname)
 else:raise SystemExit('Unsupported workspace archive member')
def git(*args,**kw):return subprocess.run(['git','-C',str(w),'-c','core.hooksPath=/dev/null','-c','user.name=Coder','-c','user.email=coder@localhost','-c','commit.gpgsign=false',*args],check=True,stdout=subprocess.DEVNULL,**kw)
git('init','-q')
patch=base64.b64decode(v['patch'])
if patch:git('apply','--binary','-',input=patch)
for f in v['included']:
 q=safe(f['path']);q.parent.mkdir(parents=True,exist_ok=True);q.write_bytes(base64.b64decode(f['content']));q.chmod(0o755 if f['executable'] else 0o644)
git('add','-f','--all');git('commit','-q','--allow-empty','-m','Cloud workspace input')
marker.write_text(expected)
PY"#,
        dir = boat::shell_quote(dir),
        digest = boat::shell_quote(&s.input_digest),
        revision = boat::shell_quote(&s.revision)
    ))
}
pub fn collect_script(r: &Record, dir: &str) -> String {
    format!(
        r#"python3 - {dir} {job} <<'PY'
import pathlib,base64,json,subprocess
p=pathlib.Path(__import__('sys').argv[1]);job=__import__('sys').argv[2];w=p/'workspace'
v={{'patch':'','files':[],'trace':None}}
if (p/'workspace-input').exists():
 def git(*a):return subprocess.run(['git','-C',str(w),'-c','core.hooksPath=/dev/null',*a],check=True,capture_output=True).stdout
 git('add','--intent-to-add','--all')
 names=git('diff','--name-only','-z','HEAD').split(b'\0')
 total=0
 for n in filter(None,names):
  name=n.decode();q=w/name
  if any(x in {{'.git','.env','auth.json','credentials.json','google-services.json'}} for x in pathlib.PurePosixPath(name).parts):raise SystemExit('Credential artifact refused')
  if q.is_file() and not q.is_symlink():
   data=q.read_bytes();total+=len(data)
   if total>16*1024*1024:raise SystemExit('Changed files exceed artifact limit')
   v['files'].append({{'path':name,'content':base64.b64encode(data).decode()}})
 patch=git('diff','--binary','HEAD');v['patch']=base64.b64encode(patch).decode()
trace=pathlib.Path('/tmp')/('oa-coder-'+job)/'state'/'sessions'/(job+'.atif.json')
if trace.exists() and trace.stat().st_size<8*1024*1024:v['trace']=json.loads(trace.read_text())
print(json.dumps(v))
PY"#,
        dir = boat::shell_quote(dir),
        job = boat::shell_quote(&r.id)
    )
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Artifacts {
    pub files: Vec<Artifact>,
    pub revision: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Artifact {
    pub name: String,
    pub bytes: usize,
    pub digest: String,
}
pub fn retain(lease: &Lease, r: &Record, payload: Option<Value>) -> Result<Artifacts> {
    let payload = payload.unwrap_or_else(|| json!({"patch":""}));
    let patch = base64::engine::general_purpose::STANDARD
        .decode(payload["patch"].as_str().unwrap_or(""))
        .map_err(|_| "Invalid patch encoding.")?;
    let mut session = atif::Session::opening(
        &r.id,
        r.result
            .as_ref()
            .and_then(|v| v["model"].as_str())
            .or(r.spec.model.as_deref())
            .unwrap_or(&r.spec.agent),
        "coder-new",
        &r.spec.cwd.to_string_lossy(),
        env!("CARGO_PKG_VERSION"),
    );
    session.state = "ended".into();
    let first = r
        .turns
        .first()
        .and_then(|v| v["task"].as_str())
        .unwrap_or(&r.spec.task);
    let mut steps = vec![atif::Step::said(atif::Source::User, first)];
    for (index, event) in r.events.iter().enumerate() {
        if event["event"] == "tool" {
            steps.push(atif::Step::called(atif::Call {
                id: format!("remote-tool-{index}"),
                name: event["name"].as_str().unwrap_or("unknown").into(),
                arguments: event["input"].clone(),
                output: event["output"].to_string(),
                outcome: if event["running"] == true {
                    atif::Outcome::Cancelled
                } else if event["output"].get("error").is_some() {
                    atif::Outcome::Failed
                } else {
                    atif::Outcome::Completed
                },
                milliseconds: 0,
                purpose: None,
                extra: Default::default(),
            }));
        }
        if let Some(text) = event["text"].as_str() {
            steps.push(atif::Step::said(
                if event["event"] == "user" {
                    atif::Source::User
                } else {
                    atif::Source::Agent
                },
                text,
            ));
        }
    }
    let mut trace = payload
        .get("trace")
        .filter(|v| !v.is_null())
        .cloned()
        .unwrap_or_else(|| atif::document(&session, &steps));
    atif::upgrade(&mut trace).map_err(|_| "Unsupported remote ATIF transcript.")?;
    let mut events = vec![];
    for e in &r.events {
        serde_json::to_writer(&mut events, e).unwrap();
        events.push(b'\n');
    }
    let files=[("changes.patch",patch),("events.ndjson",events),("trajectory.atif.json",serde_json::to_vec(&trace).unwrap()),("result.json",serde_json::to_vec(&json!({"job":r.id,"state":r.state,"result":r.result,"error":r.error,"workspace":r.workspace})).unwrap())];
    let mut manifest = Artifacts {
        files: vec![],
        revision: r.workspace.as_ref().map(|s| s.revision.clone()),
    };
    for (name, bytes) in files {
        if bytes.len() > 32 * 1024 * 1024 {
            return Err("A retained result artifact exceeds 32 MiB.".into());
        }
        write_private(&lease.file(name)?, &bytes)?;
        manifest.files.push(Artifact {
            name: name.into(),
            bytes: bytes.len(),
            digest: digest(&bytes),
        });
    }
    write_private(
        &lease.file("manifest.json")?,
        &serde_json::to_vec(&manifest).unwrap(),
    )?;
    Ok(manifest)
}
pub fn artifact(lease: &Lease, manifest: &Artifacts, name: &str) -> Result<Vec<u8>> {
    let a = manifest
        .files
        .iter()
        .find(|a| a.name == name)
        .ok_or("Unknown remote artifact.")?;
    if !matches!(
        name,
        "changes.patch" | "events.ndjson" | "trajectory.atif.json" | "result.json"
    ) {
        return Err("Invalid artifact path.".into());
    }
    let bytes = read_bounded(&lease.file(name)?, 32 * 1024 * 1024)?;
    if bytes.len() != a.bytes || digest(&bytes) != a.digest {
        return Err("Retained artifact identity mismatch.".into());
    }
    Ok(bytes)
}
pub fn apply(lease: &Lease, r: &Record, cwd: &Path) -> Result<()> {
    r.workspace
        .as_ref()
        .ok_or("This job has no repository input.")?
        .verify_caller(cwd)?;
    let manifest = r
        .artifacts
        .as_ref()
        .ok_or("No retained remote artifacts.")?;
    let bytes = artifact(lease, manifest, "changes.patch")?;
    if bytes.is_empty() {
        return Ok(());
    }
    use std::io::Write;
    for check in [true, false] {
        let mut command = Command::new("git");
        command
            .arg("-C")
            .arg(&r.workspace.as_ref().unwrap().source_root)
            .args(["apply", "--binary"]);
        if check {
            command.arg("--check");
        }
        let mut child = command
            .arg("-")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| "Cannot start Git apply.")?;
        child
            .stdin
            .take()
            .unwrap()
            .write_all(&bytes)
            .map_err(|_| "Cannot send the retained patch.")?;
        if !child
            .wait()
            .map_err(|_| "Cannot wait for Git apply.")?
            .success()
        {
            return Err("The patch cannot be applied to this checkout.".into());
        }
    }
    Ok(())
}

/// Seal the observed usage after remote teardown has been confirmed.
pub fn seal_usage(lease: &Lease, r: &mut Record) -> Result<()> {
    let bytes=serde_json::to_vec(&json!({"job":r.id,"state":r.state,"result":r.result,"error":r.error,"workspace":r.workspace,"usage":r.usage,"cleanup_complete":r.cleanup_complete})).map_err(|_| "Cannot encode the remote result.")?;
    if let Some(manifest) = &mut r.artifacts {
        write_private(&lease.file("result.json")?, &bytes)?;
        if let Some(a) = manifest.files.iter_mut().find(|a| a.name == "result.json") {
            a.bytes = bytes.len();
            a.digest = digest(&bytes);
        }
        write_private(
            &lease.file("manifest.json")?,
            &serde_json::to_vec(manifest).unwrap(),
        )?;
    }
    Ok(())
}
