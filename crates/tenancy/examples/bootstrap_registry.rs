//! Bootstrap a local registry for a development gateway: one tenant,
//! one shared door binding, one issued key. The artifact signature
//! must be the digest the backend actually publishes — the gateway
//! refuses the call otherwise, which is the check working, not a
//! bootstrap bug.
//!
//! ```text
//! cargo run -p tenancy --example bootstrap_registry -- \
//!     --registry /tmp/gw-registry --tenant eval \
//!     --door local-kev --model kev-latest \
//!     --signature sha256:<published digest>
//! ```

use std::collections::BTreeMap;
use std::path::PathBuf;

use tenancy::{Binding, Expected, Lane, Manifest, Registry, SCHEMA, Tenant, keys};

fn arg(name: &str) -> String {
    let mut args = std::env::args().skip_while(|a| a != name);
    args.next();
    args.next()
        .unwrap_or_else(|| panic!("{name} requires a value"))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let registry_dir = PathBuf::from(arg("--registry"));
    let tenant_name = arg("--tenant");
    let door = arg("--door");
    let model = arg("--model");
    let signature = arg("--signature");

    let mut shared = BTreeMap::new();
    shared.insert(
        door,
        Binding {
            lane: Lane::Shared,
            artifact: Expected {
                model,
                adapter: None,
                artifact_signature: signature,
                execution: BTreeMap::new(),
            },
            capacity: None,
            promotion: None,
            scope: vec![],
        },
    );
    let mut tenants = BTreeMap::new();
    tenants.insert(
        tenant_name.clone(),
        Tenant {
            credential: format!("key-ref:{tenant_name}"),
            principals: vec![],
            doors: BTreeMap::new(),
            quota: None,
        },
    );
    let mut manifest = Manifest {
        v: SCHEMA.to_string(),
        sequence: 0,
        supersedes: None,
        shared,
        tenants,
        digest: String::new(),
    };
    manifest.seal();

    let registry = Registry::install(&registry_dir, manifest)?;
    let issued = keys::issue(&registry_dir, registry.manifest(), &tenant_name)?;
    println!("{}", issued.token);
    Ok(())
}
