//! A forked chat's source provider session (#11963). The fork's first run
//! asks the harness to fork that session natively (Claude Code
//! `--fork-session`, Codex `thread/fork`) instead of seeding the copied
//! history into its first prompt. Cleared once the fork has its own session.
use crate::{DocError, SessionDoc};
use loro::{LoroValue, ValueOrContainer};
use openagents_proto::HarnessId;
use serde::{Deserialize, Serialize};

const KEY: &str = "openagentsNativeFork";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeFork {
    pub harness: HarnessId,
    /// The source chat's harness-native session id.
    pub session: String,
    /// The folder the source session was created in; harness session stores
    /// are folder-scoped.
    pub cwd: String,
}

pub fn get(doc: &SessionDoc) -> Option<NativeFork> {
    let ValueOrContainer::Value(LoroValue::String(value)) = doc.doc().get_map("meta").get(KEY)?
    else {
        return None;
    };
    serde_json::from_str(&value).ok()
}

pub fn set(doc: &SessionDoc, value: &NativeFork) -> Result<(), DocError> {
    doc.doc()
        .get_map("meta")
        .insert(KEY, serde_json::to_string(value)?)?;
    doc.doc().commit();
    Ok(())
}

pub fn clear(doc: &SessionDoc) -> Result<(), DocError> {
    let meta = doc.doc().get_map("meta");
    if meta.get(KEY).is_some() {
        meta.delete(KEY)?;
        doc.doc().commit();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_and_clears() {
        let doc = SessionDoc::init("chat").unwrap();
        assert_eq!(get(&doc), None);
        let fork = NativeFork {
            harness: HarnessId::ClaudeCode,
            session: "session-1".into(),
            cwd: "/repo".into(),
        };
        set(&doc, &fork).unwrap();
        assert_eq!(get(&doc), Some(fork));
        clear(&doc).unwrap();
        assert_eq!(get(&doc), None);
    }
}
