//! `openagents gym`: the separately granted Gym connection (NIP-EVAL).
//!
//! `connect` verifies a `gym-connect:` code against the profile's key and
//! keeps it under `~/.openagents/gym/PROFILE.connection`; it never prints
//! the code back. `observe` reads the host's snapshot over the relay the
//! grant names. `launch` requests one recipe revision the grant admits, and
//! only with `--confirm`. Nothing here reads a source directory or runs a
//! command: the host admits sources and recipes, and this command sends the
//! recipe id, its exact revision, and a durable request id, nothing more.

use std::path::PathBuf;
use std::time::Duration;

use gym_bridge::{Client, Connection, Grant, Recipe, RelayPolicy};
use serde_json::{Value, json};

use crate::{Args, Output};

const USAGE: &str = "usage: openagents gym COMMAND [OPTIONS]
  connect --file FILE | --code CODE [--as PROFILE] [--relay URL]
                          Verify a gym-connect: code for this profile and keep it.
  observe --relay URL --timeout SECONDS [--as PROFILE]
                          Read the host's runs, enabled recipes, and notices.
  launch RECIPE_ID --confirm --relay URL --timeout SECONDS
      [--revision DIGEST] [--request-id ID] [--as PROFILE]
                          Ask the host to start one admitted recipe revision.
  status [--as PROFILE]   Show the kept connection's host, relay, and recipes.
  forget [--as PROFILE]   Delete the kept connection.
Options:
  --as PROFILE            The Verse profile whose key the grant was issued to.
  --relay URL             The relay to reach; it must be the one the grant names.
  --timeout SECONDS       Wall time allowed for the whole exchange.
  --confirm               Required for launch; without it nothing is sent.
  --request-id ID         Retry an earlier launch whose reply was lost.
The connection code is never printed. Recipes outside the grant are refused
before anything reaches the relay.";

pub fn run(output: &Output, words: &[String]) -> u8 {
    let Some((command, rest)) = words.split_first() else {
        return output.usage("gym", "a command is required", USAGE);
    };
    let args = match Args::parse(rest, &["confirm"]) {
        Ok(args) => args,
        Err(message) => return output.usage("gym", &message, USAGE),
    };
    match command.as_str() {
        "--help" | "-h" | "help" => {
            println!("{USAGE}");
            0
        }
        "connect" => connect(output, &args),
        "status" => match Kept::load(args.option("as")) {
            Ok(kept) => {
                output.emit(&kept.describe(), render_connection);
                0
            }
            Err(message) => output.fail("gym status", &message),
        },
        "forget" => {
            let path = connection_path(args.option("as"));
            match std::fs::remove_file(&path) {
                Ok(()) => {
                    output.emit(
                        &json!({ "forgotten": true, "path": path.display().to_string() }),
                        |value| format!("forgot {}", value["path"].as_str().unwrap_or("")),
                    );
                    0
                }
                Err(error) => output.fail("gym forget", &format!("{}: {error}", path.display())),
            }
        }
        "observe" => observe(output, &args),
        "launch" => launch(output, &args),
        other => output.usage("gym", &format!("unknown command `{other}`"), USAGE),
    }
}

pub fn home() -> PathBuf {
    if let Some(dir) = std::env::var_os("OPENAGENTS_GYM_HOME") {
        return PathBuf::from(dir);
    }
    let home = std::env::var_os("HOME").map_or_else(|| PathBuf::from("."), PathBuf::from);
    home.join(".openagents").join("gym")
}

fn profile_named(flag: Option<&str>) -> String {
    flag.map(str::to_owned)
        .or_else(|| std::env::var("OPENAGENTS_PROFILE").ok())
        .unwrap_or_else(|| "default".to_owned())
}

fn connection_path(profile: Option<&str>) -> PathBuf {
    home().join(format!("{}.connection", profile_named(profile)))
}

/// A kept connection, the key it was granted to, and the grant it verifies to.
struct Kept {
    profile: String,
    path: PathBuf,
    connection: Connection,
    grant: Grant,
    secret: secp256k1::SecretKey,
}

