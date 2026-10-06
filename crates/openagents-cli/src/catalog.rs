//! Read-only discovery of published NIP-CAP capabilities (`kind:30180`),
//! NIP-PRG programs (`kind:30182`), and NIP-EXT records (`kind:30184` and
//! its release, revocation, migration, and checkpoint kinds) from a relay.
//!
//! Every command only reads. Nothing here installs a package, runs a probe,
//! mints a grant, or pins a definition; a listed head is a claim its signer
//! made, checked here for shape and signature and nothing more. The
//! `nostr::cap`, `nostr::prg`, and `nostr::ext` contracts decide whether a
//! body is well formed, and the answer travels with every record.

use std::collections::BTreeMap;
use std::time::Duration;

use nostr::domain::Event;
use serde_json::{Value, json};

use crate::relay::{Client, DEFAULT_WAIT, relay_url, signer_for, unix_now};
use crate::{Args, Output, out};
#[cfg(test)]
use coder::cli_route::tree::{Declared, Effect};

pub(crate) const CAP_USAGE: &str = "usage: openagents cap COMMAND [OPTIONS]
  list [--author PUBKEY] [--profile PROFILE] [--limit N] [--all]
        List published capability heads (kind 30180, oa:cap:v1), newest
        first, one row per signer and slug. Hide test/dev listings and publishers
        without signed presence within 7 days. --all includes them with old/test.
  describe PUBKEY:SLUG | --author PUBKEY SLUG
        Print one capability head with its parsed definition.
Options for every command:
  --relay URL         Relay to read (default wss://relay.openagents.com).
  --timeout SECONDS   How long to wait for the relay (default 8).
  --as PROFILE        Verse profile key that answers a NIP-42 challenge.
Reading a head runs no probe, installs nothing, and grants nothing.";

#[cfg(test)]
pub(crate) const CAP_EFFECTS: &[Declared] = &[
    Declared::computer("list", Effect::ReadOnly),
    Declared::computer("describe", Effect::ReadOnly),
];

pub(crate) const PRG_USAGE: &str = "usage: openagents prg COMMAND [OPTIONS]
  list [--author PUBKEY] [--step KIND] [--limit N]
        List published program heads (kind 30182, oa:program:v1), newest
        first, one row per signer and slug.
  describe PUBKEY:SLUG | --author PUBKEY SLUG
        Print one program head with its parsed definition and steps.
Options for every command:
  --relay URL         Relay to read (default wss://relay.openagents.com).
  --timeout SECONDS   How long to wait for the relay (default 8).
  --as PROFILE        Verse profile key that answers a NIP-42 challenge.
Reading a head pins nothing and admits nothing.";

#[cfg(test)]
pub(crate) const PRG_EFFECTS: &[Declared] = &[
    Declared::computer("list", Effect::ReadOnly),
    Declared::computer("describe", Effect::ReadOnly),
];

pub(crate) const EXT_USAGE: &str = "usage: openagents plugin COMMAND [OPTIONS]
  new SLUG [--name NAME] [--in DIR] [--from-rule ID]
        Start a plugin in DIR (default ./SLUG): its package.json, a skill
        under skills/, and a README with the next commands. --from-rule
        makes it from a background rule made on this computer.
  pin [DIR]
        Set every digest DIR's package.json states to its file's digest
        now. install and publish do this for a plugin folder too.
  list [--type TYPE] [--author PUBKEY] [--package ID] [--limit N]
        List published plugin records (NIP-EXT). TYPE is listing (default),
        release, revocation, migration, or checkpoint.
  run DIR [--in WORKSPACE] [--request TEXT | --request-file FILE]
        Run the plugin's workflow once on WORKSPACE through Coder's
        program runtime, granted reads only, and print its reply.
  publish [DIR] [--fee-msat N --payout ADDRESS] [--blossom URL] [--blobs-dir DIR]
        Publish the plugin in DIR (default .) under your key: its files go
        to the blob server, then a signed NIP-EXT release and listing.
        Prints its id, KEY:SLUG. --fee-msat and --payout put a per-call fee
        and the Lightning address or node key paid into the release.
        --blobs-dir writes the files by digest for an operator to upload,
        and --blossom then names where readers fetch them.
  search [QUERY] [--author PUBKEY] [--limit N]
        The published plugins that match QUERY, best first; all without one.
  install DIR | NAME | ID [--blossom URL]
        Install the plugin in DIR, or a published one by name or id, on this
        computer. A published one is checked file by file against its signed
        release. It starts off.
  installed
        The plugins installed on this computer, on or off.
  enable PLUGIN
        Turn an installed plugin on on this computer. A plugin that runs
        in the background runs only while it is on.
  disable PLUGIN
        Turn an installed plugin off on this computer.
  inspect [PLUGIN] [--results DIR]...
        Show each installed plugin by its exact release (version and
        package digest), whether it is on, and the test results under
        each DIR for that exact release apart from other releases.
  use PLUGIN --version V --digest D [--request TEXT] [--in WORKSPACE] [--thread ID]
        Run an installed plugin's workflow once through the shared route,
        only while exactly that release is installed and on.
  workbench freeze --root DIR --review FILE
        Retain a reviewed plugin draft and its original flow and task.
  workbench show --root DIR
        Read the retained flow, tests, comparisons, and action outcomes.
  workbench approve --root DIR --request FILE
        Execute one exact reviewed action from a typed JSON request.
  test run TARGET [--runs N] [--case GLOB]... [--tag TAG]... [--baseline on|off]
      [--concurrency N] [--grant read|write|exec|network]... [--trust]
      [--door NAME] [--eval-dir DIR] [--output-dir DIR] [--keep-temp] [--coder PATH]
      [--questions DIR]
        Run a plugin's tests with the plugin and without it.
  test init [TARGET] [--bare] [--out DIR] [--eval-dir DIR]
        Write a test set with the authoring interview for the plugin at
        TARGET, or with --bare a blank test named TARGET from the template.
  test release TARGET [--eval-dir DIR] [--gate ID] [--blossom URL]
        Release TARGET's test set as a NIP-EXT release without running it.
  test publish REPORT [--blossom URL]
        Add a test result to the Gym: its test set's release and a 3189 result.
  test check EVENT [TARGET] [--runs N] [--concurrency N] [--trust] [--coder PATH]
      [--questions DIR] [--blossom URL]
        Rerun a published test result and publish a confirm or a dispute.
  test studies [DIR]
        List the test results under DIR, newest first.
  test show RESULTS
        Recompute a retained test result from its attempts; runs nothing.
  defaults sync [--root PUBKEY] [--catalog DIR]... [--into DIR]
        Write the plugins Coder uses for everyone (the newest coder-defaults
        release) into the directory Coder reads.
  defaults show [--into DIR]
        Print what the last sync wrote.
Options for every command:
  --relay URL         Relay to read (default wss://relay.openagents.com).
  --timeout SECONDS   How long to wait for the relay (default 8).
  --as PROFILE        Verse profile key that answers a NIP-42 challenge.
Listing a record installs nothing; a listing is discovery, not a pin.
`ext` is another name for `plugin`, and `eval` for `test`.
Run `openagents plugin test --help` and `openagents plugin defaults --help`
for those commands in full.";

#[cfg(test)]
pub(crate) const EXT_EFFECTS: &[Declared] = &[
    Declared::computer("new", Effect::LocalWrite),
    Declared::computer("pin", Effect::LocalWrite),
    Declared::computer("list", Effect::ReadOnly),
    Declared::computer("run", Effect::ReadOnly),
    Declared::computer("publish", Effect::Publishes),
    Declared::computer("search", Effect::ReadOnly),
    Declared::computer("install", Effect::LocalWrite),
    Declared::computer("installed", Effect::ReadOnly),
    Declared::computer("enable", Effect::LocalWrite),
    Declared::computer("disable", Effect::LocalWrite),
    Declared::computer("inspect", Effect::ReadOnly),
    Declared::computer("use", Effect::LocalWrite),
    Declared::computer("workbench freeze", Effect::LocalWrite),
    Declared::computer("workbench show", Effect::ReadOnly),
    Declared::computer("workbench approve", Effect::Publishes),
    crate::ext_eval::EFFECTS[0],
    crate::ext_eval::EFFECTS[1],
    crate::ext_eval::EFFECTS[2],
    crate::ext_eval::EFFECTS[3],
    crate::ext_eval::EFFECTS[4],
    crate::ext_eval::EFFECTS[5],
    crate::ext_eval::EFFECTS[6],
    Declared::computer("defaults sync", Effect::LocalWrite),
    Declared::computer("defaults show", Effect::ReadOnly),
];

const DEFAULT_LIMIT: u64 = 100;

/// Which group is running, so usage and error lines name it.
#[derive(Clone, Copy)]
enum Group {
    Cap,
    Prg,
    Ext,
}

impl Group {
    const fn name(self) -> &'static str {
        match self {
            Self::Cap => "cap",
            Self::Prg => "prg",
            Self::Ext => "plugin",
        }
    }

    const fn usage(self) -> &'static str {
        match self {
            Self::Cap => CAP_USAGE,
            Self::Prg => PRG_USAGE,
            Self::Ext => EXT_USAGE,
        }
    }

    const fn kind(self) -> u16 {
        match self {
            Self::Cap => nostr::cap::DISCOVERY_KIND,
            Self::Prg => nostr::prg::DISCOVERY_KIND,
            Self::Ext => nostr::ext::LISTING_KIND,
        }
    }

    const fn marker(self) -> &'static str {
        match self {
            Self::Cap => nostr::cap::CAP_MARKER,
            Self::Prg => nostr::prg::PROGRAM_MARKER,
            Self::Ext => "oa:ext:listing:v1",
        }
    }
}

