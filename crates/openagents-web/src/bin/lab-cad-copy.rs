//! Run only after the owner approves the selected CAD review checksum.
use std::collections::BTreeMap;

const USAGE: &str =
    "lab-cad-copy --bucket BUCKET --account ACCOUNT [--project PROJECT] --approved-checksum SHA256";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let mut options = BTreeMap::new();
    while let Some(name) = args.next() {
        if name == "--help" {
            println!("{USAGE}");
            return Ok(());
        }
        if !["--bucket", "--account", "--project", "--approved-checksum"].contains(&name.as_str()) {
            return Err(USAGE.into());
        }
        let value = args.next().ok_or(USAGE)?;
        if options.insert(name, value).is_some() {
            return Err(USAGE.into());
        }
    }
    let bucket = options.remove("--bucket").ok_or(USAGE)?;
    let account = options.remove("--account").ok_or(USAGE)?;
    let checksum = options.remove("--approved-checksum").ok_or(USAGE)?;
    let backend = openagents_web::labs_api::backend_from_env()
        .await?
        .ok_or("Configure the destination Labs database and bucket.")?;
    let source = openagents_web::chat_store::Store::gcs(bucket, "conversations".into())?;
    let result = openagents_web::cad_migration::copy(
        source,
        backend,
        &account,
        options.get("--project").map(String::as_str),
        &checksum,
    )
    .await?;
    println!("{}", serde_json::to_string(&result)?);
    Ok(())
}
