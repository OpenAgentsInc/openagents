//! The same headless Coder runtime on Boat and GCE.
use crate::{Record, Result};
use serde_json::Value;
use std::collections::BTreeMap;

/// Values live only in memory and in a private remote file removed before dispatch.
#[derive(Default)]
pub struct Credentials {
    values: BTreeMap<String, String>,
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
        Ok(Self { values })
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
                for key in self.values.values().filter(|k| !k.is_empty()) {
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

pub fn directory(record: &Record) -> String {
    format!("/home/user/.oa-coder/jobs/{}", record.id)
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
 for b in "$HOME/.oa-pool/bin/openagents" "$HOME/.openagents/bin/openagents" "$HOME/.local/bin/openagents"; do [ ! -x "$b" ] || {{ p="$b"; break; }}; done
fi
if [ -z "$p" ]; then p=$(command -v openagents || true); fi
if [ -z "$p" ]; then p=$(find "$HOME/.openagents/targets" -path '*/debug/openagents' -type f 2>/dev/null | head -n 1 || true); fi
[ -n "$p" ] && [ -x "$p" ] || {{ echo 'The image lacks the headless Coder runtime.' >&2; exit 1; }}
"$p" coder --help | grep -q 'delegate AGENT' || {{ echo 'The image has an incompatible Coder runtime.' >&2; exit 1; }}
printf '%s' "$p" > "$d/binary"
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
                )
            }
        )
    } else {
        String::new()
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
touch "$d/started"
p=$(cat "$d/binary")
export CODEX_HOME="/tmp/oa-coder-{job}/codex"
if [ -f "/tmp/oa-coder-{job}.env" ]; then . "/tmp/oa-coder-{job}.env"; rm -f "/tmp/oa-coder-{job}.env"; fi
unset OPENAGENTS_CODER_EVENT_CHANNEL OPENAGENTS_CODER_MODEL_INPUT
{model_env}
export OA_CODER_CLOUD_CREDENTIAL_NAMES={credential_names}
"$p" --json coder --in "$d/workspace" --state "/tmp/oa-coder-{job}/state" delegate {agent} --task "$(cat "$d/task")" --session {session} > "$d/out" 2> "$d/err"
rc=$?
printf '%s' "$rc" > "$d/exit.writing"
mv "$d/exit.writing" "$d/exit"
exit "$rc"
"#,
        dir = boat::shell_quote(dir),
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
 try:os.kill(int((p/'pid').read_text()),0);alive=True
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
                        record
                            .events
                            .iter()
                            .rev()
                            .find(|v| v.get("event").is_none() || v["event"] == "finished")
                            .cloned()
                    })
                    .ok_or("The remote runtime exited without a result.")?;
                end = Some(Ok(result));
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
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