pub fn cap(output: &Output, words: &[String]) -> u8 {
    run(Group::Cap, output, words)
}

pub fn prg(output: &Output, words: &[String]) -> u8 {
    run(Group::Prg, output, words)
}

/// `openagents plugin …`, also `openagents ext …`. `test` is the plugin's
/// with-and-without evaluation, also `eval`.
pub fn ext(output: &Output, words: &[String]) -> u8 {
    if let Some((command, rest)) = words.split_first()
        && rest.first().is_some_and(|word| word == "--help")
        && !matches!(command.as_str(), "test" | "eval" | "defaults" | "run")
        && let Some(usage) = crate::argv::command_usage("plugin", command, EXT_USAGE)
    {
        println!("{usage}");
        return 0;
    }
    if words
        .first()
        .is_some_and(|word| word == "test" || word == "eval")
    {
        return crate::ext_eval::run(output, &words[1..]);
    }
    if words.first().is_some_and(|word| word == "defaults") {
        return crate::ext_defaults::run(output, &words[1..]);
    }
    if words.first().is_some_and(|word| word == "run") {
        return crate::ext_run::run(output, &words[1..]);
    }
    #[cfg(unix)]
    if let Some(code) = crate::plugin_workbench::run(output, words) {
        return code;
    }
    #[cfg(unix)]
    if let Some(code) = crate::plugin_new::run(output, words) {
        return code;
    }
    #[cfg(unix)]
    if let Some(code) = crate::plugin_registry::run(output, words) {
        return code;
    }
    #[cfg(unix)]
    if let Some(code) = crate::plugin_local::run(output, words) {
        return code;
    }
    #[cfg(unix)]
    if let Some(code) = crate::plugin_use::run(output, words) {
        return code;
    }
    run(Group::Ext, output, words)
}

