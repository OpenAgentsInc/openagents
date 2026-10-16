use oa_vault::object::{self, New, Tier};
use oa_vault::operator::{LocalOperatorKms, wrap_operator};
use serde_json::{Value, json};
use std::process::Command;
use vault_sealed::Signed;

const VERSION: &str =
    "projects/fixture/locations/global/keyRings/vault/cryptoKeys/operator/cryptoKeyVersions/1";
const VAULT: &str = "1111111111111111111111111111111111111111111111111111111111111111";
const OBJ: &str = "2222222222222222222222222222222222222222222222222222222222222222";

fn run(args: &[&str]) -> (bool, Value) {
    let out = Command::new(env!("CARGO_BIN_EXE_vault-operator"))
        .args(args)
        .output()
        .unwrap();
    let value = if out.stdout.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&out.stdout).unwrap()
    };
    (out.status.success(), value)
}

#[test]
fn cli_grants_uses_and_verifies_the_audit() {
    let temp = tempfile::tempdir().unwrap();
    let operator = secp256k1::SecretKey::from_byte_array([1; 32]).unwrap();
    let alice = secp256k1::SecretKey::from_byte_array([2; 32]).unwrap();
    let pk = |s: &secp256k1::SecretKey| {
        s.x_only_public_key(&secp256k1::Secp256k1::new())
            .0
            .to_string()
    };
    let key = temp.path().join("kms.key");
    std::fs::write(&key, "6a".repeat(32)).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&key, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    let config = temp.path().join("config.json");
    std::fs::write(
        &config,
        serde_json::to_vec(&json!({"operators":[pk(&operator)],"recipients":[pk(&alice)]}))
            .unwrap(),
    )
    .unwrap();
    let dir = temp.path().join("svc");
    let (d, k) = (dir.to_str().unwrap(), key.to_str().unwrap());
    assert!(run(&["init", "--dir", d, "--config", config.to_str().unwrap()]).0);
    let now = vault_operator::now();
    let grant = Signed::sign(json!({"v":"openagents.vault-operator-grant.v1","requires":[],"grant":"aa".repeat(32),"vault":VAULT,
        "objects":[OBJ],"recipient":pk(&alice),"purpose":"support","expires_at":now+600,"max_uses":1,"issued_at":now-1}), &operator).unwrap();
    let input = temp.path().join("grant.json");
    std::fs::write(&input, serde_json::to_vec(&grant).unwrap()).unwrap();
    let (ok, granted) = run(&[
        "grant",
        "--dir",
        d,
        "--kms-key",
        k,
        "--kms-version",
        VERSION,
        "--input",
        input.to_str().unwrap(),
    ]);
    assert!(ok, "{granted}");
    let new = New {
        object: OBJ,
        vault: VAULT,
        tier: Tier::Operator,
        media: None,
        created_at: 1,
    };
    let draft = object::seal_new(&new, b"hello", Vec::new()).unwrap();
    let mut kms = LocalOperatorKms::new(VERSION, [0x6a; 32]);
    let wrap = wrap_operator(&mut kms, &draft.header.core_digest().unwrap(), &draft.dek).unwrap();
    let nonce = oa_vault::unb64_n::<7>(&draft.header.core.content.nonce_prefix).unwrap();
    let bytes = object::seal_with(
        &new,
        b"hello",
        vec![wrap],
        draft.dek,
        nonce,
        draft.header.core.content.chunk_bytes,
    )
    .unwrap()
    .bytes;
    let object_file = temp.path().join("object.bin");
    std::fs::write(&object_file, bytes).unwrap();
    let request = Signed::sign(
        json!({"v":"openagents.vault-operator-use.v1","requires":[],"request":"bb".repeat(32),
        "grant":"aa".repeat(32),"object":OBJ,"purpose":"support","issued_at":now}),
        &alice,
    )
    .unwrap();
    std::fs::write(&input, serde_json::to_vec(&request).unwrap()).unwrap();
    let (ok, used) = run(&[
        "use",
        "--dir",
        d,
        "--kms-key",
        k,
        "--kms-version",
        VERSION,
        "--input",
        input.to_str().unwrap(),
        "--object",
        object_file.to_str().unwrap(),
    ]);
    assert!(ok, "{used}");
    assert_eq!(
        oa_vault::unb64(used["plaintext"].as_str().unwrap()).unwrap(),
        b"hello"
    );
    assert_eq!(
        run(&[
            "audit",
            "--dir",
            d,
            "--kms-key",
            k,
            "--kms-version",
            VERSION
        ])
        .1["entries"],
        3
    );
}
