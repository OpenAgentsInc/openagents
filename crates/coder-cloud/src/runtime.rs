//! The same headless Coder runtime on Boat and GCE.
use crate::{Record, Result};
use serde_json::Value;
use std::collections::BTreeMap;

/// Values live only in memory and in a private remote file removed before dispatch.
#[derive(Default)]
pub struct Credentials {
    values: BTreeMap<String, String>,
    secrets: Vec<String>,
}
impl Credentials {
    pub fn from_names(names: &[String], get: impl Fn(&str) -> Option<String>) -> Result<Self> {
        let mut values = BTreeMap::new();
        for name in names {
            let value = get(name)
                .ok_or_else(|| format!("The selected credential {name} is unavailable."))?;
            if value.len() > 1024 * 1024 || value.contains('\0') {
                return Err("A selected credential has an invalid value.".into());
            }
            values.insert(name.clone(), value);
        }
        let mut secrets = Vec::new();
        for value in values.values() {
            if !value.is_empty() {
                secrets.push(value.clone());
                if let Ok(document) = serde_json::from_str::<Value>(value) {
                    credential_fragments(&document, &mut secrets);
                }
            }
        }
        secrets.sort_by_key(|v| std::cmp::Reverse(v.len()));
        secrets.dedup();
        Ok(Self { values, secrets })
    }
    pub fn sanitize_artifacts(&self, value: &mut Value) -> Result<()> {
        use base64::Engine;
        if let Some(files) = value["files"].as_array() {
            for file in files {
                let bytes = base64::engine::general_purpose::STANDARD
                    .decode(
                        file["content"]
                            .as_str()
                            .ok_or("Missing artifact file content.")?,
                    )
                    .map_err(|_| "Invalid artifact file encoding.")?;
                for key in &self.secrets {
                    if bytes.windows(key.len()).any(|part| part == key.as_bytes()) {
                        return Err("A changed remote file contains a selected credential. The artifact was refused.".into());
                    }
                }
            }
        }
        if let Some(trace) = value.get_mut("trace") {
            self.redact(trace);
        }
        Ok(())
    }
    pub fn environment(&self) -> BTreeMap<String, String> {
        self.values.clone()
    }
    pub fn shell(&self) -> String {
        self.values
            .iter()
            .map(|(k, v)| format!("export {k}={}\n", boat::shell_quote(v)))
            .collect()
    }
    pub fn redact(&self, value: &mut Value) {
        match value {
            Value::String(s) => {
                for key in &self.secrets {
                    *s = s.replace(key, "[redacted]");
                }
            }
            Value::Array(a) => {
                for v in a {
                    self.redact(v);
                }
            }
            Value::Object(o) => {
                for v in o.values_mut() {
                    self.redact(v);
                }
            }
            _ => {}
        }
    }
}

fn credential_fragments(value: &Value, secrets: &mut Vec<String>) {
    match value {
        Value::Object(fields) => {
            for (name, value) in fields {
                let name = name.to_ascii_lowercase();
                if name.contains("token") || name.contains("secret") || name.contains("key") {
                    if let Some(s) = value.as_str().filter(|s| !s.is_empty()) {
                        secrets.push(s.into());
                    }
                }
                credential_fragments(value, secrets);
            }
        }
        Value::Array(values) => {
            for value in values {
                credential_fragments(value, secrets);
            }
        }
        _ => {}
    }
}