fn run(group: Group, output: &Output, words: &[String]) -> u8 {
    let name = group.name();
    let usage = group.usage();
    let Some((command, rest)) = words.split_first() else {
        return output.usage(name, "a command is required", usage);
    };
    if command == "--help" || command == "-h" || command == "help" {
        println!("{usage}");
        return 0;
    }
    let args = match Args::parse(rest, &["all"]) {
        Ok(args) => args,
        Err(message) => return output.usage(name, &message, usage),
    };
    let timeout = match args.number::<u64>("timeout", DEFAULT_WAIT.as_secs()) {
        Ok(0) => return output.usage(name, "--timeout must be at least 1 second", usage),
        Ok(seconds) => Duration::from_secs(seconds),
        Err(message) => return output.usage(name, &message, usage),
    };
    let limit = match args.number::<u64>("limit", DEFAULT_LIMIT) {
        Ok(0) => return output.usage(name, "--limit must be at least 1", usage),
        Ok(limit) => limit,
        Err(message) => return output.usage(name, &message, usage),
    };
    let author = match args.option("author").map(pubkey) {
        None => None,
        Some(Ok(author)) => Some(author),
        Some(Err(message)) => return output.usage(name, &message, usage),
    };
    let request = match (group, command.as_str()) {
        (_, "list") => match list_request(group, &args, author.as_deref(), limit) {
            Ok(request) => request,
            Err(message) => return output.usage(name, &message, usage),
        },
        (Group::Cap | Group::Prg, "describe") => {
            match describe_request(group, &args, author.as_deref()) {
                Ok(request) => request,
                Err(message) => return output.usage(name, &message, usage),
            }
        }
        (_, other) => return output.usage(name, &format!("unknown command `{other}`"), usage),
    };
    let signer = match signer_for(args.option("as")) {
        Ok(signer) => signer,
        Err(message) => return output.fail(name, &message),
    };
    let url = relay_url(args.option("relay"));
    let mut client = Client::connect(&url, signer);
    let mut events = Vec::new();
    let outcome = client.subscribe(vec![request.filter.clone()], false, timeout, |event| {
        events.push(event.clone());
    });
    if let Err(message) = outcome {
        client.close();
        return output.fail(name, &message);
    }
    let kind = request.filter["kinds"][0].as_u64().unwrap_or(0);
    let records = records(kind, events);
    match request.shape {
        Shape::List => {
            let all = args.switch("all");
            let items = if matches!(group, Group::Cap) {
                let now = unix_now();
                let authors: Vec<&str> = records.iter().map(|r| r.event.pubkey.as_str()).collect();
                let mut presence = Vec::new();
                if !authors.is_empty() {
                    let result = client.subscribe(
                        vec![json!({
                            "kinds": [verse::mv::STATE_KIND], "authors": authors,
                            "since": now.saturating_sub(RECENT_SECONDS), "until": now,
                        })],
                        false,
                        timeout,
                        |event| presence.push(event.clone()),
                    );
                    if let Err(message) = result {
                        client.close();
                        return output.fail(name, &message);
                    }
                }
                cap_rows(&records, &presence, now, all)
            } else {
                records.iter().map(|record| record.row(group)).collect()
            };
            client.close();
            output.emit(
                &json!({
                    "relay": url,
                    "kind": request.filter["kinds"][0],
                    "count": items.len(),
                    "all": all,
                    "items": items,
                }),
                |value| render_list(group, value),
            );
            0
        }
        Shape::Describe { author, slug } => {
            client.close();
            match records.into_iter().next() {
                Some(record) => {
                    output.emit(
                        &json!({
                            "relay": url,
                            "record": record.full(group),
                        }),
                        |value| render_describe(group, &value["record"]),
                    );
                    if record.valid { 0 } else { crate::EXIT_FAILURE }
                }
                None => output.fail(
                    name,
                    &format!("{url} has no kind {} head {author}:{slug}", group.kind()),
                ),
            }
        }
    }
}

const RECENT_SECONDS: u64 = 7 * 24 * 60 * 60;

// Publication is not presence. Only validated publisher entity states count;
// an offline state still establishes when the publisher was last seen.
fn last_seen(events: &[Event], now: u64) -> BTreeMap<String, u64> {
    let mut seen = BTreeMap::new();
    for event in events {
        if event.kind != verse::mv::STATE_KIND
            || event.created_at > now
            || event.created_at < now.saturating_sub(RECENT_SECONDS)
            || event.is_expired(now)
        {
            continue;
        }
        let Some(world) = event.tag_values("w").next() else {
            continue;
        };
        if let Ok(verse::mv::Received::State { state, .. }) = verse::mv::decode(event, world)
            && matches!(state.role.as_str(), "avatar" | "agent")
        {
            seen.entry(event.pubkey.clone())
                .and_modify(|t: &mut u64| *t = (*t).max(event.created_at))
                .or_insert(event.created_at);
        }
    }
    seen
}

fn cap_rows(records: &[Record], presence: &[Event], now: u64, all: bool) -> Vec<Value> {
    let seen = last_seen(presence, now);
    records
        .iter()
        .filter_map(|record| {
            let test = record
                .event
                .tag_values("t")
                .any(|t| matches!(t, "oa:test" | "oa:dev"));
            let last = seen.get(&record.event.pubkey);
            let old = last.is_none();
            if !all && (old || test) {
                return None;
            }
            let mut row = record.row(Group::Cap);
            row["old"] = json!(old);
            row["test"] = json!(test);
            row["last_seen"] = json!(last);
            Some(row)
        })
        .collect()
}

fn old_test(item: &Value) -> &'static str {
    match (
        item["old"].as_bool().unwrap_or(false),
        item["test"].as_bool().unwrap_or(false),
    ) {
        (true, true) => "old/test",
        (true, false) => "old",
        (false, true) => "test",
        _ => "-",
    }
}