impl Kept {
    /// Verifies a code against the profile's key without touching a network.
    fn verify(profile: Option<&str>, code: &str) -> Result<Self, String> {
        let identity = crate::relay::identity_for(profile)?;
        let connection = Connection::parse(code.trim())
            .map_err(|_| "the Gym connection code is not valid".to_owned())?;
        let grant = connection
            .verify(
                &identity.secret,
                gym_bridge::unix_time().unwrap_or(0),
                RelayPolicy::Production,
            )
            .map_err(|error| {
                format!(
                    "the Gym connection is not usable by profile `{}`: {error}",
                    identity.profile
                )
            })?;
        Ok(Self {
            path: connection_path(Some(&identity.profile)),
            profile: identity.profile,
            connection,
            grant,
            secret: identity.secret,
        })
    }

    fn load(profile: Option<&str>) -> Result<Self, String> {
        let path = connection_path(profile);
        let code = std::fs::read_to_string(&path).map_err(|error| {
            format!(
                "{}: {error}; run `openagents gym connect` first",
                path.display()
            )
        })?;
        Self::verify(profile, &code)
    }

    fn keep(&self) -> Result<(), String> {
        let code = self
            .connection
            .encode()
            .map_err(|error| error.to_string())?;
        std::fs::create_dir_all(home())
            .map_err(|error| format!("{}: {error}", home().display()))?;
        write_private(&self.path, &code)
    }

    fn require_relay(&self, flag: Option<&str>) -> Result<(), String> {
        match flag {
            None => Err("--relay URL is required for every network operation".to_owned()),
            Some(relay) if relay == self.connection.relay => Ok(()),
            Some(relay) => Err(format!(
                "the grant names relay `{}`, not `{relay}`; a Gym grant is bound to one relay",
                self.connection.relay
            )),
        }
    }

    fn recipe(&self, id: &str) -> Result<&Recipe, String> {
        self.grant
            .recipes
            .iter()
            .find(|recipe| recipe.id == id)
            .ok_or_else(|| {
                format!(
                    "the grant does not admit recipe `{id}`; it admits {}",
                    if self.grant.recipes.is_empty() {
                        "no recipes".to_owned()
                    } else {
                        self.grant
                            .recipes
                            .iter()
                            .map(|recipe| format!("`{}`", recipe.id))
                            .collect::<Vec<_>>()
                            .join(", ")
                    }
                )
            })
    }

    fn describe(&self) -> Value {
        json!({
            "profile": self.profile,
            "path": self.path.display().to_string(),
            "host": self.connection.host,
            "client": self.connection.client,
            "relay": self.connection.relay,
            "grant": self.connection.grant,
            "observe": self.grant.observe,
            "sources_digest": self.grant.sources_digest,
            "issued_at": self.grant.issued_at,
            "expires_at": self.grant.expires_at,
            "recipes": self.grant.recipes,
        })
    }

    fn client(&self) -> Result<Client, String> {
        Client::new(self.connection.clone(), self.secret).map_err(|error| error.to_string())
    }
}

fn write_private(path: &PathBuf, text: &str) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(path)
            .map_err(|error| format!("{}: {error}", path.display()))?;
        file.write_all(text.as_bytes())
            .map_err(|error| format!("{}: {error}", path.display()))
    }
    #[cfg(not(unix))]
    {
        std::fs::write(path, text).map_err(|error| format!("{}: {error}", path.display()))
    }
}

fn timeout_of(args: &Args) -> Result<Duration, String> {
    let Some(text) = args.option("timeout") else {
        return Err("--timeout SECONDS is required for every network operation".to_owned());
    };
    text.parse::<u64>()
        .ok()
        .filter(|seconds| *seconds > 0)
        .map(Duration::from_secs)
        .ok_or_else(|| "--timeout must be a positive number of seconds".to_owned())
}