pub fn directory(record: &Record) -> String {
    format!("/home/user/.oa-coder/jobs/{}", record.id)
}
/// Bind a remote directory before writing task files or credentials.
pub fn claim_script(r: &Record, dir: &str) -> String {
    let task = r
        .turns
        .first()
        .and_then(|v| v["task"].as_str())
        .unwrap_or(&r.spec.task);
    let identity=crate::workspace::digest(&serde_json::to_vec(&serde_json::json!({"cwd":r.spec.cwd,"agent":r.spec.agent,"mode":r.spec.mode,"task":task,"workspace":r.workspace.as_ref().map(|s|&s.input_digest)})).unwrap());
    format!(
        "set -eu; umask 077; d={}; mkdir -p \"$d\"; if [ ! -f \"$d/job-identity\" ]; then (set -C; printf %s {} > \"$d/job-identity\") 2>/dev/null || true; fi; [ \"$(cat \"$d/job-identity\")\" = {} ]",
        boat::shell_quote(dir),
        boat::shell_quote(&identity),
        boat::shell_quote(&identity)
    )
}
pub fn workdir(record: &Record, dir: &str) -> String {
    let mut path = std::path::PathBuf::from(dir).join("workspace");
    if let Some(s) = &record.workspace {
        path.push(&s.working_directory);
    }
    path.to_string_lossy().into_owned()
}
pub fn prepare_script(record: &Record, dir: &str) -> String {
    let model = record
        .spec
        .model
        .as_deref()
        .map(boat::shell_quote)
        .unwrap_or_default();
    format!(
        r#"set -eu
umask 077
d={dir}
mkdir -p "$d/workspace" "/tmp/oa-coder-{job}/state"
export PATH="$HOME/.local/bin:$HOME/.cargo/bin:/usr/local/bin:$PATH"
export CODEX_HOME="/tmp/oa-coder-{job}/codex"
mkdir -p "$CODEX_HOME"
if [ -f "/tmp/oa-coder-{job}.env" ]; then chmod 600 "/tmp/oa-coder-{job}.env"; . "/tmp/oa-coder-{job}.env"; fi
if [ -n "${{OA_CODEX_AUTH:-}}" ]; then mkdir -p "$CODEX_HOME"; printf '%s' "$OA_CODEX_AUTH" > "$CODEX_HOME/auth.json"; chmod 600 "$CODEX_HOME/auth.json"; unset OA_CODEX_AUTH; fi
if [ -n "${{OPENAI_API_KEY:-}}" ] && command -v codex >/dev/null 2>&1; then printf '%s' "$OPENAI_API_KEY" | codex login --with-api-key >/dev/null 2>&1; fi
"#,
        dir = boat::shell_quote(dir),
        job = record.id
    ) + &if record.spec.mode == crate::Mode::Coder {
        format!(
            r#"
p="${{OA_CODER_CLOUD_BINARY:-}}"
if [ -z "$p" ]; then
 for b in "$HOME/.oa-pool/bin/coder-cloud-runtime" "$HOME/.local/bin/coder-cloud-runtime" "$HOME/.openagents/bin/coder-cloud-runtime" "$HOME/.oa-pool/bin/openagents" "$HOME/.openagents/bin/openagents" "$HOME/.local/bin/openagents"; do [ ! -x "$b" ] || {{ p="$b"; break; }}; done
fi
if [ -z "$p" ]; then p=$(command -v coder-cloud-runtime || command -v openagents || true); fi
if [ -z "$p" ]; then p=$(find "$HOME/.openagents/targets" -path '*/debug/openagents' -type f 2>/dev/null | head -n 1 || true); fi
[ -n "$p" ] && [ -x "$p" ] || {{ echo 'The image lacks the headless Coder runtime.' >&2; exit 1; }}
"$p" coder --help | grep -q 'delegate AGENT' || {{ echo 'The image has an incompatible Coder runtime.' >&2; exit 1; }}
printf '%s' "$p" > "$d/binary"
"$p" --version > "$d/runtime-version"
if [ -n "${{OPENROUTER_API_KEY:-}}" ]; then "$p" coder --state "/tmp/oa-coder-{job}/state" plugins enable openrouter-byok >/dev/null; fi
{model_config}
"#,
            job = record.id,
            model_config = if model.is_empty() || record.spec.agent != "microcoder" {
                String::new()
            } else {
                format!(
                    "\"$p\" coder --state \"/tmp/oa-coder-{}/state\" models set {model} >/dev/null",
                    record.id
                ) + &record
                    .spec
                    .reasoning
                    .as_deref()
                    .map(|effort| format!(" --reasoning {}", boat::shell_quote(effort)))
                    .unwrap_or_default()
            }
        )
    } else {
        format!(
            "rm -f {}\n",
            boat::shell_quote(&format!("/tmp/oa-coder-{}.env", record.id))
        )
    }
}

