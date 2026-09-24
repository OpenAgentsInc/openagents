//! `tenant-keys` — the operator's key provisioning path.
//!
//! Keys live in `keys.json` beside a registry's `registry.json`. Issue,
//! rotate, and revoke write it; `list` reads it. The only time a secret
//! leaves this program is on the line `issue` and `rotate` print once —
//! the store keeps digests.
//!
//! ```text
//! tenant-keys issue   --registry DIR --tenant NAME
//! tenant-keys rotate  --registry DIR --key ID
//! tenant-keys revoke  --registry DIR --key ID
//! tenant-keys list    --registry DIR
//! ```

use std::path::Path;

use tenancy::{Registry, keys};

fn usage() -> ! {
    eprintln!(
        "Usage:\n  \
         tenant-keys issue  --registry DIR --tenant NAME\n  \
         tenant-keys rotate --registry DIR --key ID\n  \
         tenant-keys revoke --registry DIR --key ID\n  \
         tenant-keys list   --registry DIR"
    );
    std::process::exit(2);
}

fn main() {
    let mut args = std::env::args().skip(1);
    let Some(verb) = args.next() else {
        usage();
    };
    let mut registry_dir = None;
    let mut tenant = None;
    let mut key = None;
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--registry" => registry_dir = args.next(),
            "--tenant" => tenant = args.next(),
            "--key" => key = args.next(),
            _ => usage(),
        }
    }
    let Some(dir) = registry_dir else { usage() };
    let dir = Path::new(&dir);

    let result = match verb.as_str() {
        "issue" => {
            let Some(tenant) = tenant else { usage() };
            let manifest = match Registry::open(dir) {
                Ok(registry) => registry.manifest().clone(),
                Err(trouble) => {
                    eprintln!("{trouble}");
                    std::process::exit(1);
                }
            };
            keys::issue(dir, &manifest, &tenant).map(|issued| {
                // The one place a secret is printed — once, to the
                // operator's terminal, never to the store or a log.
                println!("{}", issued.token);
                println!(
                    "Issued API key `{}` for tenant `{}`. Copy the secret on the line above now; it isn't shown again.",
                    issued.key.id, tenant
                );
            })
        }
        "rotate" => {
            let Some(key) = key else { usage() };
            keys::rotate(dir, &key).map(|issued| {
                println!("{}", issued.token);
                println!(
                    "Replaced API key `{key}` with `{}`; `{key}` no longer works. Copy the secret on the line above now; it isn't shown again.",
                    issued.key.id
                );
            })
        }
        "revoke" => {
            let Some(key) = key else { usage() };
            keys::revoke(dir, &key)
                .map(|()| println!("Revoked API key `{key}`; it no longer works."))
        }
        "list" => keys::load(dir).map(|store| {
            for record in store.keys.values() {
                println!(
                    "{}\t{}\t{}\t{}",
                    record.id,
                    record.tenant,
                    match record.status {
                        keys::Status::Active => "active",
                        keys::Status::Paused => "paused",
                        keys::Status::Revoked => "revoked",
                    },
                    record.created
                );
            }
        }),
        _ => usage(),
    };
    if let Err(trouble) = result {
        eprintln!("{trouble}");
        std::process::exit(1);
    }
}
