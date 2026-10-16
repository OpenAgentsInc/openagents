//! Historical replay of the real signet output and its live TLS-bound evidence.
//! Core supplied the confirmation; this offline test does not resync signet.
use bitcoin::{
    Amount, TxOut,
    consensus::encode::deserialize_hex,
    hashes::{Hash, sha256},
};
use layertree_attest::{
    Binding, Error, Evidence, Policy, TcbFloor, TokenPolicy, verify, verify_token,
};
use layertree_tee_lab::{
    events::Body,
    rpc::{self, Parameters, Snapshot},
};
use serde_json::Value;

const IMAGE: &str = "sha256:9377f95b0562ac6b147c00b4465370c428ab681a4a776a6994c9110c53740a11";
const SOURCE: &str = "59b89bc1cda6434e33c17a99674ac70976473c9c";
const PUBLISHER: &str = "fc4d6cd08891e19627bca90ae667097d261b8290e54ada0bb7e959f06a390f8d";
macro_rules! fixture {
    ($name:literal) => {
        serde_json::from_str(include_str!(concat!("fixtures/signet-genesis/", $name))).unwrap()
    };
}

#[test]
fn confirmed_signet_output_matches_the_release_and_live_channel() {
    let attestation: Value = fixture!("attestation.json");
    let channel: Value = fixture!("channel.json");
    let snapshot: Snapshot = fixture!("state-before.json");
    let observed: Value = fixture!("verified-output.json");
    let transaction: Value = fixture!("chain-transaction.json");
    let header: Value = fixture!("chain-header.json");
    let release = fixture!("release.json");
    let head = fixture!("head.json");
    let entry = fixture!("rekor-entry.json");
    let rebuilds = [fixture!("local-rebuild.json"), fixture!("gcp-rebuild.json")];
    let params = snapshot.params.params().unwrap();
    let root = rpc::tree(params.depth, &snapshot.leaves)
        .unwrap()
        .root()
        .unwrap();
    let output: TxOut = deserialize_hex(observed["output"].as_str().unwrap()).unwrap();
    let tx = rpc::transaction(transaction["hex"].as_str().unwrap()).unwrap();
    let point = rpc::outpoint(observed["outpoint"].as_str().unwrap()).unwrap();
    assert_eq!(tx.compute_txid(), point.txid);
    assert_eq!(tx.output[point.vout as usize], output);
    assert_eq!(transaction["blockhash"], header["hash"]);
    assert!(transaction["confirmations"].as_u64().unwrap() >= 1);
    assert_eq!(header["height"], 326595);
    assert_eq!(snapshot.outpoint.as_deref(), observed["outpoint"].as_str());
    assert_eq!(root.sum, 5000);
    assert_eq!(snapshot.ephemeral_keys, 0);
    let certificate = rpc::bytes(channel["certificate_der"].as_str().unwrap()).unwrap();
    let certificate_sha256 = sha256::Hash::hash(&certificate).to_byte_array();
    let challenge = rpc::array(channel["challenge"].as_str().unwrap()).unwrap();
    assert_eq!(
        rpc::hex(&certificate_sha256),
        attestation["certificate_sha256"]
    );
    let binding = Binding::Live {
        challenge: &challenge,
        certificate_sha256: &certificate_sha256,
    };
    let now = channel["checked_at_unix"].as_u64().unwrap();
    let floor = floor();
    let policy = Policy {
        params: &params,
        publisher: PUBLISHER,
        workload: "layertree-signet",
        min_rebuilds: 2,
        seen_generation: None,
        max_age_seconds: 900,
        floor: &floor,
    };
    let evidence = Evidence {
        token: attestation["token"].as_str().unwrap(),
        release: &release,
        head: &head,
        log_entry: &entry,
        rebuilds: &rebuilds,
        consistency: None,
        output: &output,
        root,
        unroll_key: snapshot.unroll_key.as_deref().unwrap().parse().unwrap(),
    };
    let mut witness = att_release::rekor::Witness::default();
    let checked = verify(&evidence, &policy, &binding, &mut witness, now).unwrap();
    assert_eq!(checked.image_digest, IMAGE);
    assert_eq!(checked.source_commit, SOURCE);
    assert_eq!(
        serde_json::to_value(checked.event()).unwrap(),
        observed["event"]
    );
    assert!(matches!(checked.event(), Body::Attestation {result, ..} if result == "attested"));

    for mutate_value in [false, true] {
        let mut wrong_output = output.clone();
        if mutate_value {
            wrong_output.value = Amount::from_sat(5001);
        } else {
            wrong_output.script_pubkey = bitcoin::ScriptBuf::new();
        }
        let wrong_evidence = Evidence {
            output: &wrong_output,
            ..evidence
        };
        let mut empty = att_release::rekor::Witness::default();
        let before = serde_json::to_value(&empty).unwrap();
        assert_eq!(
            verify(&wrong_evidence, &policy, &binding, &mut empty, now).unwrap_err(),
            Error::OutputMismatch
        );
        assert_eq!(serde_json::to_value(&empty).unwrap(), before);
    }
}

