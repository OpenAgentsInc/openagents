#![no_main]
use bitcoin::{
    key::XOnlyPublicKey,
    secp256k1::{Secp256k1, SecretKey},
};
use layertree_attest::{Binding, TcbFloor, TokenPolicy, verify_token};
use layertree_tee_lab::rules::TreeParams;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(token) = std::str::from_utf8(data) else {
        return;
    };
    let secp = Secp256k1::new();
    let operator = SecretKey::from_slice(&[9; 32]).unwrap();
    let params = TreeParams {
        enclave_key: "a90d75983b148a0d5f684e6bcc018bd55a95237854ca0c8ab77ec4df57438c0d"
            .parse::<XOnlyPublicKey>()
            .unwrap(),
        operator_key: XOnlyPublicKey::from(operator.public_key(&secp)).serialize(),
        tree_id: [0x17; 32],
        depth: 8,
        dust_min: 330,
        unroll_delay: 144,
        migration_m: None,
    };
    let floor = TcbFloor {
        evaluation_data_number: 19,
        attester_date_unix: 1_747_180_800,
        confidential_space_build: 260900,
    };
    let policy = TokenPolicy {
        image_digest: "sha256:79bf22d374f8c33a58f4c126ce36da9f729501d881ac80b48707d007653bf100",
        max_age_seconds: 900,
        floor: &floor,
    };
    let _ = verify_token(token, &params, &policy, &Binding::Recorded, 1_792_139_163);
    let _ = verify_token(
        token,
        &params,
        &policy,
        &Binding::Live {
            challenge: &[8; 32],
            certificate_sha256: &[9; 32],
        },
        1_792_139_163,
    );
});