enum Shape {
    List,
    Describe { author: String, slug: String },
}

struct Request {
    filter: Value,
    shape: Shape,
}

fn list_request(
    group: Group,
    args: &Args,
    author: Option<&str>,
    limit: u64,
) -> Result<Request, String> {
    let mut filter = serde_json::Map::new();
    filter.insert("limit".into(), json!(limit));
    if let Some(author) = author {
        filter.insert("authors".into(), json!([author]));
    }
    match group {
        Group::Cap => {
            filter.insert("kinds".into(), json!([group.kind()]));
            let mut markers = vec![group.marker().to_owned()];
            if let Some(profile) = args.option("profile") {
                let profile = match profile {
                    "native" | "executor" | "plugin" | "adapter" | "service" => profile,
                    other => {
                        return Err(format!(
                            "--profile takes native, executor, plugin, adapter, or service, not `{other}`"
                        ));
                    }
                };
                markers.push(format!("oa:profile:{profile}"));
            }
            filter.insert("#t".into(), json!(markers));
        }
        Group::Prg => {
            filter.insert("kinds".into(), json!([group.kind()]));
            let mut markers = vec![group.marker().to_owned()];
            if let Some(step) = args.option("step") {
                let step = match step {
                    "query" | "check" | "decide" | "delegate" | "program" | "module" | "invoke" => {
                        step
                    }
                    other => {
                        return Err(format!(
                            "--step takes query, check, decide, delegate, program, module, or invoke, not `{other}`"
                        ));
                    }
                };
                markers.push(format!("oa:step:{step}"));
            }
            filter.insert("#t".into(), json!(markers));
        }
        Group::Ext => {
            let record_type = args.option("type").unwrap_or("listing");
            let kind = match record_type {
                "listing" => nostr::ext::LISTING_KIND,
                "release" => nostr::ext::RELEASE_KIND,
                "revocation" => nostr::ext::REVOCATION_KIND,
                "migration" => nostr::ext::MIGRATION_KIND,
                "checkpoint" => nostr::ext::CHECKPOINT_KIND,
                other => {
                    return Err(format!(
                        "--type takes listing, release, revocation, migration, or checkpoint, not `{other}`"
                    ));
                }
            };
            filter.insert("kinds".into(), json!([kind]));
            filter.insert("#t".into(), json!([format!("oa:ext:{record_type}:v1")]));
            if let Some(package) = args.option("package") {
                let Some((root, slug)) = package.split_once(':') else {
                    return Err("--package takes ROOT_PUBKEY:SLUG".into());
                };
                let root = pubkey(root)?;
                if slug.is_empty() {
                    return Err("--package takes ROOT_PUBKEY:SLUG".into());
                }
                filter.insert("authors".into(), json!([root]));
                if kind == nostr::ext::LISTING_KIND || kind == nostr::ext::CHECKPOINT_KIND {
                    filter.insert("#d".into(), json!([slug]));
                }
            }
        }
    }
    Ok(Request {
        filter: Value::Object(filter),
        shape: Shape::List,
    })
}

fn describe_request(group: Group, args: &Args, author: Option<&str>) -> Result<Request, String> {
    let (author, slug) = match (args.positional().first(), author) {
        (Some(word), None) => {
            let Some((author, slug)) = word.split_once(':') else {
                return Err("describe takes PUBKEY:SLUG, or --author PUBKEY and SLUG".into());
            };
            (pubkey(author)?, slug.to_owned())
        }
        (Some(slug), Some(author)) => (author.to_owned(), slug.clone()),
        (None, _) => return Err("describe takes PUBKEY:SLUG, or --author PUBKEY and SLUG".into()),
    };
    if slug.is_empty() {
        return Err("the slug is empty".into());
    }
    Ok(Request {
        filter: json!({
            "kinds": [group.kind()],
            "authors": [author],
            "#d": [slug],
            "#t": [group.marker()],
            "limit": 8,
        }),
        shape: Shape::Describe { author, slug },
    })
}

/// A 64-character lowercase hex x-only pubkey.
fn pubkey(text: &str) -> Result<String, String> {
    let key = text.trim().to_ascii_lowercase();
    if key.len() == 64 && key.chars().all(|c| c.is_ascii_hexdigit()) {
        Ok(key)
    } else {
        Err(format!("`{text}` is not a 64-character hex public key"))
    }
}

/// One relay record with the contract's verdict attached.
struct Record {
    event: Event,
    /// The parsed body when the contract accepted it.
    body: Option<Value>,
    valid: bool,
    /// The contract's typed refusal when it did not.
    refusal: Option<String>,
    expired: bool,
}

impl Record {
    fn slug(&self) -> &str {
        self.event.tag_values("d").next().unwrap_or("")
    }

    fn markers(&self) -> Vec<&str> {
        self.event
            .tag_values("t")
            .filter(|value| value.starts_with("oa:"))
            .collect()
    }

    fn row(&self, group: Group) -> Value {
        let mut row = serde_json::Map::new();
        row.insert("id".into(), self.event.id.clone().into());
        row.insert("pubkey".into(), self.event.pubkey.clone().into());
        row.insert("kind".into(), self.event.kind.into());
        row.insert("created_at".into(), self.event.created_at.into());
        if let Some(slug) = self.event.tag_values("d").next() {
            row.insert("d".into(), slug.into());
        }
        row.insert("tags".into(), json!(self.markers()));
        row.insert("valid".into(), self.valid.into());
        if let Some(refusal) = &self.refusal {
            row.insert("refusal".into(), refusal.clone().into());
        }
        if self.expired {
            row.insert("expired".into(), true.into());
        }
        if let Some(body) = &self.body {
            match group {
                Group::Cap => {
                    copy(&mut row, body, &["id", "profile", "summary"]);
                    if body.get("definition").is_some() {
                        row.insert("definition".into(), body["definition"].clone());
                    }
                }
                Group::Prg => {
                    copy(&mut row, body, &["id", "summary"]);
                    if let Some(steps) = body["steps"].as_array() {
                        row.insert("steps".into(), steps.len().into());
                    }
                    if body.get("definition").is_some() {
                        row.insert("definition".into(), body["definition"].clone());
                    }
                }
                Group::Ext => {
                    copy(
                        &mut row,
                        body,
                        &[
                            "type", "package", "state", "title", "version", "release", "reason",
                            "revision", "from", "to", "role",
                        ],
                    );
                }
            }
        }
        Value::Object(row)
    }

