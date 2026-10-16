//! Collect a Confidential Space token for a fresh, in-memory test key.
//! This probe signs no transaction and exposes no wallet or key export API.

#[cfg(unix)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use bitcoin::key::XOnlyPublicKey;
    use bitcoin::secp256k1::Secp256k1;
    use layertree_tee_lab::guest::{hex, token};
    use layertree_tee_lab::{attest::report_data, rules::TreeParams};

    let secp = Secp256k1::new();
    let (_secret, public) = secp.generate_keypair(&mut bitcoin::secp256k1::rand::thread_rng());
    let (_operator_secret, operator) =
        secp.generate_keypair(&mut bitcoin::secp256k1::rand::thread_rng());
    let params = TreeParams {
        enclave_key: XOnlyPublicKey::from(public),
        operator_key: XOnlyPublicKey::from(operator).serialize(),
        tree_id: [0x17; 32],
        depth: 8,
        dust_min: 330,
        unroll_delay: 144,
        migration_m: None,
    };
    let report = report_data(&params);
    let binding = hex(&report);
    // The remaining 32 report bytes are zero; the nonce carries the hash.
    let nonce = hex(&report[..32]);
    let token = token(&report)?;
    println!(
        "{}",
        serde_json::json!({
            "schema":"openagents.layertree.attestation-probe.v1",
            "source_commit":option_env!("LAYERTREE_SOURCE_COMMIT").unwrap_or("unrecorded"),
            "enclave_key":params.enclave_key.to_string(),
            "operator_key":hex(&params.operator_key),
            "tree_id":hex(&params.tree_id),
            "depth":params.depth,
            "dust_min":params.dust_min,
            "unroll_delay":params.unroll_delay,
            "migration_m":null,
            "report_data":binding,
        "nonce":nonce,
            "audience":"openagents.layertree.route.v1",
            "token":token,
            "transaction_signing":false,
        })
    );
    Ok(())
}

#[cfg(not(unix))]
fn main() {
    eprintln!("The attestation probe requires a Linux Confidential Space guest.");
    std::process::exit(1);
}