pub fn launch_script(record: &Record, dir: &str) -> String {
    format!(
        r#"#!/bin/sh
set -u
umask 077
d={dir}
# A filesystem lock prevents a second dispatcher from starting this job.
exec 9>"$d/owner.lock"
flock -n 9 || exit 75
[ ! -f "$d/started" ] || exit 0
printf '%s' "$$" > "$d/pid"
awk '{{print $22}}' "/proc/$$/stat" > "$d/pid-start"
touch "$d/started"
p=$(cat "$d/binary")
export PATH="$HOME/.local/bin:$HOME/.cargo/bin:/usr/local/bin:$PATH"
export CODEX_HOME="/tmp/oa-coder-{job}/codex"
if [ -f "/tmp/oa-coder-{job}.env" ]; then . "/tmp/oa-coder-{job}.env"; rm -f "/tmp/oa-coder-{job}.env"; fi
unset OA_CODEX_AUTH
unset OPENAGENTS_CODER_EVENT_CHANNEL OPENAGENTS_CODER_MODEL_INPUT
{model_env}
export OA_CODER_CLOUD_CREDENTIAL_NAMES={credential_names}
"$p" --json coder --in {workdir} --state "/tmp/oa-coder-{job}/state" delegate {agent} --task "$(cat "$d/task")" --session {session} > "$d/out" 2> "$d/err"
rc=$?
printf '%s' "$rc" > "$d/exit.writing"
mv "$d/exit.writing" "$d/exit"
exit "$rc"
"#,
        dir = boat::shell_quote(dir),
        workdir = boat::shell_quote(&workdir(record, dir)),
        job = record.id,
        model_env = format!(
            "export CODER_CODEX_MODEL={}\nexport CODER_CODEX_REASONING={}\n",
            boat::shell_quote(record.spec.model.as_deref().unwrap_or("")),
            boat::shell_quote(record.spec.reasoning.as_deref().unwrap_or(""))
        ),
        credential_names = boat::shell_quote(&record.spec.credential_names.join(",")),
        agent = boat::shell_quote(&record.spec.agent),
        session = boat::shell_quote(&record.id)
    )
}

/// Read a bounded log slice and process evidence without starting work.
pub fn poll_script(dir: &str, offset: u64) -> String {
    format!(
        r#"python3 - {dir} {offset} <<'PY'
import sys,pathlib,json,base64,os
p=pathlib.Path(sys.argv[1]);offset=int(sys.argv[2]);data=b''
if (p/'out').exists():
 with (p/'out').open('rb') as f:
  f.seek(offset);data=f.read(128*1024)
code=int((p/'exit').read_text()) if (p/'exit').exists() else None
alive=False
if (p/'pid').exists():
 try:
  pid=int((p/'pid').read_text());stat=pathlib.Path(f'/proc/{{pid}}/stat').read_text().rsplit(')',1)[1].split()
  alive=stat[0]!='Z' and ((not (p/'pid-start').exists()) or stat[19]==(p/'pid-start').read_text().strip())
 except (OSError,ValueError):pass
print(json.dumps({{'data':base64.b64encode(data).decode(),'offset':offset,'exit':code,'alive':alive,'started':(p/'started').exists(),'more':(p/'out').exists() and (p/'out').stat().st_size>offset+len(data)}}))
PY"#,
        dir = boat::shell_quote(dir)
    )
}