    fn full(&self, group: Group) -> Value {
        let mut value = self.row(group);
        if let Some(object) = value.as_object_mut() {
            if let Some(body) = &self.body {
                object.insert("body".into(), body.clone());
            }
            object.insert(
                "event".into(),
                serde_json::to_value(&self.event).unwrap_or(Value::Null),
            );
        }
        value
    }
}

fn copy(row: &mut serde_json::Map<String, Value>, body: &Value, names: &[&str]) {
    for name in names {
        if let Some(value) = body.get(*name)
            && !value.is_null()
        {
            row.insert((*name).to_owned(), value.clone());
        }
    }
}

/// Check each event against its contract, keep the newest head per
/// signer and slug for addressable kinds, and order newest first.
fn records(kind: u64, events: Vec<Event>) -> Vec<Record> {
    let now = unix_now();
    let group = match kind {
        30_180 => Group::Cap,
        30_182 => Group::Prg,
        _ => Group::Ext,
    };
    let addressable = (30_000..40_000).contains(&kind);
    let mut newest: BTreeMap<(String, u16, String), Record> = BTreeMap::new();
    let mut regular = Vec::new();
    for event in events {
        let record = check(group, event, now);
        if addressable {
            let key = (
                record.event.pubkey.clone(),
                record.event.kind,
                record.slug().to_owned(),
            );
            match newest.get(&key) {
                Some(current) if current.event.created_at >= record.event.created_at => {}
                _ => {
                    newest.insert(key, record);
                }
            }
        } else {
            regular.push(record);
        }
    }
    let mut records: Vec<Record> = newest.into_values().chain(regular).collect();
    records.sort_by(|a, b| {
        b.event
            .created_at
            .cmp(&a.event.created_at)
            .then_with(|| a.event.id.cmp(&b.event.id))
    });
    records
}

fn check(group: Group, event: Event, now: u64) -> Record {
    let expired = event.is_expired(now);
    let (body, refusal) = match group {
        Group::Ext => match nostr::ext::parse_record(&event) {
            Ok(value) => (Some(value), None),
            Err(error) => (parse_loose(&event.content), Some(error.to_string())),
        },
        Group::Cap | Group::Prg => {
            let signature = event
                .validate_crypto()
                .map_err(|error| format!("signature: {error}"));
            let value = parse_loose(&event.content);
            let verdict = signature.and_then(|()| {
                let Some(value) = &value else {
                    return Err("body is not a JSON object".to_owned());
                };
                head_verdict(group, &event, value)
            });
            match verdict {
                Ok(()) => (value, None),
                Err(message) => (value, Some(message)),
            }
        }
    };
    Record {
        valid: refusal.is_none(),
        event,
        body,
        refusal,
        expired,
    }
}

/// A head is either a complete definition or `{v, requires, definition}`,
/// a reference this reader does not fetch. Both pass; the body's own
/// contract decides a complete definition.
fn head_verdict(group: Group, event: &Event, body: &Value) -> Result<(), String> {
    if event.tag_values("d").count() != 1 {
        return Err("head needs exactly one d tag".into());
    }
    if body.get("definition").is_some() {
        if body["v"] != json!(1) {
            return Err("head v must be 1".into());
        }
        return match body["definition"] {
            Value::Object(_) => Ok(()),
            _ => Err("definition reference must be an object".into()),
        };
    }
    match group {
        Group::Cap => nostr::cap::parse_definition(body)
            .and_then(|definition| nostr::cap::check_discovery_tags(&event.tags, &definition))
            .map_err(|error| error.to_string()),
        Group::Prg => nostr::prg::parse_definition(body)
            .and_then(|definition| nostr::prg::check_discovery_tags(&event.tags, &definition))
            .map_err(|error| error.to_string()),
        Group::Ext => Ok(()),
    }
}

fn parse_loose(content: &str) -> Option<Value> {
    match serde_json::from_str(content) {
        Ok(Value::Object(object)) => Some(Value::Object(object)),
        _ => None,
    }
}

fn short(key: &str) -> String {
    key.chars().take(12).collect()
}

fn text<'a>(value: &'a Value, name: &str) -> &'a str {
    value[name].as_str().unwrap_or("")
}

fn render_list(group: Group, value: &Value) -> String {
    let rows = render_list_rows(group, value);
    if matches!(group, Group::Ext) {
        crate::plugin_registry::published_list_text(text(value, "relay"), &rows)
    } else {
        rows
    }
}

