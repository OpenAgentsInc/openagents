//! Explicit local bundle transport. No command here opens a relay or model.

use super::*;
use knowledge::{private, snapshot};
use std::collections::BTreeSet;

pub(super) fn run(command: &str, o: &Options) -> Result<u8, String> {
    if command == "snapshot-check" {
        let verified = snapshot::read(Path::new(one(o)?))?;
        println!(
            "verified EXT release {}: {} candidate entries; no admission or inference",
            verified.release.id,
            verified.base.entries.len()
        );
        return Ok(0);
    }
    let key = o.key_file.clone().map_or_else(default_key, Ok)?;
    let identity = Identity::load_from(&key)?;
    match command {
        "snapshot-create" => {
            let docs: Vec<String> = entries(o)?.into_iter().map(|(_, s)| s).collect();
            let bundle = snapshot::create(
                &docs,
                identity.signer(),
                o.package.as_deref().ok_or("--package is required")?,
                o.snapshot_version
                    .as_deref()
                    .ok_or("--snapshot-version is required")?,
                o.license.as_deref().ok_or("--license is required")?,
                now(),
            )?;
            snapshot::write_new(output(o)?, &bundle)?;
            println!(
                "retained immutable EXT release {}; loading remains explicit",
                bundle.release.id
            );
        }
        "private-seal" => {
            let _ = one(o)?;
            let docs = entries(o)?;
            let (_, text) = docs.first().ok_or("entry not found")?;
            let key =
                remote::parse_author(o.recipient.as_deref().ok_or("--recipient is required")?)
                    .ok_or("invalid recipient")?;
            let recipient = key.parse().map_err(|_| "invalid recipient public key")?;
            let bundle = private::seal(
                text,
                identity.secret(),
                &recipient,
                now(),
                o.retain_until.ok_or("--retain-until is required")?,
            )?;
            snapshot::write_new(output(o)?, &bundle)?;
            println!(
                "retained encrypted 3188 declaration {}; deliver this bundle only to its recipient",
                bundle.declaration.id
            );
        }
        "private-show" => {
            let opened = private::read(Path::new(one(o)?), identity.secret())?;
            println!("{}\n{}", private::reference(&opened), opened.document);
        }
        "private-grant" => {
            let opened = private::read(Path::new(one(o)?), identity.secret())?;
            let recipients: BTreeSet<String> = o.model_recipients.iter().cloned().collect();
            let grant = private::Disclosure::new(&opened, recipients)?;
            snapshot::write_new(output(o)?, &grant)?;
            println!(
                "retained local model-disclosure permission for {}; no inference or publication",
                opened.event
            );
        }
        _ => return Err("unsupported bundle command".into()),
    }
    Ok(0)
}
fn one(o: &Options) -> Result<&str, String> {
    match o.words.as_slice() {
        [one] => Ok(one),
        _ => Err("this command needs exactly one entry or bundle".into()),
    }
}
fn output(o: &Options) -> Result<&Path, String> {
    o.output.as_deref().ok_or("--output is required".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn local_cli_snapshot_and_private_delivery_roundtrip_without_models_or_relay() {
        let root =
            std::env::temp_dir().join(format!("microcoder-bundle-cli-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let dir = root.join("entries");
        std::fs::create_dir(&dir).unwrap();
        let text = "---\nid: fixture.entry\nversion: 1\nkind: method\ntitle: Synthetic fixture\nsummary: A local command fixture.\ntags: [fixture]\napplies_when: Exercising immutable delivery.\nstatus: admitted\nauthor: Fixture\nprovenance:\n  written_from: [fixture-source]\n  cites: [https://example.invalid/reference]\nevidence: []\n---\n\nSynthetic retained content.\n";
        std::fs::write(dir.join("fixture.entry.md"), text).unwrap();
        let key = root.join("sender-key");
        let recipient_key = root.join("recipient-key");
        let recipient = Identity::load_from(&recipient_key).unwrap();
        let bundle = root.join("snapshot.json");
        let mut options = Options {
            dir,
            words: vec!["fixture.entry".into()],
            key_file: Some(key),
            output: Some(bundle.clone()),
            package: Some("fixture".into()),
            snapshot_version: Some("1".into()),
            license: Some("CC0".into()),
            ..Options::default()
        };
        assert_eq!(run("snapshot-create", &options).unwrap(), 0);
        let snapshot = snapshot::read(&bundle).unwrap();
        assert_eq!(snapshot.base.entries[0].id, "fixture.entry");
        let private_path = root.join("private.json");
        options.output = Some(private_path.clone());
        options.recipient = Some(recipient.pubkey().into());
        options.retain_until = Some(now() + 100);
        assert_eq!(run("private-seal", &options).unwrap(), 0);
        let opened = private::read(&private_path, recipient.secret()).unwrap();
        assert_eq!(opened.document, text);
        options.key_file = Some(recipient_key);
        options.words = vec![private_path.to_string_lossy().into_owned()];
        let grant = root.join("permission.json");
        options.output = Some(grant.clone());
        options.model_recipients = vec!["typesafe:https://example.invalid#fixture".into()];
        assert_eq!(run("private-grant", &options).unwrap(), 0);
        private::Disclosure::read(&grant)
            .unwrap()
            .check(&opened, &options.model_recipients.iter().cloned().collect())
            .unwrap();
        assert!(run("private-grant", &options).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
}
