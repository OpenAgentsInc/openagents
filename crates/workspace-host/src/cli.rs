//! A local authority CLI over the same authenticated host contracts.
use crate::{
    Result,
    artifact::{self, MAX_BYTES},
    git::GitDocument,
    host::{Host, request},
    outbox::Bundle,
};
use nostr::workspace::ResourceRef;
use secp256k1::SecretKey;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};
pub const USAGE: &str = "openagents workspace init|binding|add-git|read|descriptor|replace-plan|apply|grant|revoke|view|snapshot|changes|fetch|visibility|checkpoint|restore-plan|restore|projects --state DIR --key-file FILE [OPTIONS]\nadd-git: --repository DIR --branch refs/heads/NAME --path FILE --name TITLE\nread|descriptor|replace-plan: --resource ID [--content FILE]\napply: --bundle FILE\ngrant: --actor HEX --resources FILE --expires UNIX [--write] [--require-approval]\nrevoke: --actor HEX\nview: --recipient HEX --resources FILE --retain-until UNIX\nsnapshot|changes: --projection FILE [--cursor TOKEN] [--max-items N] [--max-bytes N]\nfetch: --projection FILE --artifact FILE [--max-bytes N]\nvisibility: --projection FILE --receipt FILE\ncheckpoint: --resources FILE --retain-until UNIX\nrestore-plan: --checkpoint FILE\nrestore: --plan FILE\nprojects: --projects FILE";
pub fn run(words: Vec<String>) -> Result<Option<Value>> {
    if words.is_empty() || words.iter().any(|s| matches!(s.as_str(), "--help" | "-h")) {
        println!("{USAGE}");
        return Ok(None);
    }
    let command = words[0].as_str();
    let mut options = BTreeMap::new();
    let mut index = 1;
    while index < words.len() {
        let name = &words[index];
        if !name.starts_with("--") {
            return Err(USAGE.into());
        }
        let value = if matches!(name.as_str(), "--write" | "--require-approval") {
            "true".into()
        } else {
            index += 1;
            words
                .get(index)
                .filter(|s| !s.starts_with("--"))
                .ok_or("workspace option needs a value")?
                .clone()
        };
        if options.insert(name.clone(), value).is_some() {
            return Err("workspace option repeated".into());
        }
        index += 1;
    }
    let allowed: &[&str] = match command {
        "init" | "binding" => &[],
        "add-git" => &["--repository", "--branch", "--path", "--name"],
        "read" | "descriptor" => &["--resource"],
        "replace-plan" => &["--resource", "--content"],
        "apply" => &["--bundle"],
        "grant" => &[
            "--actor",
            "--resources",
            "--expires",
            "--write",
            "--require-approval",
        ],
        "revoke" => &["--actor"],
        "view" => &["--recipient", "--resources", "--retain-until"],
        "snapshot" | "changes" => &["--projection", "--cursor", "--max-items", "--max-bytes"],
        "fetch" => &["--projection", "--artifact", "--max-bytes"],
        "visibility" => &["--projection", "--receipt"],
        "checkpoint" => &["--resources", "--retain-until"],
        "restore-plan" => &["--checkpoint"],
        "restore" => &["--plan"],
        "projects" => &["--projects"],
        _ => return Err(USAGE.into()),
    };
    if options
        .keys()
        .any(|k| !matches!(k.as_str(), "--state" | "--key-file") && !allowed.contains(&k.as_str()))
    {
        return Err("option does not apply to this workspace command".into());
    }
    let option = |name: &str| {
        options
            .get(name)
            .map(String::as_str)
            .ok_or_else(|| format!("missing workspace option {name}"))
    };
    let secret = secret(Path::new(option("--key-file")?))?;
    let root = Path::new(option("--state")?).to_path_buf();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| "clock is before Unix time")?
        .as_secs();
    let host = if command == "init" {
        Host::create(root, secret, now)?
    } else {
        Host::open(root, secret)?
    };
    let public = host.signer.pubkey();
    let input = |name: &str| read_json(Path::new(option(name)?));
    let number = |name: &str| {
        option(name)?
            .parse::<u64>()
            .map_err(|_| format!("invalid number for {name}"))
    };
    let resource = || -> Result<ResourceRef> {
        let binding = host.binding()?;
        let r = ResourceRef {
            authority: public.into(),
            workspace: binding["workspace"].as_str().ok_or("workspace ID")?.into(),
            resource: option("--resource")?.into(),
        };
        r.validate().map_err(|e| e.to_string())?;
        Ok(r)
    };
    let signed = |body: Value| request(&host.secret, public, body, now);
    let value=match command{
        "init"|"binding"=>host.binding()?,
        "add-git"=>serde_json::to_value(host.register_git(GitDocument::admit(Path::new(option("--repository")?),option("--branch")?,option("--path")?)?,option("--name")?.into(),now)?).map_err(|e|e.to_string())?,
        "read"|"descriptor"=>{let event=signed(json!({"v":"openagents.ws-resource-read.v1","requires":[],"resource":resource()?}))?;if command=="read"{host.read_document(&event,now)?}else{host.descriptor(&event,now)?}},
        "replace-plan"=>{
            let r=resource()?;let before=host.read_document(&signed(json!({"v":"openagents.ws-resource-read.v1","requires":[],"resource":r}))?,now)?;let bytes=artifact::read(Path::new(option("--content")?),MAX_BYTES)?;std::str::from_utf8(&bytes).map_err(|_|"replacement must be UTF-8")?;let content=artifact::reference(&bytes,"text/plain",None);
            let replacement=json!({"v":"openagents.document-replacement.v1","requires":[],"snapshot":before["snapshot"],"content":content,"encoding":before["body"]["encoding"]});let proposal=json!({"v":"openagents.resource-change.v1","requires":[],"command":artifact::token(),"actor":public,"resource":r,"binding":before["body"]["binding"],"expected":before["body"]["observation"],"operation":before["operation"],"input":artifact::json_ref(&replacement)?,"authorization":before["authorization"],"issued_at":now,"expires_at":now+300});let admission=json!({"v":"openagents.workspace-action-admission.v1","requires":[],"proposal":artifact::json_ref(&proposal)?,"actor":public,"approval":null});let mut attachments=BTreeMap::from([(content["digest"].as_str().ok_or("content digest")?.to_string(),bytes)]);for body in [proposal,replacement]{let bytes=nostr::contracts::jcs(&body).map_err(|e|e.to_string())?;attachments.insert(nostr::contracts::digest_bytes(&bytes),bytes);}serde_json::to_value(Bundle{event:signed(admission)?,attachments}).map_err(|e|e.to_string())?
        }
        "apply"=>{let value=nostr::contracts::parse_strict_bounded(&artifact::read(Path::new(option("--bundle")?),24*MAX_BYTES)?,24*MAX_BYTES).map_err(|e|e.to_string())?;let bundle:Bundle=serde_json::from_value(value).map_err(|_|"invalid workspace change bundle")?;host.change(&bundle.event,&bundle.attachments,now,None)?},
        "grant"=>{let resources:BTreeSet<String>=serde_json::from_value(input("--resources")?).map_err(|_|"resources must be an array of IDs")?;host.grant(option("--actor")?,resources,options.contains_key("--write"),options.contains_key("--require-approval"),number("--expires")?)?},
        "revoke"=>{host.revoke(option("--actor")?)?;json!({"revoked":true})},
        "view"=>{let resources:BTreeSet<String>=serde_json::from_value(input("--resources")?).map_err(|_|"resources must be an array of IDs")?;host.define_projection(option("--recipient")?,resources,number("--retain-until")?,now)?},
        "snapshot"|"changes"=>{let projection=input("--projection")?;let result=host.projection_read(&signed(json!({"v":"openagents.projection-read.v1","requires":[],"request":artifact::token(),"projection":projection,"mode":if command=="snapshot"{"snapshot"}else{"changes"},"after":options.get("--cursor"),"max_items":options.get("--max-items").map(|n|n.parse::<u64>()).transpose().map_err(|_|"invalid max-items")?.unwrap_or(64),"max_bytes":options.get("--max-bytes").map(|n|n.parse::<u64>()).transpose().map_err(|_|"invalid max-bytes")?.unwrap_or(65536)}))?,now)?;let page=if result["status"]=="completed"{let bytes=host.projection_fetch(&signed(json!({"v":"openagents.ws-projection-fetch.v1","requires":[],"projection":projection,"artifact":result["page"],"max_bytes":1048576}))?,now)?;Some(nostr::contracts::parse_strict(&bytes).map_err(|e|e.to_string())?)}else{None};json!({"result":result,"page":page})},
        "fetch"=>{let reference=input("--artifact")?;let bytes=host.projection_fetch(&signed(json!({"v":"openagents.ws-projection-fetch.v1","requires":[],"projection":input("--projection")?,"artifact":reference,"max_bytes":options.get("--max-bytes").map(|n|n.parse::<u64>()).transpose().map_err(|_|"invalid max-bytes")?.unwrap_or(1048576)}))?,now)?;if reference["media_type"]=="application/json"{nostr::contracts::parse_strict(&bytes).map_err(|e|e.to_string())?}else{json!({"reference":reference,"content_utf8":String::from_utf8(bytes).map_err(|_|"artifact is not UTF-8")?})}},
        "visibility"=>host.command_visibility(&signed(json!({"v":"openagents.ws-command-visibility-read.v1","requires":[],"projection":input("--projection")?,"command":input("--receipt")?}))?,now)?,
        "checkpoint"=>{let ids:Vec<String>=serde_json::from_value(input("--resources")?).map_err(|_|"resources must be an array of IDs")?;let binding=host.binding()?;let resources=ids.iter().map(|id|json!({"authority":public,"workspace":binding["workspace"],"resource":id})).collect::<Vec<_>>();host.checkpoint(&signed(json!({"v":"openagents.ws-checkpoint-create.v1","requires":[],"resources":resources,"retain_until":number("--retain-until")?}))?,now)?},
        "restore-plan"=>host.plan_restore(&signed(json!({"v":"openagents.ws-restore-plan.v1","requires":[],"checkpoint":input("--checkpoint")?}))?,now)?,
        "restore"=>host.restore(&signed(json!({"v":"openagents.workspace-action-admission.v1","requires":[],"proposal":input("--plan")?,"actor":public,"approval":null}))?,now,None)?,
        "projects"=>host.associate_projects(serde_json::from_value(input("--projects")?).map_err(|_|"projects must be an array of exact references")?)?,
        _=>return Err(USAGE.into()),
    };
    Ok(Some(value))
}
fn read_json(path: &Path) -> Result<Value> {
    nostr::contracts::parse_strict(&artifact::read(path, MAX_BYTES)?).map_err(|e| e.to_string())
}
fn secret(path: &Path) -> Result<SecretKey> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if std::fs::metadata(path)
            .map_err(|_| "workspace key file unavailable")?
            .permissions()
            .mode()
            & 0o077
            != 0
        {
            return Err("key file must be readable only by its owner".into());
        }
    }
    let value =
        String::from_utf8(artifact::read(path, 4096)?).map_err(|_| "invalid workspace key file")?;
    if value.trim().starts_with("nsec1") {
        SecretKey::from_byte_array(
            nostr::nip19::decode_nsec(value.trim()).map_err(|_| "invalid workspace signing key")?,
        )
        .map_err(|_| "invalid workspace signing key".into())
    } else {
        value
            .trim()
            .parse()
            .map_err(|_| "invalid workspace signing key".into())
    }
}
pub fn display(value: &Value) -> String {
    value.to_string()
}