fn render_list_rows(group: Group, value: &Value) -> String {
    let Some(items) = value["items"].as_array() else {
        return String::new();
    };
    if items.is_empty() {
        return format!(
            "no kind {} records on {}",
            value["kind"],
            text(value, "relay")
        );
    }
    let mut rows = Vec::new();
    match group {
        Group::Cap => {
            let all = value["all"].as_bool().unwrap_or(false);
            let mut header = row(&["SIGNER", "SLUG", "PROFILE", "VALID", "SUMMARY"]);
            if all {
                header.push("old/test".into());
            }
            rows.push(header);
            for item in items {
                let profile = text(item, "profile").to_owned();
                let profile = if profile.is_empty() {
                    item["tags"]
                        .as_array()
                        .and_then(|tags| {
                            tags.iter()
                                .filter_map(Value::as_str)
                                .find_map(|tag| tag.strip_prefix("oa:profile:"))
                        })
                        .unwrap_or("-")
                        .to_owned()
                } else {
                    profile
                };
                let mut cells = vec![
                    short(text(item, "pubkey")),
                    text(item, "d").to_owned(),
                    profile,
                    verdict(item),
                    summary(item),
                ];
                if all {
                    cells.push(old_test(item).into());
                }
                rows.push(cells);
            }
        }
        Group::Prg => {
            rows.push(row(&["SIGNER", "SLUG", "STEPS", "VALID", "SUMMARY"]));
            for item in items {
                rows.push(vec![
                    short(text(item, "pubkey")),
                    text(item, "d").to_owned(),
                    item["steps"]
                        .as_u64()
                        .map_or_else(|| "-".to_owned(), |n| n.to_string()),
                    verdict(item),
                    summary(item),
                ]);
            }
        }
        Group::Ext => {
            rows.push(row(&[
                "SIGNER", "TYPE", "PACKAGE", "STATE", "VALID", "TITLE",
            ]));
            for item in items {
                let state = [
                    text(item, "state"),
                    text(item, "version"),
                    text(item, "role"),
                ]
                .into_iter()
                .find(|s| !s.is_empty())
                .unwrap_or("-")
                .to_owned();
                let title = [text(item, "title"), text(item, "reason")]
                    .into_iter()
                    .find(|s| !s.is_empty())
                    .unwrap_or("")
                    .to_owned();
                rows.push(vec![
                    short(text(item, "pubkey")),
                    text(item, "type").to_owned(),
                    text(item, "package").to_owned(),
                    state,
                    verdict(item),
                    title,
                ]);
            }
        }
    }
    out::table(&rows)
}

fn row(cells: &[&str]) -> Vec<String> {
    cells.iter().map(|cell| (*cell).to_owned()).collect()
}

fn verdict(item: &Value) -> String {
    let mut word = if item["valid"].as_bool().unwrap_or(false) {
        "yes".to_owned()
    } else {
        "no".to_owned()
    };
    if item["expired"].as_bool().unwrap_or(false) {
        word.push_str(" (expired)");
    }
    word
}

fn summary(item: &Value) -> String {
    let summary = text(item, "summary");
    if summary.is_empty() && item.get("definition").is_some() {
        return "(definition by reference)".to_owned();
    }
    summary
        .chars()
        .take(60)
        .collect::<String>()
        .replace('\n', " ")
}

