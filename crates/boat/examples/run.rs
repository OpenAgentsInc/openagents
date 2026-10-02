//! What `scripts/boat-run.sh` does, through the SDK: reuse or create a
//! sandbox, upload a patch, run a detached command, poll it, print its output.
//!
//! cargo run -p boat --example run -- SANDBOX_ID_OR_NEW PATCH_FILE COMMAND
//!
//! This creates billable machine time. Stop the sandbox when you are done.

use std::time::Duration;

use boat::{Client, Nullable, WaitOptions, models::*};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), std::boxed::Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let (Some(target), Some(patch), Some(command)) = (args.next(), args.next(), args.next()) else {
        return Err("usage: run SANDBOX_ID|new PATCH_FILE COMMAND".into());
    };
    let client = Client::from_env().await?;
    let id = if target == "new" {
        let created = client
            .create(&CreateParams {
                idempotency_key: Some(format!("oa-boat-run-{}", std::process::id())),
                body: Some(CreateSandboxRequest {
                    type_: Some("large".into()),
                    ttl_seconds: Nullable::Value(14_400),
                    no_env: Some(true),
                    ..Default::default()
                }),
                ..Default::default()
            })
            .await?;
        created.sandbox.id
    } else {
        target
    };
    let wait = WaitOptions {
        timeout: Duration::from_secs(300),
        ..Default::default()
    };
    client.wait_until_ready(&id, &wait).await?;
    client
        .write_bytes(&id, "/tmp/oa.patch", &std::fs::read(patch)?)
        .await?;
    let script = format!(
        "set -e; cd ~; [ -d openagents/.git ] || git clone -q https://github.com/OpenAgentsInc/openagents.git; \
         cd openagents; git fetch -q origin; git reset -q --hard origin/main; git clean -qfd; \
         git apply --allow-empty /tmp/oa.patch; export CARGO_INCREMENTAL=0; {command}"
    );
    let process = client
        .exec_detached(
            &id,
            CommandRequest {
                command: script,
                ..Default::default()
            },
        )
        .await?;
    eprintln!("boat: {id}, process {}", process.process_id);
    let done = client
        .wait_command(
            &id,
            process.process_id,
            &WaitOptions {
                timeout: Duration::from_secs(4 * 3600),
                interval: Duration::from_secs(5),
                ..Default::default()
            },
        )
        .await?;
    print!("{}", done.stdout);
    eprint!("{}", done.stderr);
    std::process::exit(done.exit_code.unwrap_or(0) as i32);
}
