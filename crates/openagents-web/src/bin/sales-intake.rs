//! Local owner provisioning. The public server never receives this credential.
use coder::task::sales::{Store, intake::Policy};
use std::path::Path;

fn main() -> Result<(), String> {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    match arguments.as_slice() {
        [operation, root, owner_credential, policy_or_id, intake_credential] if operation == "grant" => {
            let policy: Policy = openagents_web::pilot::private_json(Path::new(policy_or_id))?;
            let mut store = Store::open(Path::new(root))?;
            let secret = Store::read_credential(Path::new(owner_credential))?;
            let owner = store.authenticate(&secret)?;
            store.issue_intake(&owner, policy, Path::new(intake_credential))?;
            println!("Create-only intake terms and standing human responsibility recorded.");
        }
        [operation, root, owner_credential, id] if operation == "revoke" => {
            let mut store = Store::open(Path::new(root))?;
            let secret = Store::read_credential(Path::new(owner_credential))?;
            let owner = store.authenticate(&secret)?;
            store.revoke_intake(&owner, id)?;
            println!("Intake capability revoked.");
        }
        _ => return Err("usage: sales-intake grant ROOT OWNER_CREDENTIAL PRIVATE_POLICY_JSON INTAKE_CREDENTIAL | sales-intake revoke ROOT OWNER_CREDENTIAL POLICY_ID".into()),
    }
    Ok(())
}
