//! `tenant-db`: the account database's operator tool (#11154).
//!
//! ```text
//! tenant-db migrate [--dsn-env VAR]
//! tenant-db import  --registry DIR [--force] [--dsn-env VAR]
//! tenant-db verify  --registry DIR [--dsn-env VAR]
//! tenant-db tenant-provider-keys [--dsn-env VAR]
//! tenant-db assign-provider-key --tenant T --provider P --workspace WS [--dsn-env VAR]
//! ```
//!
//! The connection string comes from the environment variable named by
//! `--dsn-env` (default `OPENAGENTS_ACCOUNTS_DATABASE_URL`), never from
//! the command line, so it stays out of shell history and process lists.
//! `import` moves a registry directory's files (an NFS export) into the
//! database and is safe to run again; `verify` reads every account,
//! session and key back and compares it with the files. Both print their
//! counts as one JSON line; `verify` exits 1 on any difference.
//!
//! Provider keys kept by registry tenant before #11186 wait in
//! `identity.provider_keys_by_tenant` until the gateway moves each to its
//! workspace. `tenant-provider-keys` lists them (tenant, provider, the
//! workspace named, the fingerprint; never a key), and
//! `assign-provider-key` names the workspace that saved one on a tenant
//! many workspaces share, so the gateway moves it there at its next start.

use std::path::PathBuf;
use std::process::ExitCode;

use tenancy::db::{Database, import};

const DEFAULT_ENV: &str = "OPENAGENTS_ACCOUNTS_DATABASE_URL";

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(message) => {
            eprintln!("tenant-db: {message}");
            ExitCode::from(2)
        }
    }
}

fn run() -> Result<ExitCode, String> {
    let mut args = std::env::args().skip(1);
    let command = args.next().ok_or_else(usage)?;
    let mut registry: Option<PathBuf> = None;
    let mut force = false;
    let (mut tenant, mut provider, mut workspace) = (None, None, None);
    let mut dsn_env = DEFAULT_ENV.to_string();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--registry" => registry = Some(args.next().ok_or_else(usage)?.into()),
            "--force" => force = true,
            "--tenant" => tenant = Some(args.next().ok_or_else(usage)?),
            "--provider" => provider = Some(args.next().ok_or_else(usage)?),
            "--workspace" => workspace = Some(args.next().ok_or_else(usage)?),
            "--dsn-env" => dsn_env = args.next().ok_or_else(usage)?,
            _ => return Err(usage()),
        }
    }
    let dsn =
        std::env::var(&dsn_env).map_err(|_| format!("set {dsn_env} to the connection string"))?;
    let database = Database::connect(&dsn).map_err(|e| e.to_string())?;
    let started = std::time::Instant::now();
    match command.as_str() {
        "migrate" => {
            println!("{{\"migrated\":true}}");
            Ok(ExitCode::SUCCESS)
        }
        "import" => {
            let dir = registry.ok_or_else(usage)?;
            let report = import::import(&database, &dir, force).map_err(|e| e.to_string())?;
            print(&report, started);
            Ok(ExitCode::SUCCESS)
        }
        "verify" => {
            let dir = registry.ok_or_else(usage)?;
            let report = import::verify(&database, &dir).map_err(|e| e.to_string())?;
            print(&report, started);
            Ok(if report.mismatches.is_empty() {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            })
        }
        "tenant-provider-keys" => {
            let rows =
                tenancy::db::records::tenant_provider_keys(&database).map_err(|e| e.to_string())?;
            for row in rows {
                println!(
                    "{}",
                    serde_json::json!({
                        "tenant": row.tenant,
                        "provider": row.provider,
                        "workspace": row.workspace,
                        "fingerprint": row.record.get("fingerprint"),
                        "added_at": row.record.get("added_at"),
                    })
                );
            }
            Ok(ExitCode::SUCCESS)
        }
        "assign-provider-key" => {
            let (Some(tenant), Some(provider), Some(workspace)) = (tenant, provider, workspace)
            else {
                return Err(usage());
            };
            let named = tenancy::db::records::assign_tenant_provider_key(
                &database, &tenant, &provider, &workspace,
            )
            .map_err(|e| e.to_string())?;
            println!("{}", serde_json::json!({"assigned": named}));
            Ok(if named {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            })
        }
        _ => Err(usage()),
    }
}

fn print(report: &import::Report, started: std::time::Instant) {
    let mut value = serde_json::to_value(report).unwrap_or_default();
    if let Some(object) = value.as_object_mut() {
        object.insert(
            "millis".into(),
            serde_json::json!(started.elapsed().as_millis() as u64),
        );
    }
    println!("{value}");
}

fn usage() -> String {
    "usage: tenant-db migrate | import --registry DIR [--force] | verify --registry DIR \
     | tenant-provider-keys | assign-provider-key --tenant T --provider P --workspace WS \
     [--dsn-env VAR]"
        .into()
}