fn connect(output: &Output, args: &Args) -> u8 {
    let code = match (args.option("code"), args.option("file")) {
        (Some(_), Some(_)) => {
            return output.usage("gym connect", "pass --code or --file, not both", USAGE);
        }
        (Some(code), None) => code.to_owned(),
        (None, Some(file)) => match std::fs::read_to_string(file) {
            Ok(text) => text,
            Err(error) => return output.fail("gym connect", &format!("{file}: {error}")),
        },
        (None, None) => {
            return output.usage(
                "gym connect",
                "--code CODE or --file FILE is required",
                USAGE,
            );
        }
    };
    let kept = match Kept::verify(args.option("as"), &code) {
        Ok(kept) => kept,
        Err(message) => return output.fail("gym connect", &message),
    };
    if let Some(relay) = args.option("relay")
        && let Err(message) = kept.require_relay(Some(relay))
    {
        return output.fail("gym connect", &message);
    }
    if let Err(message) = kept.keep() {
        return output.fail("gym connect", &message);
    }
    let mut value = kept.describe();
    value["kept"] = Value::Bool(true);
    output.emit(&value, render_connection);
    0
}

fn observe(output: &Output, args: &Args) -> u8 {
    let kept = match Kept::load(args.option("as")) {
        Ok(kept) => kept,
        Err(message) => return output.fail("gym observe", &message),
    };
    if let Err(message) = kept.require_relay(args.option("relay")) {
        return output.usage("gym observe", &message, USAGE);
    }
    let timeout = match timeout_of(args) {
        Ok(timeout) => timeout,
        Err(message) => return output.usage("gym observe", &message, USAGE),
    };
    if !kept.grant.observe {
        return output.fail("gym observe", "this grant does not allow observation");
    }
    let client = match kept.client() {
        Ok(client) => client,
        Err(message) => return output.fail("gym observe", &message),
    };
    let snapshot = crate::runtime().block_on(async {
        tokio::time::timeout(timeout, client.snapshot())
            .await
            .map_err(|_| format!("no snapshot within {} seconds", timeout.as_secs()))
            .and_then(|result| result.map_err(|error| error.to_string()))
    });
    match snapshot {
        Ok(snapshot) => {
            let now = gym_bridge::unix_time().unwrap_or(u64::MAX);
            let stale = now.saturating_sub(snapshot.observed_at) > 30;
            output.emit(
                &json!({
                    "host": kept.connection.host,
                    "relay": kept.connection.relay,
                    "observed_at": snapshot.observed_at,
                    "stale": stale,
                    "runs": snapshot.runs,
                    "recipes": snapshot.recipes,
                    "notices": snapshot.notices,
                }),
                render_snapshot,
            );
            0
        }
        Err(message) => output.fail("gym observe", &message),
    }
}

