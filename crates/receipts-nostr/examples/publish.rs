//! Sign one explicitly selected gateway receipt for public disclosure.
//! See NIP-AGENT for the upload endpoint and the fields disclosed.

use nostr::domain::{Event, RelaySigner};
use std::{
    env, fs,
    time::{SystemTime, UNIX_EPOCH},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().skip(1).collect();
    if args.len() != 2 {
        return Err("Usage: publish DEFINITION_EVENT.json SELECTED_RECEIPT.json; set NOSTR_SECRET_KEY to the owner key".into());
    }
    let definition: Event = serde_json::from_slice(&fs::read(&args[0])?)?;
    let receipt = receipts::ExecutionReceipt::parse(&fs::read_to_string(&args[1])?)?;
    let owner = RelaySigner::from_secret_hex(&env::var("NOSTR_SECRET_KEY")?)?;
    let at = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    let event = receipts_nostr::event(&owner, &definition, &receipt, at)?;
    println!("{}", serde_json::to_string(&event)?);
    Ok(())
}
