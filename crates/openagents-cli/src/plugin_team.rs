//! Native team reuse of one reviewed request-only Wasm release.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fs::OpenOptions;
use std::io::Read;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::{Arc, atomic::AtomicBool};

use crate::{Args, Output};
use discovery::curated::{self, Source};
use nostr::contracts::{ArtifactRef, check_artifact_bytes, parse_artifact, parse_strict};
use serde_json::{Value, json};
use tenancy::accounts::team_capabilities::{
    self as team, Action, Completed, Release, Request, Sources, Verified,
};

pub(crate) const USAGE: &str =
    "usage: openagents plugin team COMMAND --registry DIR --credential FILE --workspace ID
  inspect --catalog FILE --mirror DIR --select KEY:PACKAGE/COMPONENT
  prepare --catalog FILE --mirror DIR --select KEY:PACKAGE/COMPONENT
    --id ID --member ACCOUNT --input FILE --purpose TEXT --expires-at UNIX
  grant --catalog FILE --mirror DIR --select KEY:PACKAGE/COMPONENT
    --request FILE --approve DIGEST
  revoke --grant ID --approve DIGEST
  install --catalog FILE --mirror DIR --select KEY:PACKAGE/COMPONENT
    --grant ID --operation-id ID
  enable --catalog FILE --mirror DIR --select KEY:PACKAGE/COMPONENT
    --grant ID --operation-id ID
  use --catalog FILE --mirror DIR --select KEY:PACKAGE/COMPONENT
    --grant ID --operation-id ID --input FILE --approve-input DIGEST
Selected registry and credential files are private, explicit native account
custody. Read access grants no enablement or execution. An owner or admin reviews
one exact release, member, local recipient, input digest, purpose, and expiry.
Install stays off; enable and use check the member's current action scope.
Only zero-fee, request-only Wasm packages with scoped signed evaluation evidence
are supported. Use has no network, provider, protected examples, or workspace
reads. An interrupted operation stays unknown and is never automatically rerun.";

