//! Retained workbench views over the existing plugin authoring engines.

use std::path::PathBuf;

use openagents_chat::plugin_workbench::{Declarations, Owner, Request};
use serde::Deserialize;
use serde_json::json;

use crate::{Args, Output};

const USAGE: &str = "usage: openagents plugin workbench freeze|show|approve --root DIR\n  freeze --review FILE: retain a reviewed draft and its original flow/task.\n  show: read the kept flow, tests, comparisons, and action outcomes.\n  approve --request FILE: execute one exact reviewed action.\nReview and request files are typed JSON. Reading a flow starts nothing.";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Review {
    flow: String,
    snapshot: openagents_chat::service::Snapshot,
    directory: PathBuf,
    declarations: Declarations,
}

pub fn run(output: &Output, words: &[String]) -> Option<u8> {
    if words.first().is_none_or(|w| w != "workbench") {
        return None;
    }
    if words.get(1).is_none_or(|w| w == "--help" || w == "-h") {
        println!("{USAGE}");
        return Some(0);
    }
    let result: Result<serde_json::Value, String> = (|| {
        let args = Args::parse(&words[2..], &[])?;
        let root = PathBuf::from(
            args.option("root")
                .ok_or("A workbench flow needs an explicit private --root DIR.")?,
        );
        if !root.is_absolute() {
            return Err("The flow root must be absolute.".into());
        }
        let owner = Owner::open(root, coder::task::chat_client::Here);
        let value = match words[1].as_str() {
            "freeze" => {
                let review: Review = read(
                    args.option("review")
                        .ok_or("Freezing needs --review FILE.")?,
                )?;
                let record = owner.freeze_routed(
                    review.flow,
                    &review.snapshot,
                    &review.directory,
                    review.declarations,
                )?;
                json!({"record": record, "text": "Retained the reviewed draft. Nothing was run, published, installed, or enabled."})
            }
            "show" => {
                let record = owner.read()?;
                let draft = owner.reviewed_files()?.into_iter().map(|(path, bytes)| {
                    json!({"path":path, "digest":route_contract::digest::Digest::of_bytes(&bytes),
                        "bytes":bytes.len(), "content":String::from_utf8(bytes).ok()})
                }).collect::<Vec<_>>();
                let value = json!({"record":record, "draft":draft});
                let text = serde_json::to_string_pretty(&value).map_err(|e| e.to_string())?;
                json!({"record":value["record"], "draft":value["draft"], "text":text})
            }
            "approve" => {
                let request: Request = read(args.option("request").ok_or(
                    "Approval needs --request FILE naming the exact release and action.",
                )?)?;
                let outcome = owner.apply(request)?;
                let text = serde_json::to_string_pretty(&outcome).map_err(|e| e.to_string())?;
                json!({"outcome": outcome, "text": text})
            }
            _ => return Err(USAGE.into()),
        };
        Ok(value)
    })();
    Some(match result {
        Ok(value) => {
            output.emit(&value, |v| {
                v["text"].as_str().unwrap_or_default().to_owned()
            });
            0
        }
        Err(message) => output.fail("plugin workbench", &message),
    })
}

fn read<T: serde::de::DeserializeOwned>(path: &str) -> Result<T, String> {
    let meta = std::fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if !meta.is_file() || meta.len() > 64 * 1024 {
        return Err("The review or request file is unsafe or too large.".into());
    }
    serde_json::from_slice(&std::fs::read(path).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests;