fn floor() -> TcbFloor {
    TcbFloor {
        evaluation_data_number: 19,
        attester_date_unix: 1_747_180_800,
        confidential_space_build: 260900,
    }
}

#[test]
fn live_capture_refuses_substituted_keys_parameters_channel_and_tcb() {
    let capture: Value = fixture!("attestation.json");
    let channel: Value = fixture!("channel.json");
    let parameters: Parameters = serde_json::from_value(capture["params"].clone()).unwrap();
    let params = parameters.params().unwrap();
    let token = capture["token"].as_str().unwrap();
    let now = channel["checked_at_unix"].as_u64().unwrap();
    let challenge = rpc::array(channel["challenge"].as_str().unwrap()).unwrap();
    let certificate = rpc::bytes(channel["certificate_der"].as_str().unwrap()).unwrap();
    let certificate_sha256 = sha256::Hash::hash(&certificate).to_byte_array();
    let binding = Binding::Live {
        challenge: &challenge,
        certificate_sha256: &certificate_sha256,
    };
    let floor = floor();
    let policy = TokenPolicy {
        image_digest: IMAGE,
        max_age_seconds: 900,
        floor: &floor,
    };
    let claims = verify_token(token, &params, &policy, &binding, now).unwrap();
    let wrong_image = TokenPolicy {
        image_digest: "sha256:0000000000000000000000000000000000000000000000000000000000000000",
        ..policy
    };
    assert_eq!(
        verify_token(token, &params, &wrong_image, &binding, now).unwrap_err(),
        Error::UnknownMeasurement
    );
    for mutation in 0..3 {
        let mut other = params.clone();
        match mutation {
            0 => {
                other.enclave_key =
                    bitcoin::key::XOnlyPublicKey::from_slice(&params.operator_key).unwrap()
            }
            1 => other.tree_id[0] ^= 1,
            _ => other.unroll_delay += 1,
        }
        assert_eq!(
            verify_token(token, &other, &policy, &binding, now).unwrap_err(),
            Error::ReportDataMismatch
        );
    }
    for change_challenge in [false, true] {
        let mut wrong_challenge = challenge;
        let mut wrong_certificate = certificate_sha256;
        if change_challenge {
            wrong_challenge[0] ^= 1;
        } else {
            wrong_certificate[0] ^= 1;
        }
        assert_eq!(
            verify_token(
                token,
                &params,
                &policy,
                &Binding::Live {
                    challenge: &wrong_challenge,
                    certificate_sha256: &wrong_certificate
                },
                now
            )
            .unwrap_err(),
            Error::ReportDataMismatch
        );
    }
    assert_eq!(
        verify_token(token, &params, &policy, &binding, claims.iat + 901).unwrap_err(),
        Error::Stale
    );
    let higher = TcbFloor {
        evaluation_data_number: claims.raw["tdx"]["tcb_evaluation_data_number"]
            .as_u64()
            .unwrap() as u32
            + 1,
        ..floor
    };
    let higher_policy = TokenPolicy {
        floor: &higher,
        ..policy
    };
    assert_eq!(
        verify_token(token, &params, &higher_policy, &binding, now).unwrap_err(),
        Error::TcbFloor
    );
    let mut pieces: Vec<_> = token.split('.').map(str::to_owned).collect();
    let replace = if pieces[2].starts_with('a') { "b" } else { "a" };
    pieces[2].replace_range(..1, replace);
    assert!(matches!(
        verify_token(&pieces.join("."), &params, &policy, &binding, now),
        Err(Error::Token(_))
    ));
}