pub(crate) fn run(output: &Output, words: &[String]) -> Option<u8> {
    if words.first().is_none_or(|w| w != "team") {
        return None;
    }
    if words
        .get(1)
        .is_none_or(|w| matches!(w.as_str(), "--help" | "-h"))
    {
        println!("{USAGE}");
        return Some(0);
    }
    let result = execute(&words[1..]);
    Some(match result {
        Ok(v) => {
            let failed = v["effect"]["receipt"]["outcome"]["type"] == "error";
            output.emit(&v, |v| serde_json::to_string_pretty(v).unwrap_or_default());
            if failed { crate::EXIT_FAILURE } else { 0 }
        }
        Err(e) => output.fail("plugin team", &e),
    })
}
fn execute(words: &[String]) -> Result<Value, String> {
    let args = Args::parse(&words[1..], &[])?;
    let mut allowed = vec!["registry", "credential", "workspace"];
    if words[0] != "revoke" {
        allowed.extend(["catalog", "mirror", "select"]);
    }
    allowed.extend(match words[0].as_str() {
        "inspect" => vec![],
        "prepare" => vec!["id", "member", "input", "purpose", "expires-at"],
        "grant" => vec!["request", "approve"],
        "revoke" => vec!["grant", "approve"],
        "install" | "enable" => vec!["grant", "operation-id"],
        "use" => vec!["grant", "operation-id", "input", "approve-input"],
        _ => return Err(USAGE.into()),
    });
    if !args.positional().is_empty()
        || args.option_names().iter().any(|s| !allowed.contains(s))
        || allowed.iter().any(|s| args.options(s).len() > 1)
    {
        return Err(USAGE.into());
    }
    let required = |name| {
        args.option(name)
            .filter(|v| !v.is_empty())
            .ok_or_else(|| format!("Missing --{name}.\n{USAGE}"))
    };
    let root = private_directory(Path::new(required("registry")?))?;
    let accounts = tenancy::accounts::Accounts::open(&root).map_err(|e| e.to_string())?;
    let credential = private_read(Path::new(required("credential")?), 4096)?;
    let token = std::str::from_utf8(&credential)
        .map_err(|_| "Invalid native credential file.")?
        .trim();
    let workspace = required("workspace")?;
    let grants = accounts.team_list(workspace, token)?;
    if words[0] == "revoke" {
        return Ok(
            json!({"revision":accounts.team_revoke(workspace, token, required("grant")?, required("approve")?)?,"authority":"revoked_for_new_operations"}),
        );
    }
    let mut source = NativeSource::new(
        Path::new(required("catalog")?),
        Path::new(required("mirror")?),
        required("select")?,
    )?;
    let cache = Rc::clone(&source.cache);
    let package = source
        .selected
        .rsplit_once('/')
        .map(|(package, _)| package.to_owned())
        .ok_or("Select one publisher-qualified package and component.")?;
    match words[0].as_str() {
        "inspect" => {
            let permissions = accounts.team_permissions(workspace, token)?;
            let current = accounts.team_source(workspace, token, &package, &mut source);
            Ok(
                json!({"schema":team::SCHEMA,"workspace":workspace,"authorization":{"state":if permissions.policy_blocks_new_effects {"policy_blocked"}else if !permissions.review && !permissions.enable && !permissions.use_capability {"insufficient_rights"}else{"separate_grant_required"},"permissions":permissions,"execution_authorized":false},"shared_cards":accounts.team_cards(workspace,token)?,"grants":grants.iter().map(|r| json!({"id":r.request.id,"member":r.request.member,"release":r.request.release,"digest":r.digest,"expires_at":r.request.expires_at,"state":if !r.active {"withdrawn"} else if r.request.expires_at<=crate::relay::unix_now(){"expired"}else if r.request.member == permissions.account && r.member_epoch != permissions.member_epoch {"insufficient_rights"}else if permissions.policy_blocks_new_effects {"policy_blocked"}else{"reviewed_grant"}})).collect::<Vec<_>>(),"selected":match current {Ok(v)=>json!({"state":if permissions.policy_blocks_new_effects {"policy_blocked"}else{"qualified_source"},"source_state":"qualified_source","release":v.release,"execution_authorized":false}),Err(e)=>json!({"state":source.state,"reason":e,"execution_authorized":false})},"limitations":["Signed publication and scoped measurement claims are attributable evidence, not remote attestation.","An explicit mirror may withhold newer records; freshness and retained head knowledge cannot prove global completeness.","Evaluation and discovery grant no installation, data access, or execution."]}),
            )
        }
        "prepare" => {
            let verified = accounts.team_source(workspace, token, &package, &mut source)?;
            let bytes = private_read(Path::new(required("input")?), 64 * 1024)?;
            std::str::from_utf8(&bytes).map_err(|_| "The selected request must be UTF-8 text.")?;
            let member = required("member")?.to_owned();
            let request = Request {
                schema: team::SCHEMA.into(),
                id: required("id")?.into(),
                workspace: workspace.into(),
                recipient: format!("local-member:{member}"),
                member,
                release: verified.release,
                input: team::bytes_digest(&bytes),
                input_bytes: bytes.len() as u64,
                purpose: required("purpose")?.into(),
                expires_at: required("expires-at")?
                    .parse()
                    .map_err(|_| "Invalid --expires-at.")?,
            };
            Ok(
                json!({"request":request,"approval":request.digest(),"authority_granted":false,"data_authority":"Reviewer must hold the exact input's rights; this preparation does not establish them."}),
            )
        }
        "grant" => {
            let request: Request = serde_json::from_value(
                parse_strict(&private_read(Path::new(required("request")?), 64 * 1024)?)
                    .map_err(|_| "Invalid team request.")?,
            )
            .map_err(|_| "Invalid team request.")?;
            if request.workspace != workspace {
                return Err("Team request names another selected workspace.".into());
            }
            let revision =
                accounts.team_grant(token, request, required("approve")?, &mut source)?;
            Ok(
                json!({"revision":revision,"authority":"exact_member_release_input_only","installed":false,"enabled":false}),
            )
        }
        "install" | "enable" | "use" => {
            let action = match words[0].as_str() {
                "install" => Action::Install,
                "enable" => Action::Enable,
                _ => Action::Use,
            };
            let input = if action == Action::Use {
                Some(private_read(Path::new(required("input")?), 64 * 1024)?)
            } else {
                None
            };
            let approved = if input.is_some() {
                Some(required("approve-input")?)
            } else {
                None
            };
            let layout = background::Layout::from_env().map_err(|e| e.to_string())?;
            let operation_id = required("operation-id")?;
            let operation = action.clone();
            let result = accounts.team_apply(workspace,token,required("grant")?,operation_id,action,
                input.as_deref().zip(approved),&mut source, |revision,fence| {
                    let bundle = cache.borrow().as_ref().ok_or("Native team source was not reconstructed.")?.clone();
                    if bundle.release != revision.request.release { return Err("Native source differs from the admitted release.".into()); }
                    fence.before_effect()?;
                    let (receipt, output) = match operation {
                        Action::Install => {
                            std::fs::create_dir_all(layout.extensions()).map_err(|e|e.to_string())?;
                            let stage = tempfile::Builder::new().prefix(".team-").tempdir_in(layout.extensions()).map_err(|e|e.to_string())?;
                            for (name,bytes) in &bundle.files { let path=stage.path().join(name); std::fs::create_dir_all(path.parent().unwrap()).map_err(|e|e.to_string())?; std::fs::write(path,bytes).map_err(|e|e.to_string())?; }
                            fence.before_effect()?;
                            let installed = crate::plugin_local::install_into(&layout,stage.path())?;
                            (json!({"owner":"native_plugin_install","package":bundle.release.package,"version":bundle.release.version,"manifest":bundle.release.manifest,"release":bundle.release.release,"enabled":false}),Some(installed))
                        }
                        Action::Enable => {
                            verify_installed(&layout,&bundle,false)?;
                            fence.before_effect()?;
                            let record = &bundle.files["package.json"];
                            let value = crate::plugin_local::enable_exact_in(&layout,&bundle.release.package,true,Some((&bundle.release.version,&team::bytes_digest(record))))?;
                            (json!({"owner":"native_plugin_enable","package":bundle.release.package,"version":bundle.release.version,"release":bundle.release.release,"enabled":true}),Some(value))
                        }
                        Action::Use => {
                            let _native_lock = background::plugins::mutation_lock(&layout)?;
                            verify_installed(&layout,&bundle,true)?;
                            let packet=crate::pay_plugin::packet_from_program(&bundle.program).map_err(|e|e.message)?;
                            let text=std::str::from_utf8(input.as_deref().unwrap()).map_err(|_|"The selected request must be UTF-8 text.")?;
                            let mut value=packet.input.clone();
                            if let Some(key)=&packet.request_key { if value.is_null(){value=json!({});} value.as_object_mut().ok_or("Unsupported request input.")?.insert(key.clone(),json!(text)); } else {value=json!(text);}
                            fence.before_effect()?;
                            let invocation=format!("team:{workspace}:{operation_id}");
                            let empty=plugin::Snapshot::default();
                            let (outcome,receipt)=plugin::invoke_with_receipt(plugin::Call {wasm:&packet.wasm,profile:packet.profile,invocation:&invocation,operation:&packet.operation,input:&value,snapshot:&empty,handles:&BTreeMap::new(),limits:packet.limits,cancelled:Arc::new(AtomicBool::new(false)),required:true});
                            let output=match outcome {Ok(v)=>json!({"status":v.status,"value":v.value}),Err(e)=>json!({"status":"failed","reason":e.to_string()})};
                            (receipt.to_json(),Some(output))
                        }
                    };
                    fence.after_effect()?;
                    Ok(Completed {receipt,output})
                })?;
            Ok(
                json!({"effect":result.effect,"output":result.output,"replayed":result.replayed,"authority":"original_exact_admission","model_cost":"none_no_model_invoked"}),
            )
        }
        _ => Err(USAGE.into()),
    }
}

