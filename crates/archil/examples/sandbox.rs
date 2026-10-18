//! A small operator CLI over Archil sandboxes, for trying images and
//! commands by hand.
//!
//! ```text
//! export ARCHIL_API_KEY=... ARCHIL_REGION=aws-us-east-1
//! cargo run -p archil --example sandbox -- create rust:bookworm 8 16384
//! cargo run -p archil --example sandbox -- exec SANDBOX 'uname -a; nproc'
//! cargo run -p archil --example sandbox -- fork SANDBOX
//! cargo run -p archil --example sandbox -- list
//! cargo run -p archil --example sandbox -- delete SANDBOX
//! ```

use std::io::Write;
use std::time::Instant;

use archil::{
    Archil, CreateSandboxRequest, ProcessStartOptions, ProcessStream, SandboxEgressPolicy,
    SandboxNetwork, SandboxNetworkAction, SandboxStatus,
};

#[tokio::main]
async fn main() -> archil::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let client = Archil::from_env()?;
    let began = Instant::now();
    match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["create", image, rest @ ..] => {
            let vcpu = rest.first().and_then(|v| v.parse().ok()).unwrap_or(4);
            let mem = rest.get(1).and_then(|v| v.parse().ok()).unwrap_or(8192);
            let sandbox = client
                .sandboxes()
                .create(
                    CreateSandboxRequest::new()
                        .base_image(*image)
                        .vcpu_count(vcpu)
                        .mem_size_mib(mem)
                        .idle_ttl_seconds(0),
                )
                .await?;
            println!("{} {}", sandbox.id, sandbox.status);
        }
        ["exec", id, command] => {
            let sandbox = client.sandboxes().get(id).await?;
            let mut process = sandbox
                .run(
                    command,
                    ProcessStartOptions::new()
                        .stream_output(true)
                        .collect_output(false)
                        .timeout_seconds(7200),
                )
                .await?;
            while let Some(chunk) = process.next_output().await {
                match chunk.stream {
                    ProcessStream::Stdout => std::io::stdout().write_all(&chunk.data).ok(),
                    ProcessStream::Stderr => std::io::stderr().write_all(&chunk.data).ok(),
                };
            }
            let result = process.wait().await?;
            eprintln!(
                "exit {:?} ({:?}) after {:.1}s",
                result.exit_code,
                result.exit_reason,
                began.elapsed().as_secs_f64()
            );
        }
        ["fork", id] => {
            let mut sandbox = client.sandboxes().get(id).await?;
            let fork = sandbox.fork().await?;
            println!("{} {}", fork.id, fork.status);
        }
        ["stop", id] => {
            let mut sandbox = client.sandboxes().get(id).await?;
            sandbox.stop().await?;
            println!("{} {}", sandbox.id, sandbox.status);
        }
        ["pause", id] => {
            let mut sandbox = client.sandboxes().get(id).await?;
            sandbox.pause().await?;
            println!("{} {}", sandbox.id, sandbox.status);
        }
        ["delete", id] => {
            let mut sandbox = client.sandboxes().get(id).await?;
            if !matches!(
                sandbox.status,
                SandboxStatus::Stopped | SandboxStatus::Exited
            ) {
                sandbox.stop().await?;
            }
            sandbox.delete().await?;
            println!("{id} deleted");
        }
        ["net", id, allow @ ..] => {
            let sandbox = client.sandboxes().get(id).await?;
            println!("{:?}", sandbox.get_network().await?);
            if !allow.is_empty() {
                let mut policy = SandboxEgressPolicy::new(if allow == ["*"] {
                    SandboxNetworkAction::Allow
                } else {
                    SandboxNetworkAction::Deny
                });
                if allow != ["*"] {
                    policy.allow = Some(allow.iter().map(|a| (*a).into()).collect());
                }
                let network = SandboxNetwork {
                    egress: Some(policy),
                    managed: None,
                };
                println!("{:?}", sandbox.update_network(&network).await?);
            }
        }
        ["list"] => {
            for sandbox in client.sandboxes().list(Default::default()).await? {
                println!("{} {} {}", sandbox.id, sandbox.status, sandbox.name);
            }
        }
        _ => {
            eprintln!(
                "usage: sandbox create IMAGE [VCPU MIB] | exec ID CMD | fork ID | stop ID | pause ID | delete ID | list"
            );
            std::process::exit(2);
        }
    }
    eprintln!("({:.1}s)", began.elapsed().as_secs_f64());
    Ok(())
}
