use env_host::allocator::{Bundle, envelope};
use nostr::contracts::{digest_bytes, jcs};
use secp256k1::{Secp256k1, SecretKey};
use serde_json::{Value, json};
use std::{collections::BTreeMap, path::Path, process::Command};

fn run(args: &[&str]) -> (bool, Value) {
    let output = Command::new(env!("CARGO_BIN_EXE_env-host"))
        .args(args)
        .output()
        .unwrap();
    let value = if output.stdout.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&output.stdout).unwrap()
    };
    (output.status.success(), value)
}
fn pk(secret: &SecretKey) -> String {
    secret.x_only_public_key(&Secp256k1::new()).0.to_string()
}
fn put(map: &mut BTreeMap<String, Vec<u8>>, bytes: &[u8], schema: Option<&str>) -> Value {
    map.insert(digest_bytes(bytes), bytes.to_vec());
    let mut value =
        json!({"digest":digest_bytes(bytes),"size":bytes.len(),"media_type":"application/json"});
    if let Some(schema) = schema {
        value["schema"] = json!(schema);
    }
    value
}
fn put_json(map: &mut BTreeMap<String, Vec<u8>>, body: &Value) -> Value {
    put(map, &jcs(body).unwrap(), body["v"].as_str())
}
fn write_key(path: &Path, secret: &SecretKey) {
    std::fs::write(path, secret.display_secret().to_string()).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
}

#[test]
fn cli_allocates_materializes_and_destroys_a_local_environment() {
    let temp = tempfile::tempdir().unwrap();
    let allocator = SecretKey::from_byte_array([21; 32]).unwrap();
    let owner = SecretKey::from_byte_array([22; 32]).unwrap();
    let worker = SecretKey::from_byte_array([23; 32]).unwrap();
    let keyfile = temp.path().join("key");
    write_key(&keyfile, &allocator);
    let state = temp.path().join("state");
    let resources = temp.path().join("resources");
    let policy = temp.path().join("policy.json");
    std::fs::write(
        &policy,
        serde_json::to_vec(&json!({"owners":[pk(&owner)],"controllers":[],"readers":[],
            "account":"local","max_ttl_seconds":3600,"max_lifetime_seconds":86400}))
        .unwrap(),
    )
    .unwrap();
    let (state_s, key_s, res_s) = (
        state.to_str().unwrap(),
        keyfile.to_str().unwrap(),
        resources.to_str().unwrap(),
    );
    assert!(
        run(&[
            "init",
            "--state",
            state_s,
            "--key-file",
            key_s,
            "--policy",
            policy.to_str().unwrap()
        ])
        .0
    );

    let mut artifacts = BTreeMap::new();
    let requirements = put_json(
        &mut artifacts,
        &json!({"v":"openagents.environment-requirements.v1","requires":[],
            "recipients":[pk(&worker)],"assurance":"host","opaque_models":true,"bounds":["wall_ms"]}),
    );
    let runtime = json!({"id":format!("{}:runtime/shell",pk(&owner)),"artifact":put(&mut artifacts,b"runtime",None)});
    let configuration = put(&mut artifacts, b"config", None);
    let definition = json!({"v":"openagents.environment-definition.v1","requires":[],
        "components":[{"id":"base","role":"base","artifact":put(&mut artifacts,b"base",None)}],
        "workspace":null,"bootstrap":null,"runtime":runtime,"configuration":configuration,
        "model":null,"requirements":requirements});
    let reservation = json!({"v":"openagents.reservation.v1","requires":[],"id":"bb".repeat(32),
        "parent":null,"currency":"SAT","ceiling_microunits":5000,"spent_microunits":0,"unknown":false});
    let now = env_host::now();
    let request = json!({"v":"openagents.environment-request.v1","requires":[],
        "request":"cd".repeat(32),"run":"ce".repeat(32),"owner":pk(&owner),"allocator":pk(&allocator),
        "definition":put_json(&mut artifacts,&definition),"frame":put(&mut artifacts,b"frame",None),
        "policy":put(&mut artifacts,b"policy",None),"reservation":put_json(&mut artifacts,&reservation),
        "mode":"create","resource":null,
        "effects":{"reads":[],"writes":[],"network":[],"process":true,"delegates":false,"spend":false},
        "bounds":[{"bound":"wall_ms","ceiling":60000,"assurance":"host","mechanism":"env-host"},
            {"bound":"spend_microunits","ceiling":1000,"assurance":"host","mechanism":"env-host","currency":"SAT"}],
        "ttl_seconds":600,"idle_seconds":600,"not_after":now+3600,"retain_until":now+7200,"cleanup":"destroy"});
    let assurance = put(&mut artifacts, b"assurance", None);
    let bundle = Bundle {
        event: envelope(&owner, &pk(&allocator), &request, now).unwrap(),
        artifacts,
        declarations: Vec::new(),
    };
    let input = temp.path().join("bundle.json");
    std::fs::write(&input, serde_json::to_vec(&bundle).unwrap()).unwrap();
    let common = [
        "--state",
        state_s,
        "--key-file",
        key_s,
        "--provider-root",
        res_s,
    ];
    let mut args = vec!["submit"];
    args.extend(common);
    args.extend(["--input", input.to_str().unwrap()]);
    let (ok, result) = run(&args);
    assert!(ok, "{result}");
    assert_eq!(result["status"], "accepted");
    // A transport retry of the same signed request returns the same result.
    assert_eq!(run(&args).1, result);

    let mut args = vec!["leases"];
    args.extend(common);
    let lease = run(&args).1["leases"][0].as_str().unwrap().to_string();
    let spec = temp.path().join("spec.json");
    std::fs::write(&spec, serde_json::to_vec(&json!({"participants":[{"principal":pk(&worker),
        "role":"executor","runtime":runtime,"model":null,"configuration":configuration,"assurance":assurance}],
        "enforcement":[{"bound":"wall_ms","ceiling":60000,"assurance":"host","mechanism":"env-host"}]})).unwrap()).unwrap();
    let mut args = vec!["materialize"];
    args.extend(common);
    args.extend(["--lease", &lease, "--input", spec.to_str().unwrap()]);
    let (ok, materialized) = run(&args);
    assert!(ok, "{materialized}");

    let mut args = vec!["status"];
    args.extend(common);
    args.extend(["--lease", &lease]);
    assert_eq!(run(&args).1["status"], "ready");
    // Without an attachment no dispatch is admitted.
    let mut check = vec!["check"];
    check.extend(common);
    check.extend(["--attachment", "none", "--fingerprint", "sha256:00"]);
    assert!(!run(&check).0);

    let mut args = vec!["cleanup"];
    args.extend(common);
    args.extend(["--lease", &lease]);
    let (ok, cleaned) = run(&args);
    assert!(ok, "{cleaned}");
    assert_eq!(cleaned["status"], "released");
    assert_eq!(std::fs::read_dir(&resources).unwrap().count(), 0);
}
