//! Fresh loopback-only synthetic fixture for isolated browser verification.
use coder::task::sales::{Store, intake};
use openagents_web::pilot::Configuration;
use std::path::PathBuf;

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [directory, origin] = args.as_slice() else {
        return Err("usage: pilot_fixture NEW_SCRATCH_DIRECTORY http://127.0.0.1:PORT".into());
    };
    if !origin
        .strip_prefix("http://127.0.0.1:")
        .is_some_and(|p| p.parse::<u16>().is_ok_and(|n| n > 0))
    {
        return Err("fixture requires a loopback origin".into());
    }
    let dir = PathBuf::from(directory);
    std::fs::create_dir(&dir).map_err(|_| "fixture directory must be new")?;
    let root = dir.join("tasks");
    let mut store = Store::open(&root)?;
    let owner_file = dir.join("owner");
    store.initialize("fixture-operator", &owner_file)?;
    let owner = store.authenticate(&Store::read_credential(&owner_file)?)?;
    let credential = dir.join("intake");
    store.issue_intake(
        &owner,
        intake::Policy {
            schema: intake::POLICY_SCHEMA.into(),
            id: "synthetic-browser-v1".into(),
            offer: intake::OFFER.into(),
            origin: origin.clone(),
            public_owner: "Synthetic fixture operator".into(),
            support_email: "operator@example.invalid".into(),
            commercial_approval: "fixture-only:synthetic-terms".into(),
            responsibility_acceptance: "fixture-only:synthetic-standing-review".into(),
            consent_version: "synthetic-email-review-v1".into(),
            expires_at: coder::task::sales::unix_now() + 86400,
            retention_seconds: 86400,
            review_within_seconds: 3600,
            max_leads: 4,
        },
        &credential,
    )?;
    let access = store.authenticate_intake(&Store::read_credential(&credential)?)?;
    let policy = store.intake_policy(&access)?;
    let bytes = serde_json::to_vec_pretty(&Configuration { root, credential })
        .map_err(|_| "fixture configuration failed")?;
    let config = dir.join("web.json");
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    use std::io::Write;
    options
        .open(&config)
        .map_err(|_| "fixture configuration failed")?
        .write_all(&bytes)
        .map_err(|_| "fixture configuration failed")?;
    let policy_bytes = serde_json::to_vec_pretty(&policy).map_err(|_| "fixture policy failed")?;
    options
        .open(dir.join("policy.json"))
        .map_err(|_| "fixture policy failed")?
        .write_all(&policy_bytes)
        .map_err(|_| "fixture policy failed")?;
    println!("{}", config.display());
    Ok(())
}
