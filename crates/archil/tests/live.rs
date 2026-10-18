//! Live integration test against the real Archil API.
//!
//! Ignored by default. Run it with real credentials:
//!
//! ```text
//! ARCHIL_API_KEY=... ARCHIL_REGION=aws-us-east-1 cargo test --test live -- --ignored --nocapture
//! ```
//!
//! It creates a disk and a sandbox and deletes both, even when an assertion fails.
//! It does nothing (and passes) unless both variables are set.

use std::panic::AssertUnwindSafe;

use futures_util::FutureExt;

use archil::{
    Archil, CreateDiskRequest, CreateSandboxRequest, ListDisksOptions, ListObjectsOptions,
    ProcessStartOptions, ProcessStatus, SandboxFileUploadOptions, SandboxStatus,
};

fn live_client() -> Option<Archil> {
    let key = std::env::var("ARCHIL_API_KEY")
        .ok()
        .filter(|v| !v.is_empty())?;
    let region = std::env::var("ARCHIL_REGION")
        .ok()
        .filter(|v| !v.is_empty())?;
    Some(Archil::new(archil::ClientOptions::new().api_key(key).region(region)).expect("client"))
}

fn unique(prefix: &str) -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("{prefix}-{}-{}", std::process::id(), nanos % 1_000_000_000)
}

#[tokio::test]
#[ignore = "needs ARCHIL_API_KEY and ARCHIL_REGION and creates real resources"]
async fn disks_objects_and_sandbox_round_trip() {
    let Some(client) = live_client() else {
        eprintln!("ARCHIL_API_KEY / ARCHIL_REGION not set; skipping");
        return;
    };

    let created = client
        .disks()
        .create(CreateDiskRequest::new(unique("sdk-live")))
        .await
        .expect("create disk");
    let disk = created.disk.clone();
    let mut sandbox = None;
    let outcome = AssertUnwindSafe(async {
        assert!(created.token.is_some(), "a mount token is generated");
        let found = client.disks().list(ListDisksOptions::default()).await?;
        assert!(found.iter().any(|d| d.id == disk.id));
        assert_eq!(client.disks().get(&disk.id).await?.name, disk.name);

        // Object API round trip.
        disk.put_object("live/hello.txt", "hello archil").await?;
        assert_eq!(
            &disk.get_object("live/hello.txt").await?[..],
            b"hello archil"
        );
        assert!(disk.object_exists("live/hello.txt").await?);
        let listing = disk
            .list_objects(Some("live/"), ListObjectsOptions::default())
            .await?;
        assert!(listing.objects.iter().any(|o| o.key == "live/hello.txt"));
        disk.append_object("live/log.txt", "a\n").await?;
        disk.append_object("live/log.txt", "b\n").await?;
        assert_eq!(&disk.get_object("live/log.txt").await?[..], b"a\nb\n");
        disk.delete_object("live/hello.txt").await?;
        assert!(!disk.object_exists("live/hello.txt").await?);

        // Serverless exec on the disk.
        let exec = disk.exec("cat /mnt/archil/live/log.txt").await?;
        assert_eq!(exec.exit_code, 0);

        // Sandbox: create, run, stream, files, ports, stop.
        let sb = sandbox.insert(
            client
                .sandboxes()
                .create(
                    // No disk mount: as of 2026-10-18 Archil fails to start a
                    // sandbox with an attached disk in aws-us-east-1.
                    CreateSandboxRequest::new().name(unique("sdk-live")),
                )
                .await?,
        );
        assert_eq!(sb.status, SandboxStatus::Running);
        let result = sb
            .exec("echo out; echo err >&2; exit 3", ProcessStartOptions::new())
            .await?;
        assert_eq!(
            (result.status, result.exit_code),
            (ProcessStatus::Completed, Some(3))
        );
        assert_eq!(result.stdout.trim(), "out");
        assert_eq!(result.stderr.trim(), "err");

        let mut proc = sb
            .run(
                "printf one; sleep 1; printf two",
                ProcessStartOptions::new().stream_output(true),
            )
            .await?;
        let mut streamed = String::new();
        while let Some(chunk) = proc.next_output().await {
            streamed.push_str(&String::from_utf8_lossy(&chunk.data));
        }
        assert_eq!(streamed, "onetwo");
        proc.wait().await?;

        sb.files()
            .upload_file(
                "from sdk",
                "/tmp/sdk-live/up.txt",
                SandboxFileUploadOptions::default(),
            )
            .await?;
        assert_eq!(
            sb.files().download_bytes("/tmp/sdk-live/up.txt").await?,
            b"from sdk"
        );
        let host = sb.expose_port(8080).await?;
        assert!(!host.is_empty());
        assert!(sb.list_ports().await?.iter().any(|p| p.port == 8080));
        sb.unexpose_port(8080).await?;

        sb.stop().await?;
        assert_eq!(sb.status, SandboxStatus::Stopped);
        Ok::<_, archil::Error>(())
    })
    .catch_unwind()
    .await;

    // Always clean up, then report the real failure, panics included.
    if let Some(sb) = sandbox.as_mut() {
        if sb.status == SandboxStatus::Running {
            let _ = sb.stop().await;
        }
        if let Err(e) = sb.delete().await {
            eprintln!("cleanup: deleting sandbox {} failed: {e}", sb.id);
        }
    }
    if let Err(e) = disk.delete().await {
        eprintln!("cleanup: deleting disk {} failed: {e}", disk.id);
    }
    match outcome {
        Ok(result) => result.expect("live round trip"),
        Err(panic) => std::panic::resume_unwind(panic),
    }
}