#[derive(Clone)]
struct Bundle {
    release: Release,
    files: BTreeMap<String, Vec<u8>>,
    program: Value,
}
struct NativeSource {
    catalog: PathBuf,
    mirror: PathBuf,
    selected: String,
    cache: Rc<RefCell<Option<Bundle>>>,
    knowledge: Vec<Value>,
    state: String,
}
impl NativeSource {
    fn new(catalog: &Path, mirror: &Path, selected: &str) -> Result<Self, String> {
        if !catalog.is_absolute() || !mirror.is_absolute() {
            return Err("Team sources must use explicit absolute paths.".into());
        }
        // No metadata locator or publisher-provided URL is opened.
        crate::plugin_discovery::Mirror::open(mirror)?;
        Ok(Self {
            catalog: catalog.to_owned(),
            mirror: mirror.to_owned(),
            selected: selected.into(),
            cache: Rc::new(RefCell::new(None)),
            knowledge: Vec::new(),
            state: "unavailable".into(),
        })
    }
}
impl Sources for NativeSource {
    fn knowledge(&self) -> Vec<Value> {
        self.knowledge.clone()
    }
    fn current(&mut self, previous: &[Value]) -> Result<Verified, String> {
        *self.cache.borrow_mut() = None;
        self.state = "unavailable".into();
        let bytes = crate::plugin_discovery::read_file(&self.catalog, curated::MAX_CATALOG_BYTES)?;
        let mut source = crate::plugin_discovery::Mirror::open(&self.mirror)?;
        let previous = previous
            .iter()
            .cloned()
            .map(serde_json::from_value)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| "Invalid retained team source evidence.")?;
        let snapshot = curated::discover(&bytes, &mut source, &previous, crate::relay::unix_now())?;
        self.knowledge = snapshot
            .evidence
            .iter()
            .map(|e| serde_json::to_value(e).expect("an event serializes"))
            .collect();
        let card = snapshot
            .cards
            .iter()
            .find(|c| format!("{}/{}", c.id, c.selected_operation) == self.selected)
            .ok_or("Selected exact team capability is absent.")?;
        self.state = if card.state == "withdrawn" {
            "withdrawn"
        } else if card.state == "unavailable" {
            "unavailable"
        } else {
            "unqualified"
        }
        .into();
        if card.state != "verified_discovery" || card.review["state"] != "current_scoped_review" {
            return Err(format!(
                "Team capability is {} or unqualified: {}",
                card.state,
                card.error
                    .as_deref()
                    .unwrap_or("No current exact passing evaluation review.")
            ));
        }
        self.state = "unqualified".into();
        if card.kind != "extension"
            || card.operation["kind"] != "program"
            || card.price["publisher_fee_msat"].as_u64() != Some(0)
        {
            return Err("This team lane supports zero-fee native Wasm programs; a paid release needs its separate owning purchase path.".into());
        }
        if card.review["data_requirements"] != json!(["explicit-request-text"])
            || card.review["recipients"] != json!(["local-wasm"])
        {
            return Err("Source data or recipient requirements exceed the selected local request-only lane.".into());
        }
        let program = card
            .component
            .clone()
            .ok_or("Missing verified program bytes.")?;
        let packet = crate::pay_plugin::packet_from_program(&program).map_err(|e| e.message)?;
        let event = snapshot
            .evidence
            .iter()
            .find(|e| e.id == card.selected_event)
            .ok_or("The selected release has no retained signed evidence.")?
            .clone();
        let body = nostr::ext::parse_record(&event).map_err(|e| e.to_string())?;
        let reference = parse_artifact(&body["manifest"]).map_err(|e| e.to_string())?;
        let manifest_bytes = source.artifact(&reference)?;
        check_artifact_bytes(&reference, &manifest_bytes).map_err(|e| e.to_string())?;
        let manifest_value =
            parse_strict(&manifest_bytes).map_err(|_| "Invalid exact team manifest.")?;
        let manifest = nostr::ext::parse_manifest(&manifest_value).map_err(|e| e.to_string())?;
        let program_path = format!("programs/{}.json", card.selected_operation);
        if manifest.files.len() != 2
            || manifest_value["components"]
                .as_array()
                .is_none_or(|v| v.len() != 1)
            || manifest
                .files
                .iter()
                .any(|f| f.path != "package.json" && f.path != program_path)
        {
            return Err("Team reuse accepts only the package and selected program, without creator examples or additional files.".into());
        }
        let mut files = BTreeMap::new();
        for file in &manifest.files {
            let reference = ArtifactRef {
                digest: file.digest.clone(),
                size: file.size,
                media_type: file.media_type.clone(),
                schema: None,
                event: None,
                sources: vec![],
            };
            let bytes = source.artifact(&reference)?;
            check_artifact_bytes(&reference, &bytes).map_err(|e| e.to_string())?;
            files.insert(file.path.clone(), bytes);
        }
        if card.operation["definition"] != team::bytes_digest(&files[&program_path])
            || parse_strict(&files[&program_path]).map_err(|_| "Invalid selected program.")?
                != program
        {
            return Err(
                "The package program differs from the selected evaluated component.".into(),
            );
        }
        let record: Value =
            parse_strict(&files["package.json"]).map_err(|_| "Invalid selected team package.")?;
        if record["publisher"] != event.pubkey
            || record["version"] != manifest.version
            || record["classes"].as_array().is_some_and(|v| !v.is_empty())
            || [
                "background",
                "questions",
                "sources",
                "policies",
                "capabilities",
                "trusted_publishers",
            ]
            .iter()
            .any(|field| record[*field].as_array().is_some_and(|v| !v.is_empty()))
        {
            return Err("Selected team package carries unsupported authority or identity.".into());
        }
        let release = Release {
            source: serde_json::to_string(&json!({"catalog":self.catalog,"mirror":self.mirror}))
                .map_err(|e| e.to_string())?,
            catalog: snapshot.catalog_digest,
            package: card.id.clone(),
            publisher: event.pubkey,
            release: card.selected_event.clone(),
            manifest: card.selected_digest.clone(),
            version: manifest.version,
            component: card.selected_operation.clone(),
            program: team::bytes_digest(&files[&program_path]),
            operation: packet.operation,
            wasm: team::bytes_digest(&packet.wasm),
            evaluations: card
                .evaluation
                .iter()
                .filter_map(|e| e["event"].as_str().map(str::to_owned))
                .collect(),
            data_requirements: vec!["explicit-request-text".into()],
            source_recipients: vec!["local-wasm".into()],
        };
        *self.cache.borrow_mut() = Some(Bundle {
            release: release.clone(),
            files,
            program,
        });
        self.state = "qualified_source".into();
        Ok(Verified {
            release,
            evidence: snapshot
                .evidence
                .iter()
                .map(|e| serde_json::to_value(e).expect("an event serializes"))
                .collect(),
        })
    }
}
fn verify_installed(
    layout: &background::Layout,
    bundle: &Bundle,
    enabled: bool,
) -> Result<(), String> {
    let installed = background::plugins::find(layout, &bundle.release.package)?;
    if installed.version != bundle.release.version
        || (enabled && !installed.enabled)
        || !installed.background.is_empty()
        || !installed.classes.is_empty()
    {
        return Err("The exact team release is unavailable or disabled on this computer.".into());
    }
    for (path, expected) in &bundle.files {
        let bytes = crate::plugin_discovery::read_file(
            &installed.dir.join(path),
            curated::MAX_ARTIFACT_BYTES,
        )?;
        if &bytes != expected {
            return Err(
                "Installed team release bytes differ from the reviewed signed release.".into(),
            );
        }
    }
    Ok(())
}
fn private_directory(path: &Path) -> Result<PathBuf, String> {
    if !path.is_absolute() {
        return Err("Select an absolute private native registry directory.".into());
    }
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW)
        .open(path)
        .map_err(|_| "Select a private native registry directory without a symlink.")?;
    let meta = file.metadata().map_err(|e| e.to_string())?;
    // SAFETY: geteuid returns the current process identity and has no pointers.
    if meta.mode() & 0o077 != 0 || meta.uid() != unsafe { libc::geteuid() } {
        return Err(
            "The selected native registry must be owned by this user with mode 0700.".into(),
        );
    }
    Ok(path.to_owned())
}
fn private_read(path: &Path, limit: usize) -> Result<Vec<u8>, String> {
    if !path.is_absolute() {
        return Err("Select an absolute private file path.".into());
    }
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .map_err(|_| "Select an explicit private regular file without a symlink.")?;
    let meta = file.metadata().map_err(|e| e.to_string())?;
    // SAFETY: geteuid returns the current process identity and has no pointers.
    if !meta.is_file()
        || meta.mode() & 0o077 != 0
        || meta.nlink() != 1
        || meta.uid() != unsafe { libc::geteuid() }
        || meta.len() > limit as u64
    {
        return Err(
            "The selected file must be private, user-owned, without hard links, and within its byte limit."
                .into(),
        );
    }
    let mut bytes = Vec::new();
    file.by_ref()
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > limit {
        return Err("Private input exceeds its byte limit.".into());
    }
    Ok(bytes)
}
