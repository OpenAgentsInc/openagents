//! Real token and release composition; output bytes here are a unit fixture.
//! The live signet qualification supplies its separately observed output.
use bitcoin::{Amount, TxOut, key::XOnlyPublicKey, secp256k1::Secp256k1};
use layertree_attest::{
    Binding, Error, Evidence, Policy, TcbFloor, TokenPolicy, verify, verify_token,
};
use layertree_tee_lab::{events::Body, output::TreeOutput, rules::TreeParams, tree::Node};
use serde_json::Value;

const IMAGE: &str = "sha256:79bf22d374f8c33a58f4c126ce36da9f729501d881ac80b48707d007653bf100";
const PUBLISHER: &str = "fc4d6cd08891e19627bca90ae667097d261b8290e54ada0bb7e959f06a390f8d";
const NOW: u64 = 1_792_139_163;

fn bytes(text: &str) -> [u8; 32] {
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
        .collect::<Vec<_>>()
        .try_into()
        .unwrap()
}
fn fixture() -> Value {
    serde_json::from_str(include_str!(
        "../../../oa-att/tests/fixtures/layertree-key-restored-tdx.json"
    ))
    .unwrap()
}
fn params(probe: &Value) -> TreeParams {
    TreeParams {
        enclave_key: probe["enclave_key"].as_str().unwrap().parse().unwrap(),
        operator_key: bytes(probe["operator_key"].as_str().unwrap()),
        tree_id: bytes(probe["tree_id"].as_str().unwrap()),
        depth: 8,
        dust_min: 330,
        unroll_delay: 144,
        migration_m: None,
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
fn the_real_token_release_and_expected_output_are_composed() {
    let probe = fixture();
    let params = params(&probe);
    let floor = floor();
    let release = serde_json::from_str(include_str!("fixtures/release.json")).unwrap();
    let head = serde_json::from_str(include_str!("fixtures/head.json")).unwrap();
    let entry = serde_json::from_str(include_str!("fixtures/rekor-entry.json")).unwrap();
    let rebuilds = [
        serde_json::from_str(include_str!("fixtures/local-rebuild.json")).unwrap(),
        serde_json::from_str(include_str!("fixtures/gcp-rebuild.json")).unwrap(),
    ];
    let root = Node {
        hash: [5; 32],
        sum: 10_000,
    };
    let unroll_key = XOnlyPublicKey::from_slice(&params.operator_key).unwrap();
    let output = TxOut {
        value: Amount::from_sat(root.sum),
        script_pubkey: TreeOutput {
            root_hash: root.hash,
            unroll_key,
        }
        .script_pubkey(
            &Secp256k1::verification_only(),
            &params.enclave_key,
            params.unroll_delay,
        ),
    };
    let mut evidence = Evidence {
        token: probe["token"].as_str().unwrap(),
        release: &release,
        head: &head,
        log_entry: &entry,
        rebuilds: &rebuilds,
        consistency: None,
        output: &output,
        root,
        unroll_key,
    };
    let policy = Policy {
        params: &params,
        publisher: PUBLISHER,
        workload: "layertree-signet-key-probe",
        min_rebuilds: 2,
        seen_generation: None,
        max_age_seconds: 900,
        floor: &floor,
    };
    let mut witness = att_release::rekor::Witness::default();
    let verified = verify(&evidence, &policy, &Binding::Recorded, &mut witness, NOW).unwrap();
    assert_eq!(verified.image_digest, IMAGE);
    assert_eq!(verified.tcb_evaluation_data_number, 19);
    assert!(
        matches!(verified.event(),Body::Attestation { result,measurement,.. } if result=="attested" && measurement==IMAGE)
    );
    let mut wrong_output = output.clone();
    wrong_output.value = Amount::from_sat(root.sum + 1);
    evidence.output = &wrong_output;
    let mut empty_witness = att_release::rekor::Witness::default();
    let before = serde_json::to_value(&empty_witness).unwrap();
    assert_eq!(
        verify(
            &evidence,
            &policy,
            &Binding::Recorded,
            &mut empty_witness,
            NOW
        )
        .unwrap_err(),
        Error::OutputMismatch
    );
    assert_eq!(serde_json::to_value(&empty_witness).unwrap(), before);
}

#[test]
fn real_evidence_refuses_other_measurements_keys_parameters_and_stale_views() {
    let probe = fixture();
    let params = params(&probe);
    let floor = floor();
    let token = probe["token"].as_str().unwrap();
    let policy = TokenPolicy {
        image_digest: IMAGE,
        max_age_seconds: 900,
        floor: &floor,
    };
    let claims = verify_token(token, &params, &policy, &Binding::Recorded, NOW).unwrap();
    assert_eq!(
        verify_token(token, &params, &policy, &Binding::Recorded, u64::MAX).unwrap_err(),
        Error::Policy
    );

    let wrong = TokenPolicy {
        image_digest: "sha256:0000000000000000000000000000000000000000000000000000000000000000",
        ..policy
    };
    assert_eq!(
        verify_token(token, &params, &wrong, &Binding::Recorded, NOW).unwrap_err(),
        Error::UnknownMeasurement
    );
    let mut other = params.clone();
    other.enclave_key = XOnlyPublicKey::from_slice(&params.operator_key).unwrap();
    assert_eq!(
        verify_token(token, &other, &policy, &Binding::Recorded, NOW).unwrap_err(),
        Error::ReportDataMismatch
    );
    let mut other = params.clone();
    other.tree_id[0] ^= 1;
    assert_eq!(
        verify_token(token, &other, &policy, &Binding::Recorded, NOW).unwrap_err(),
        Error::ReportDataMismatch
    );
    let mut other = params.clone();
    other.unroll_delay += 1;
    assert_eq!(
        verify_token(token, &other, &policy, &Binding::Recorded, NOW).unwrap_err(),
        Error::ReportDataMismatch
    );
    assert_eq!(
        verify_token(
            token,
            &params,
            &policy,
            &Binding::Recorded,
            claims.iat + 901
        )
        .unwrap_err(),
        Error::Stale
    );
    let higher = TcbFloor {
        evaluation_data_number: 20,
        ..floor
    };
    let higher_policy = TokenPolicy {
        floor: &higher,
        ..policy
    };
    assert_eq!(
        verify_token(token, &params, &higher_policy, &Binding::Recorded, NOW).unwrap_err(),
        Error::TcbFloor
    );
    assert_eq!(
        verify_token(
            token,
            &params,
            &policy,
            &Binding::Live {
                challenge: &[8; 32],
                certificate_sha256: &[9; 32]
            },
            NOW
        )
        .unwrap_err(),
        Error::ReportDataMismatch
    );
    assert_eq!(
        verify_token(
            &"a".repeat(32_769),
            &params,
            &policy,
            &Binding::Recorded,
            NOW
        )
        .unwrap_err(),
        Error::TooLarge
    );
    let mut pieces: Vec<_> = token.split('.').map(str::to_owned).collect();
    let replacement = if pieces[1].starts_with('a') { "b" } else { "a" };
    pieces[1].replace_range(0..1, replacement);
    assert!(matches!(
        verify_token(&pieces.join("."), &params, &policy, &Binding::Recorded, NOW),
        Err(Error::Token(_))
    ));
}
