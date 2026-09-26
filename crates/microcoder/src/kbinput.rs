//! Explicit pinned knowledge inputs. Loading never fetches, admits an entry,
//! calls a model, or merges ambient local/relay knowledge into a selected bundle.

use std::collections::BTreeSet;
use std::path::Path;

use coder::relay::Identity;
use knowledge::{Base, private, snapshot};
use serde_json::{Value, json};

/// Checked inputs and the original bytes to retain with the run.
pub struct Input {
    pub base: Base,
    pub provenance: Value,
    pub retained: Value,
    pub private: bool,
}

/// Select an explicit snapshot or encrypted delivery, or use the ordinary loader.
///
/// # Errors
/// Refuses ambiguous selections, implicit admission, incomplete private grants,
/// and mismatched identity or model recipients. The key is never serialized.
pub fn load(
    snapshot_path: Option<&Path>,
    private_path: Option<&Path>,
    grant_path: Option<&Path>,
    key_path: Option<&Path>,
    models: &BTreeSet<String>,
    mode: &str,
) -> Result<Option<Input>, String> {
    if snapshot_path.is_some() && private_path.is_some() {
        return Err("choose either --kb-snapshot or --kb-private".into());
    }
    if private_path.is_none() && grant_path.is_some() {
        return Err("--kb-private-grant needs --kb-private".into());
    }
    if snapshot_path.is_none() && private_path.is_none() {
        return Ok(None);
    }
    if mode != "candidates" {
        return Err(
            "pinned knowledge is not local admission; explicitly use --kb candidates".into(),
        );
    }
    if let Some(path) = snapshot_path {
        let verified = snapshot::read(path)?;
        let retained = read_bounded_json(path)?;
        // Recheck the retained copy so a file replacement cannot change the run's pin.
        let bundle: snapshot::Bundle =
            serde_json::from_value(retained.clone()).map_err(|e| e.to_string())?;
        let second = snapshot::verify(&bundle)?;
        if second.release.id != verified.release.id {
            return Err("snapshot changed while loading".into());
        }
        return Ok(Some(Input {
            base: second.base,
            provenance: json!({"kind":"ext-snapshot","release":second.release,
            "manifest_digest":knowledge::digest(&nostr::contracts::jcs(&second.manifest).map_err(|e|e.to_string())?),
            "revocation":"explicit offline pin; no freshness claim"}),
            retained,
            private: false,
        }));
    }
    let path = private_path.ok_or("missing private input")?;
    let key = key_path
        .map(Path::to_path_buf)
        .or_else(knowledge::remote::key_file)
        .ok_or("no private reader key")?;
    if !key.is_file() {
        return Err("private reader key must already exist".into());
    }
    let identity = Identity::load_from(&key)?;
    let retained = read_bounded_json(path)?;
    let bundle: private::Bundle =
        serde_json::from_value(retained.clone()).map_err(|e| e.to_string())?;
    let opened = private::open(&bundle, identity.secret())?;
    let grant = private::Disclosure::read(grant_path.ok_or("--kb-private-grant is required")?)?;
    grant.check(&opened, models)?;
    let provenance = json!({"kind":"private-3188","artifact":private::reference(&opened),"model_disclosure":grant,
        "embeddings":"disabled for private input"});
    Ok(Some(Input {
        base: Base {
            entries: vec![opened.entry],
        },
        provenance,
        retained: json!({"bundle":retained,"model_disclosure":grant}),
        private: true,
    }))
}
fn read_bounded_json(path: &Path) -> Result<Value, String> {
    use std::io::Read;
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(snapshot::MAX_BUNDLE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > snapshot::MAX_BUNDLE_BYTES {
        return Err("knowledge input exceeds byte limit".into());
    }
    nostr::contracts::parse_strict(&bytes).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn scratch(name: &str) -> std::path::PathBuf {
        let path =
            std::env::temp_dir().join(format!("microcoder-kbinput-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir(&path).unwrap();
        path
    }
    fn document() -> String {
        "---\nid: fixture.only\nversion: 1\nkind: method\ntitle: Fixture only\nsummary: Synthetic selected reference.\ntags: [fixture]\napplies_when: Reading a frozen fixture.\nstatus: admitted\nauthor: Fixture\nprovenance:\n  written_from: [fixture-source]\n  cites: [https://example.invalid/ref]\nevidence: []\n---\n\nExact fixture bytes.\n".into()
    }
    #[test]
    fn explicit_snapshot_replaces_ambient_sources_and_retains_its_pin() {
        let dir = scratch("snapshot");
        let path = dir.join("bundle.json");
        let signer = nostr::domain::RelaySigner::from_secret_hex(&"12".repeat(32)).unwrap();
        let bundle = snapshot::create(&[document()], &signer, "fixture", "1", "CC0", 10).unwrap();
        snapshot::write_new(&path, &bundle).unwrap();
        let models = BTreeSet::new();
        assert!(load(Some(&path), None, None, None, &models, "on").is_err());
        let input = load(Some(&path), None, None, None, &models, "candidates")
            .unwrap()
            .unwrap();
        assert_eq!(input.base.entries.len(), 1);
        assert_eq!(input.base.entries[0].id, "fixture.only");
        assert_eq!(input.provenance["release"]["id"], bundle.release.id);
        let retained: snapshot::Bundle = serde_json::from_value(input.retained).unwrap();
        snapshot::verify(&retained).unwrap();
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn private_loading_requires_every_actual_model_and_retains_no_plaintext() {
        let dir = scratch("private");
        let key = dir.join("key");
        let file = dir.join("private.json");
        let permission = dir.join("grant.json");
        let recipient = Identity::load_from(&key).unwrap();
        let sender = Identity::from_secret("13".repeat(32).parse().unwrap()).unwrap();
        let bundle = private::seal(
            &document(),
            sender.secret(),
            &recipient.pubkey().parse().unwrap(),
            10,
            20,
        )
        .unwrap();
        snapshot::write_new(&file, &bundle).unwrap();
        let opened = private::open(&bundle, recipient.secret()).unwrap();
        let models = BTreeSet::from([
            "provider:https://example.invalid#generator".into(),
            "provider:https://example.invalid#judge".into(),
        ]);
        let grant = private::Disclosure::new(&opened, models.clone()).unwrap();
        snapshot::write_new(&permission, &grant).unwrap();
        assert!(load(None, Some(&file), None, Some(&key), &models, "candidates").is_err());
        let input = load(
            None,
            Some(&file),
            Some(&permission),
            Some(&key),
            &models,
            "candidates",
        )
        .unwrap()
        .unwrap();
        assert!(input.private);
        assert_eq!(input.base.entries.len(), 1);
        assert!(!input.retained.to_string().contains("Exact fixture bytes"));
        let changed = BTreeSet::from(["provider:https://example.invalid#different".into()]);
        assert!(
            load(
                None,
                Some(&file),
                Some(&permission),
                Some(&key),
                &changed,
                "candidates"
            )
            .is_err()
        );
        assert!(
            load(
                Some(&file),
                Some(&file),
                Some(&permission),
                Some(&key),
                &models,
                "candidates"
            )
            .is_err()
        );
        std::fs::remove_file(&permission).unwrap();
        assert!(
            load(
                None,
                Some(&file),
                Some(&permission),
                Some(&key),
                &models,
                "candidates"
            )
            .is_err()
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
}
