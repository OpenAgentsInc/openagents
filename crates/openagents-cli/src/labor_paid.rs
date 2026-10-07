//! Explicit private adapters for the admitted postacceptance coding lane.
use super::*;
use coder_labor::paid::{Authority, PipelineAuthority, private_document};
use std::os::unix::fs::{MetadataExt, PermissionsExt};

pub(super) fn with_authority<T>(
    args: &Args,
    operation: impl FnOnce(&mut dyn Authority) -> Result<T, String>,
) -> Result<T, String> {
    let pipeline = PathBuf::from(
        args.option("pipeline")
            .ok_or("paid fulfillment requires --pipeline DIR")?,
    );
    let credential = PathBuf::from(
        args.option("credential")
            .ok_or("paid fulfillment requires --credential FILE")?,
    );
    let file = args
        .option("authority-evidence")
        .ok_or("paid fulfillment requires --authority-evidence FILE")?;
    let evidence: Blobs =
        serde_json::from_slice(&private_document(Path::new(file), 8 * 1024 * 1024)?)
            .map_err(|_| "authority evidence must be a bounded private artifact closure")?;
    let mut authority = PipelineAuthority {
        root: &pipeline,
        credential: &credential,
        evidence: &evidence,
    };
    operation(&mut authority)
}
fn resident(args: &Args) -> Result<openagents_wallet::resident::RemoteWallet, String> {
    let home = args
        .option("wallet-home")
        .ok_or("paid funding requires an explicit --wallet-home DIR")?;
    let home = Path::new(home);
    let metadata =
        std::fs::symlink_metadata(home).map_err(|_| "explicit resident wallet home unavailable")?;
    if !metadata.is_dir() || metadata.permissions().mode() & 0o077 != 0 {
        return Err("resident wallet home must be an existing private directory".into());
    }
    openagents_wallet::resident::RemoteWallet::probe(home).ok_or_else(|| {
        "the explicit resident wallet is unavailable; no replacement node was opened".into()
    })
}
fn ledger_path(args: &Args) -> Result<PathBuf, String> {
    let path = PathBuf::from(
        args.option("ledger")
            .ok_or("paid accrual requires --ledger FILE")?,
    );
    let parent = path.parent().ok_or("central ledger parent unavailable")?;
    let meta =
        std::fs::symlink_metadata(parent).map_err(|_| "central ledger parent unavailable")?;
    if !meta.is_dir() || meta.permissions().mode() & 0o077 != 0 {
        return Err("central ledger must remain in an explicit private directory".into());
    }
    let meta = std::fs::symlink_metadata(&path)
        .map_err(|_| "the existing central ledger is unavailable")?;
    if !meta.is_file() || meta.nlink() != 1 || meta.permissions().mode() & 0o077 != 0 {
        return Err("central ledger must be an ordinary private file".into());
    }
    Ok(path)
}
pub(super) fn run(
    output: &Output,
    command: &str,
    args: &Args,
    secret: &secp256k1::SecretKey,
) -> u8 {
    let Some(name) = args.positional().first() else {
        return output.usage("labor", "NAME is required", USAGE);
    };
    let result = (|| -> Result<Value, String> {
        let dir = book_dir(name)?;
        let setup = read_setup(&dir)?;
        if setup.paid.is_none() {
            return Err(
                "this command requires the explicitly admitted paid fulfillment profile".into(),
            );
        }
        if command == "support" {
            let source = args
                .positional()
                .get(1)
                .ok_or("support requires one signed EVENT file")?;
            if args.positional().len() != 2 {
                return Err("support requires exactly one signed EVENT file".into());
            }
            let event: Event =
                serde_json::from_slice(&private_document(Path::new(source), 1024 * 1024)?)
                    .map_err(|_| "malformed signed support event")?;
            let mut attachments = Blobs::default();
            for file in args.options("attach") {
                let value: Value =
                    serde_json::from_slice(&private_document(Path::new(file), 1024 * 1024)?)
                        .map_err(|_| "malformed support artifact")?;
                attachments.0.insert(
                    nostr::contracts::digest_bytes(
                        &nostr::contracts::jcs(&value).map_err(|e| e.to_string())?,
                    ),
                    value,
                );
            }
            match open_store(&dir, setup.clone(), secret) {
                Ok(mut store) => {
                    let outcome = store.receive(event, now(), attachments)?;
                    if outcome.starts_with("refused:") {
                        return Err(outcome);
                    }
                    Ok(json!({"name":name,"outcome":outcome,"paid":store.paid_report()?}))
                }
                Err(error) if error == "labor store is busy" => {
                    Store::queue_paid_notice(
                        &dir.join("journal"),
                        setup,
                        *secret,
                        event,
                        attachments,
                        now(),
                    )?;
                    Ok(
                        json!({"name":name,"outcome":"queued","next_action":"The existing runner validates the notice and stops unaccepted work when required."}),
                    )
                }
                Err(error) => Err(error),
            }
        } else {
            if args.positional().len() != 1 {
                return Err("paid command requires exactly one NAME".into());
            }
            let mut store = open_store(&dir, setup, secret)?;
            match command {
                "verify" => with_authority(args, |authority| {
                    let checked =
                        crate::runtime().block_on(store.verify_paid_delivery(authority, now()))?;
                    let passed = checked.observation.as_ref().is_some_and(|task| {
                        task.execution == coder::task::Execution::Finished
                            && task
                                .run
                                .as_ref()
                                .and_then(|r| r.result.as_ref())
                                .is_some_and(|r| r.exit_code == Some(0) && !r.output_incomplete)
                    });
                    Ok(
                        json!({"name":name,"checker_passed":passed,"checked":checked,"paid":store.paid_report()?}),
                    )
                }),
                "invoice" => {
                    let wallet = resident(args)?;
                    with_authority(args, |authority| {
                        coder_labor::paid::check_private_host_path(
                            &store.book,
                            Path::new(args.option("wallet-home").unwrap()),
                        )?;
                        Ok(
                            json!({"name":name,"invoice":store.prepare_worker_invoice(authority,&wallet,now())?,"paid":store.paid_report()?}),
                        )
                    })
                }
                "fund" => {
                    let wallet = resident(args)?;
                    let path = ledger_path(args)?;
                    // Authenticate current canonical authority before opening a
                    // writable ledger, even when funding is still unknown.
                    with_authority(args, |authority| {
                        coder_labor::paid::check_private_host_path(&store.book, &path)?;
                        coder_labor::paid::check_private_host_path(
                            &store.book,
                            Path::new(args.option("wallet-home").unwrap()),
                        )?;
                        let setup = store.book.paid().unwrap().clone();
                        authority.check(&setup, &store.book, now(), false)?;
                        let mut ledger =
                            pay_ledger::Ledger::open(path).map_err(|e| e.to_string())?;
                        let funding = store.reconcile_worker_funding(
                            authority,
                            &wallet,
                            &mut ledger,
                            now(),
                        )?;
                        Ok(json!({"name":name,"funding":funding,"paid":store.paid_report()?}))
                    })
                }
                _ => Err("unsupported paid fulfillment command".into()),
            }
        }
    })();
    match result {
        Ok(value) => {
            let failed = value.get("checker_passed") == Some(&json!(false));
            output.emit(&value, |v| {
                serde_json::to_string_pretty(v).unwrap_or_default()
            });
            if failed { EXIT_FAILURE } else { 0 }
        }
        Err(error) => refuse(output, command, Reason::Admission, &error),
    }
}