fn launch(output: &Output, args: &Args) -> u8 {
    let Some(recipe_id) = args.positional().first() else {
        return output.usage("gym launch", "RECIPE_ID is required", USAGE);
    };
    let kept = match Kept::load(args.option("as")) {
        Ok(kept) => kept,
        Err(message) => return output.fail("gym launch", &message),
    };
    if let Err(message) = kept.require_relay(args.option("relay")) {
        return output.usage("gym launch", &message, USAGE);
    }
    let timeout = match timeout_of(args) {
        Ok(timeout) => timeout,
        Err(message) => return output.usage("gym launch", &message, USAGE),
    };
    let recipe = match kept.recipe(recipe_id) {
        Ok(recipe) => recipe.clone(),
        Err(message) => return output.fail("gym launch", &message),
    };
    if let Some(revision) = args.option("revision")
        && revision != recipe.revision
    {
        return output.fail(
            "gym launch",
            &format!(
                "the grant admits recipe `{}` at revision `{}`, not `{revision}`",
                recipe.id, recipe.revision
            ),
        );
    }
    if !args.switch("confirm") {
        let mut value = json!({ "recipe": recipe, "sent": false });
        value["error"] = "launch requires --confirm; nothing was sent".into();
        output.emit(&value, |value| {
            format!(
                "refused: {}\n{}",
                value["error"].as_str().unwrap_or(""),
                render_recipe(&value["recipe"])
            )
        });
        return crate::EXIT_FAILURE;
    }
    let request_id = args
        .option("request-id")
        .map_or_else(gym_bridge::new_request_id, str::to_owned);
    let client = match kept.client() {
        Ok(client) => client,
        Err(message) => return output.fail("gym launch", &message),
    };
    let outcome = crate::runtime().block_on(async {
        tokio::time::timeout(
            timeout,
            client.launch(&request_id, &recipe.id, &recipe.revision),
        )
        .await
    });
    let base = json!({
        "host": kept.connection.host,
        "relay": kept.connection.relay,
        "request_id": request_id,
        "recipe": recipe.id,
        "revision": recipe.revision,
    });
    match outcome {
        Ok(Ok(receipt)) => {
            let mut value = base;
            value["sent"] = Value::Bool(true);
            value["disposition"] = "accepted".into();
            value["receipt"] = serde_json::to_value(&receipt).unwrap_or(Value::Null);
            output.emit(&value, render_launch);
            0
        }
        Ok(Err(error)) => {
            let mut value = base;
            value["sent"] = Value::Bool(true);
            let confirmed = gym_bridge::confirmed_refusal(&error);
            value["disposition"] = if confirmed { "refused" } else { "unknown" }.into();
            value["error"] = error.to_string().into();
            if !confirmed {
                value["retry"] = format!(
                    "the host may have admitted this request; retry with --request-id {}",
                    value["request_id"].as_str().unwrap_or("")
                )
                .into();
            }
            output.emit(&value, render_launch);
            crate::EXIT_FAILURE
        }
        Err(_) => {
            let mut value = base;
            value["sent"] = Value::Bool(true);
            value["disposition"] = "unknown".into();
            value["error"] = format!("no reply within {} seconds", timeout.as_secs()).into();
            value["retry"] = format!(
                "the host may have admitted this request; retry with --request-id {}",
                value["request_id"].as_str().unwrap_or("")
            )
            .into();
            output.emit(&value, render_launch);
            crate::EXIT_FAILURE
        }
    }
}

fn text(value: &Value, key: &str) -> String {
    value[key].as_str().unwrap_or("").to_owned()
}

fn render_recipe(recipe: &Value) -> String {
    format!(
        "  {} {} rev {} wall {} ms, {} start(s){}",
        text(recipe, "id"),
        text(recipe, "title"),
        text(recipe, "revision"),
        recipe["budget"]["wall_ms"].as_u64().unwrap_or(0),
        recipe["budget"]["max_starts"].as_u64().unwrap_or(0),
        recipe["budget"]["spend_limit_usd"]
            .as_f64()
            .map(|usd| format!(", spend limit {usd} USD not enforced"))
            .unwrap_or_default(),
    )
}

fn render_connection(value: &Value) -> String {
    let mut lines = vec![
        format!(
            "profile {}  host {}  relay {}",
            text(value, "profile"),
            text(value, "host"),
            text(value, "relay")
        ),
        format!(
            "observe {}  expires_at {}  kept at {}",
            value["observe"].as_bool().unwrap_or(false),
            value["expires_at"].as_u64().unwrap_or(0),
            text(value, "path")
        ),
    ];
    let recipes = value["recipes"].as_array();
    if recipes.is_none_or(Vec::is_empty) {
        lines.push("recipes: none admitted".to_owned());
    } else {
        lines.push("recipes:".to_owned());
        lines.extend(recipes.into_iter().flatten().map(render_recipe));
    }
    lines.join("\n")
}