pub fn parse_poll(record: &Record, body: &str) -> Result<crate::Observation> {
    use base64::Engine;
    let body: Value =
        serde_json::from_str(body).map_err(|_| "Cannot decode the remote process observation.")?;
    let offset = record
        .cursor
        .as_deref()
        .unwrap_or("0")
        .parse::<u64>()
        .map_err(|_| "Invalid remote output cursor.")?;
    if body["offset"].as_u64() != Some(offset) {
        return Err("Remote output cursor mismatch.".into());
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(body["data"].as_str().unwrap_or(""))
        .map_err(|_| "Invalid remote output encoding.")?;
    let consumed = bytes.iter().rposition(|b| *b == b'\n').map_or(0, |n| n + 1);
    if consumed == 0 && bytes.len() >= 128 * 1024 {
        return Err("A remote output line exceeds 128 KiB.".into());
    }
    let mut events = vec![];
    let mut result = None;
    for line in bytes[..consumed]
        .split(|b| *b == b'\n')
        .filter(|l| !l.is_empty())
    {
        let value: Value = serde_json::from_slice(line)
            .map_err(|_| "The remote runtime emitted invalid NDJSON.")?;
        if value.get("event").is_none() || value["event"] == "finished" {
            result = Some(value.clone());
        }
        events.push(value);
    }
    let mut end = None;
    if body["more"] == false && consumed == bytes.len() {
        if let Some(code) = body["exit"].as_i64() {
            if code == 0 {
                let result = result
                    .or_else(|| {
                        record.events[record.binding["turn_start"].as_u64().unwrap_or(0) as usize..]
                            .iter()
                            .rev()
                            .find(|v| v.get("event").is_none() || v["event"] == "finished")
                            .cloned()
                    })
                    .ok_or("The remote runtime exited without a result.")?;
                end = Some(Ok(observed_result(result)));
            } else {
                end = Some(Err(format!(
                    "The remote Coder runtime exited with status {code}."
                )));
            }
        } else if body["started"] == true && body["alive"] == false {
            end = Some(Err(
                "The remote runtime ended without an exit receipt.".into()
            ));
        }
    }
    Ok(crate::Observation {
        events,
        cursor: Some((offset + consumed as u64).to_string()),
        end,
    })
}

// Programmatic delegation wraps the native answer in `result`.
fn observed_result(mut value: Value) -> Value {
    for field in ["model", "tokens", "usage"] {
        if value.get(field).is_none() {
            if let Some(observed) = value.get("result").and_then(|v| v.get(field)).cloned() {
                value[field] = observed;
            }
        }
    }
    value
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn native_result_retains_the_observed_model_and_tokens() {
        let nested = json!({"reply":"done","model":"served-model","tokens":2680});
        let answer = observed_result(json!({"session":"job","reply":"done","result":nested}));
        assert_eq!(answer["model"], "served-model");
        assert_eq!(answer["tokens"], 2680);
        assert_eq!(answer["result"], nested);
        let direct = json!({"model":"direct","tokens":3,"result":nested});
        assert_eq!(observed_result(direct.clone()), direct);
    }
    #[test]
    fn structured_credentials_redact_tokens_and_refuse_token_artifacts() {
        use base64::Engine;
        let auth = json!({"auth_mode":"chatgpt","tokens":{"access_token":"access-secret","refresh_token":"refresh-secret"}}).to_string();
        let credentials =
            Credentials::from_names(&["OA_CODEX_AUTH".into()], |_| Some(auth.clone())).unwrap();
        let mut event = json!({"text":"access-secret refresh-secret"});
        credentials.redact(&mut event);
        assert_eq!(event["text"], "[redacted] [redacted]");
        let mut files = json!({"files":[{"content":base64::engine::general_purpose::STANDARD.encode(b"leaked refresh-secret")}]});
        assert!(credentials.sanitize_artifacts(&mut files).is_err());
        let mut safe = json!({"files":[{"content":base64::engine::general_purpose::STANDARD.encode(b"auth_mode chatgpt")}]});
        assert!(credentials.sanitize_artifacts(&mut safe).is_ok());
    }
    #[test]
    fn credentials_are_redacted_recursively_and_shell_quoted() {
        let c = Credentials::from_names(&["TEST_API_KEY".into()], |_| Some("secret'value".into()))
            .unwrap();
        let mut v = json!({"text":"secret'value","nested":["secret'value"]});
        c.redact(&mut v);
        assert_eq!(v, json!({"text":"[redacted]","nested":["[redacted]"]}));
        let output = std::process::Command::new("sh")
            .args(["-c", &format!("{}printf %s \"$TEST_API_KEY\"", c.shell())])
            .output()
            .unwrap();
        assert!(output.status.success());
        assert_eq!(output.stdout, b"secret'value");
    }
}

/// Stop the identified wrapper and its descendants, including separate child groups.
pub fn cancel_script(dir: &str) -> String {
    format!(
        r#"python3 - {} <<'PY'
import pathlib,os,signal,time,subprocess,sys
p=pathlib.Path(sys.argv[1])
def identity(pid):
 try:
  stat=pathlib.Path(f'/proc/{{pid}}/stat').read_text().rsplit(')',1)[1].split()
  return None if stat[0]=='Z' else stat[19]
 except (OSError,ValueError):return None
if (p/'pid').exists():
 root=int((p/'pid').read_text());start=identity(root)
 if start is not None:
  if not (p/'pid-start').exists() or (p/'pid-start').read_text().strip()!=start:raise SystemExit('Remote process identity cannot be verified')
  owned={{root:start}}
  def discover():
   table=[list(map(int,line.split())) for line in subprocess.run(['ps','-eo','pid=,ppid=,pgid='],capture_output=True,text=True,check=True).stdout.splitlines()]
   for _ in range(len(table)):
    added=False
    for pid,parent,group in table:
     if pid not in owned and (parent in owned or group==root):
      stamp=identity(pid)
      if stamp is not None:owned[pid]=stamp;added=True
    if not added:break
  def live():return [pid for pid,stamp in owned.items() if identity(pid)==stamp]
  discover()
  for sig in [signal.SIGTERM,signal.SIGKILL]:
   discover()
   for pid in reversed(live()):
    try:os.kill(pid,sig)
    except ProcessLookupError:pass
   for _ in range(30):
    if not live():break
    time.sleep(.1)
  if live():raise SystemExit('Remote process descendants remain alive')
print('stopped')
PY"#,
        boat::shell_quote(dir)
    )
}