fn render_describe(group: Group, record: &Value) -> String {
    let mut lines = vec![
        format!("signer      {}", text(record, "pubkey")),
        format!("slug        {}", text(record, "d")),
        format!("event       {}", text(record, "id")),
        format!("created_at  {}", record["created_at"]),
        format!("valid       {}", verdict(record)),
    ];
    if let Some(refusal) = record["refusal"].as_str() {
        lines.push(format!("refusal     {refusal}"));
    }
    if let Some(tags) = record["tags"].as_array() {
        let tags: Vec<&str> = tags.iter().filter_map(Value::as_str).collect();
        lines.push(format!("tags        {}", tags.join(" ")));
    }
    let body = &record["body"];
    if body.is_object() {
        if let Some(id) = body["id"].as_str() {
            lines.push(format!("id          {id}"));
        }
        if let Some(summary) = body["summary"].as_str() {
            lines.push(format!("summary     {summary}"));
        }
        if body.get("definition").is_some() {
            lines.push(format!("definition  {}", body["definition"]));
        }
        match group {
            Group::Cap => {
                if let Some(profile) = body["profile"].as_str() {
                    lines.push(format!("profile     {profile}"));
                }
                if body.get("effects").is_some() {
                    lines.push(format!("effects     {}", body["effects"]));
                }
                if body.get("minimum").is_some() {
                    lines.push(format!("minimum     {}", body["minimum"]));
                }
                if body.get("binding_contract").is_some() {
                    lines.push(format!("binding     {}", body["binding_contract"]));
                }
                let x402 = &body["binding_contract"]["x402"];
                if x402.is_object() {
                    let bindings: Vec<&str> = x402["bindings"]
                        .as_array()
                        .map(|b| b.iter().filter_map(Value::as_str).collect())
                        .unwrap_or_default();
                    lines.push(format!(
                        "x402        {} merchant {} bindings {}",
                        x402["flow"].as_str().unwrap_or("?"),
                        x402["merchant"].as_str().unwrap_or("?"),
                        bindings.join(" ")
                    ));
                    for receiver in x402["receivers"].as_array().into_iter().flatten() {
                        lines.push(format!(
                            "receiver    {} on {}",
                            receiver["pay_to"].as_str().unwrap_or("?"),
                            receiver["network"].as_str().unwrap_or("?")
                        ));
                    }
                }
            }
            Group::Prg => {
                if let Some(steps) = body["steps"].as_array() {
                    lines.push(format!("steps       {}", steps.len()));
                    for step in steps {
                        lines.push(format!(
                            "  {:<24} {:<9} after={} on_error={} target={}",
                            text(step, "name"),
                            text(step, "kind"),
                            step["after"]
                                .as_array()
                                .map(|after| {
                                    after
                                        .iter()
                                        .filter_map(Value::as_str)
                                        .collect::<Vec<_>>()
                                        .join(",")
                                })
                                .filter(|s| !s.is_empty())
                                .unwrap_or_else(|| "-".to_owned()),
                            text(step, "on_error"),
                            step["target"]
                        ));
                    }
                }
            }
            Group::Ext => {}
        }
    } else {
        lines.push("body        (not a JSON object)".to_owned());
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use nostr::domain::{RelaySigner, Tag};

    fn words(text: &str) -> Vec<String> {
        text.split_whitespace().map(str::to_owned).collect()
    }

    fn signer() -> RelaySigner {
        RelaySigner::from_secret_hex(&"7".repeat(64)).unwrap()
    }

    fn head(signer: &RelaySigner, kind: u16, tags: &[(&str, &str)], body: &Value) -> Event {
        let tags = tags
            .iter()
            .map(|(name, value)| Tag::new(vec![(*name).to_owned(), (*value).to_owned()]))
            .collect();
        signer.sign(1_700_000_000, kind, tags, body.to_string())
    }

    fn presence(signer: &RelaySigner, time: u64, role: &str) -> Event {
        let state = verse::mv::State {
            v: 1,
            id: "avatar".into(),
            role: role.into(),
            p: [0.0; 3],
            q: [0.0, 0.0, 0.0, 1.0],
            t: time * 1000,
            online: false,
            follows: None,
            name: None,
            set: None,
            b: None,
        };
        verse::mv::state_event(signer, "test-world", &state, time)
    }

    #[test]
    fn cap_listing_filters_presence_at_seven_day_boundary() {
        let signer = signer();
        let now = 1_800_000_000;
        let records = records(
            30_180,
            vec![head(
                &signer,
                30_180,
                &[("d", "real"), ("t", "oa:cap:v1")],
                &json!({"v": 1, "definition": {"digest": "sha256:00"}}),
            )],
        );
        assert!(cap_rows(&records, &[], now, false).is_empty());
        for time in [now, now - RECENT_SECONDS] {
            assert_eq!(
                cap_rows(&records, &[presence(&signer, time, "avatar")], now, false).len(),
                1
            );
        }
        for time in [now + 1, now - RECENT_SECONDS - 1] {
            assert!(
                cap_rows(&records, &[presence(&signer, time, "avatar")], now, false).is_empty()
            );
        }
        let other = RelaySigner::from_secret_hex(&"8".repeat(64)).unwrap();
        let mut forged = presence(&signer, now, "avatar");
        forged.content.push(' ');
        for event in [
            forged,
            presence(&other, now, "avatar"),
            presence(&signer, now, "object"),
        ] {
            assert!(cap_rows(&records, &[event], now, false).is_empty());
        }
        assert_eq!(cap_rows(&records, &[], now, true)[0]["old"], true);
    }

    #[test]
    fn all_lists_test_and_dev_with_status_and_data_rows() {
        let signer = signer();
        let now = 1_800_000_000;
        let records = records(
            30_180,
            ["oa:test", "oa:dev"]
                .iter()
                .map(|tag| {
                    head(
                        &signer,
                        30_180,
                        &[("d", tag), ("t", "oa:cap:v1"), ("t", tag)],
                        &json!({"v": 1, "definition": {"digest": "sha256:00"}}),
                    )
                })
                .collect(),
        );
        let recent = vec![presence(&signer, now, "agent")];
        assert!(cap_rows(&records, &recent, now, false).is_empty());
        for (events, status) in [(&recent, "test"), (&Vec::new(), "old/test")] {
            let items = cap_rows(&records, events, now, true);
            assert_eq!(items.len(), 2);
            assert!(items.iter().all(|i| old_test(i) == status));
            let rendered = render_list(Group::Cap, &json!({"all": true, "items": items}));
            assert!(rendered.lines().next().unwrap().contains("old/test"));
            let data = rendered.lines().nth(1).unwrap();
            assert!(data.contains("oa:"));
            assert!(data.contains(status));
        }
    }

    #[test]
    fn cap_list_filter_carries_marker_profile_and_author() {
        let author = "a".repeat(64);
        let args = Args::parse(
            &words(&format!("--author {author} --profile service --limit 5")),
            &[],
        )
        .unwrap();
        let request = list_request(Group::Cap, &args, Some(&author), 5).unwrap();
        assert_eq!(request.filter["kinds"], json!([30_180]));
        assert_eq!(request.filter["authors"], json!([author]));
        assert_eq!(
            request.filter["#t"],
            json!(["oa:cap:v1", "oa:profile:service"])
        );
        assert_eq!(request.filter["limit"], json!(5));
    }

    #[test]
    fn cap_list_refuses_an_unknown_profile() {
        let args = Args::parse(&words("--profile shell"), &[]).unwrap();
        assert!(list_request(Group::Cap, &args, None, 10).is_err());
    }

    #[test]
    fn prg_list_filter_carries_step_marker() {
        let args = Args::parse(&words("--step delegate"), &[]).unwrap();
        let request = list_request(Group::Prg, &args, None, 10).unwrap();
        assert_eq!(request.filter["kinds"], json!([30_182]));
        assert_eq!(
            request.filter["#t"],
            json!(["oa:program:v1", "oa:step:delegate"])
        );
        let args = Args::parse(&words("--step shell"), &[]).unwrap();
        assert!(list_request(Group::Prg, &args, None, 10).is_err());
    }

    #[test]
    fn ext_list_selects_kind_by_type_and_scopes_by_package() {
        let root = "b".repeat(64);
        let args = Args::parse(
            &words(&format!("--type release --package {root}:tools")),
            &[],
        )
        .unwrap();
        let request = list_request(Group::Ext, &args, None, 10).unwrap();
        assert_eq!(request.filter["kinds"], json!([3_184]));
        assert_eq!(request.filter["#t"], json!(["oa:ext:release:v1"]));
        assert_eq!(request.filter["authors"], json!([root]));
        assert!(request.filter.get("#d").is_none());

        let args = Args::parse(&words(&format!("--package {root}:tools")), &[]).unwrap();
        let request = list_request(Group::Ext, &args, None, 10).unwrap();
        assert_eq!(request.filter["kinds"], json!([30_184]));
        assert_eq!(request.filter["#d"], json!(["tools"]));

        let args = Args::parse(&words("--type archive"), &[]).unwrap();
        assert!(list_request(Group::Ext, &args, None, 10).is_err());
        let args = Args::parse(&words("--package tools"), &[]).unwrap();
        assert!(list_request(Group::Ext, &args, None, 10).is_err());
    }

    #[test]
    fn describe_reads_pubkey_colon_slug_or_author_and_slug() {
        let author = "c".repeat(64);
        let args = Args::parse(&words(&format!("{author}:probe")), &[]).unwrap();
        let request = describe_request(Group::Cap, &args, None).unwrap();
        assert_eq!(request.filter["authors"], json!([author]));
        assert_eq!(request.filter["#d"], json!(["probe"]));
        assert_eq!(request.filter["#t"], json!(["oa:cap:v1"]));

        let args = Args::parse(&words("probe"), &[]).unwrap();
        let request = describe_request(Group::Prg, &args, Some(&author)).unwrap();
        assert_eq!(request.filter["kinds"], json!([30_182]));
        assert_eq!(request.filter["#d"], json!(["probe"]));

        let args = Args::parse(&words("probe"), &[]).unwrap();
        assert!(describe_request(Group::Cap, &args, None).is_err());
        let args = Args::parse(&words("short:probe"), &[]).unwrap();
        assert!(describe_request(Group::Cap, &args, None).is_err());
    }

    #[test]
    fn a_reference_head_passes_and_a_malformed_body_is_refused_not_dropped() {
        let signer = signer();
        let reference = head(
            &signer,
            30_180,
            &[
                ("d", "probe"),
                ("t", "oa:cap:v1"),
                ("t", "oa:profile:native"),
            ],
            &json!({"v": 1, "requires": [], "definition": {"digest": "sha256:00"}}),
        );
        let broken = head(
            &signer,
            30_180,
            &[("d", "broken"), ("t", "oa:cap:v1")],
            &json!({"v": 1, "requires": [], "id": "not-qualified"}),
        );
        let records = records(30_180, vec![broken, reference]);
        assert_eq!(records.len(), 2);
        let by_slug: BTreeMap<&str, &Record> = records.iter().map(|r| (r.slug(), r)).collect();
        assert!(by_slug["probe"].valid);
        assert!(by_slug["probe"].refusal.is_none());
        assert!(!by_slug["broken"].valid);
        assert!(by_slug["broken"].refusal.is_some());
        let row = by_slug["broken"].row(Group::Cap);
        assert_eq!(row["valid"], json!(false));
        assert!(row["refusal"].is_string());
    }

    #[test]
    fn newest_head_wins_per_signer_and_slug() {
        let signer = signer();
        let tags = [("d", "probe"), ("t", "oa:cap:v1")];
        let body = json!({"v": 1, "requires": [], "definition": {"digest": "sha256:00"}});
        let older = signer.sign(
            1_000,
            30_180,
            tags.iter()
                .map(|(n, v)| Tag::new(vec![(*n).to_owned(), (*v).to_owned()]))
                .collect(),
            body.to_string(),
        );
        let newer = signer.sign(
            2_000,
            30_180,
            tags.iter()
                .map(|(n, v)| Tag::new(vec![(*n).to_owned(), (*v).to_owned()]))
                .collect(),
            body.to_string(),
        );
        let records = records(30_180, vec![older, newer.clone()]);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].event.id, newer.id);
    }

    #[test]
    fn a_tampered_signature_is_refused() {
        let signer = signer();
        let mut event = head(
            &signer,
            30_182,
            &[("d", "flow"), ("t", "oa:program:v1")],
            &json!({"v": 1, "requires": [], "definition": {"digest": "sha256:00"}}),
        );
        event.content =
            json!({"v": 1, "requires": [], "definition": {"digest": "sha256:01"}}).to_string();
        let records = records(30_182, vec![event]);
        assert!(!records[0].valid);
        assert!(
            records[0]
                .refusal
                .as_deref()
                .unwrap()
                .starts_with("signature")
        );
    }

    #[test]
    fn list_and_describe_render_text() {
        let signer = signer();
        let event = head(
            &signer,
            30_180,
            &[
                ("d", "probe"),
                ("t", "oa:cap:v1"),
                ("t", "oa:profile:native"),
            ],
            &json!({"v": 1, "requires": [], "definition": {"digest": "sha256:00"}}),
        );
        let records = records(30_180, vec![event]);
        let items: Vec<Value> = records.iter().map(|r| r.row(Group::Cap)).collect();
        let text = render_list(
            Group::Cap,
            &json!({"relay": "wss://r", "kind": 30_180, "items": items}),
        );
        assert!(text.starts_with("SIGNER"));
        assert!(text.contains("probe"));
        assert!(text.contains("native"));
        assert!(text.contains("(definition by reference)"));
        let text = render_describe(Group::Cap, &records[0].full(Group::Cap));
        assert!(text.contains("slug        probe"));
        assert!(text.contains("valid       yes"));
        let empty = render_list(
            Group::Ext,
            &json!({"relay": "wss://r", "kind": 30_184, "items": []}),
        );
        assert!(empty.lines().next().unwrap().contains("Published plugins"));
        assert!(empty.contains("not your installed plugins"));
        assert!(empty.contains("openagents plugin installed"));
        assert!(empty.ends_with("no kind 30184 records on wss://r"));
        let populated = render_list(
            Group::Ext,
            &json!({
                "relay": "wss://r", "kind": 30184,
                "items": [{"package": "project-map", "title": "Project map", "valid": true}]
            }),
        );
        assert!(
            populated
                .lines()
                .next()
                .unwrap()
                .contains("Published plugins")
        );
        assert!(populated.contains("project-map"));
    }
}