fn render_snapshot(value: &Value) -> String {
    let mut lines = vec![format!(
        "host {}  observed_at {}{}",
        text(value, "host"),
        value["observed_at"].as_u64().unwrap_or(0),
        if value["stale"].as_bool().unwrap_or(false) {
            "  stale"
        } else {
            ""
        }
    )];
    lines.push(format!(
        "runs ({}):",
        value["runs"].as_array().map_or(0, Vec::len)
    ));
    for run in value["runs"].as_array().into_iter().flatten() {
        lines.push(format!(
            "  {} {} [{}] {} {:.0}%",
            text(run, "id"),
            text(run, "title"),
            text(run, "category"),
            text(run, "status"),
            run["progress"].as_f64().unwrap_or(0.0) * 100.0
        ));
    }
    lines.push(format!(
        "recipes ({}):",
        value["recipes"].as_array().map_or(0, Vec::len)
    ));
    lines.extend(
        value["recipes"]
            .as_array()
            .into_iter()
            .flatten()
            .map(render_recipe),
    );
    for notice in value["notices"].as_array().into_iter().flatten() {
        lines.push(format!("notice: {}", notice.as_str().unwrap_or("")));
    }
    lines.join("\n")
}

fn render_launch(value: &Value) -> String {
    let mut line = format!(
        "{} request {} recipe {} rev {}",
        text(value, "disposition"),
        text(value, "request_id"),
        text(value, "recipe"),
        text(value, "revision")
    );
    if let Some(run) = value["receipt"]["run_id"].as_str() {
        line.push_str(&format!(
            " run {run} status {}",
            text(&value["receipt"], "status")
        ));
    }
    if let Some(error) = value["error"].as_str() {
        line.push_str(&format!("\n{error}"));
    }
    if let Some(retry) = value["retry"].as_str() {
        line.push_str(&format!("\n{retry}"));
    }
    line
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(list: &[&str]) -> Vec<String> {
        list.iter().map(|w| (*w).to_owned()).collect()
    }

    #[test]
    fn group_usage_is_exit_64() {
        let output = Output::new(true);
        assert_eq!(run(&output, &[]), crate::EXIT_USAGE);
        assert_eq!(run(&output, &words(&["nope"])), crate::EXIT_USAGE);
        assert_eq!(run(&output, &words(&["connect"])), crate::EXIT_USAGE);
        assert_eq!(
            run(&output, &words(&["connect", "--code", "x", "--file", "y"])),
            crate::EXIT_USAGE
        );
        assert_eq!(run(&output, &words(&["launch"])), crate::EXIT_USAGE);
    }

    #[test]
    fn timeout_is_required_and_positive() {
        let none = Args::parse(&[], &[]).unwrap();
        assert!(timeout_of(&none).unwrap_err().contains("--timeout"));
        let zero = Args::parse(&words(&["--timeout", "0"]), &[]).unwrap();
        assert!(timeout_of(&zero).is_err());
        let ok = Args::parse(&words(&["--timeout", "12"]), &[]).unwrap();
        assert_eq!(timeout_of(&ok).unwrap(), Duration::from_secs(12));
    }

    #[test]
    fn an_invalid_code_is_refused_without_a_network() {
        let dir = std::env::temp_dir().join(format!("oa-gym-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // SAFETY: tests in this module run in one process and set the same value.
        unsafe {
            std::env::set_var("VERSE_HOME", &dir);
            std::env::set_var("OPENAGENTS_GYM_HOME", &dir);
        }
        let error = Kept::verify(Some("gymtest"), "gym-connect:not-a-code")
            .map(drop)
            .unwrap_err();
        assert!(error.contains("not valid"));
        assert!(!dir.join("gymtest.connection").exists());
        let output = Output::new(true);
        assert_eq!(
            run(
                &output,
                &words(&["connect", "--as", "gymtest", "--code", "gym-connect:nope"])
            ),
            crate::EXIT_FAILURE
        );
        assert_eq!(
            run(
                &output,
                &words(&[
                    "launch",
                    "r1",
                    "--as",
                    "gymtest",
                    "--confirm",
                    "--relay",
                    "wss://x",
                    "--timeout",
                    "1"
                ])
            ),
            crate::EXIT_FAILURE
        );
    }
}
