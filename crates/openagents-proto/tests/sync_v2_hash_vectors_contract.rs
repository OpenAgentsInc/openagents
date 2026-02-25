use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

#[derive(Debug, Deserialize)]
struct VectorFixture {
    version: String,
    algorithm: String,
    vectors: Vec<HashVector>,
}

#[derive(Debug, Deserialize)]
struct HashVector {
    name: String,
    payload: Value,
    canonical_json: String,
    sha256: String,
}

fn fixture() -> VectorFixture {
    serde_json::from_str(include_str!(
        "../../../docs/protocol/testdata/spacetime_payload_hash_vectors.v1.json"
    ))
    .expect("sync v2 hash vectors fixture must parse")
}

fn canonical_json(value: &Value) -> String {
    match value {
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {
            serde_json::to_string(value).expect("scalar JSON should serialize")
        }
        Value::Array(items) => {
            let body = items.iter().map(canonical_json).collect::<Vec<_>>().join(",");
            format!("[{body}]")
        }
        Value::Object(map) => {
            let mut keys = map.keys().cloned().collect::<Vec<_>>();
            keys.sort();

            let body = keys
                .iter()
                .map(|key| {
                    let key_json = serde_json::to_string(key).expect("object key should serialize");
                    let value_json = canonical_json(map.get(key).expect("key must exist"));
                    format!("{key_json}:{value_json}")
                })
                .collect::<Vec<_>>()
                .join(",");
            format!("{{{body}}}")
        }
    }
}

fn sha256_prefixed(value: &str) -> String {
    let digest = Sha256::digest(value.as_bytes());
    format!("sha256:{digest:x}")
}

#[test]
fn sync_v2_hash_vectors_match_canonical_json_and_sha256() {
    let fixture = fixture();
    assert_eq!(fixture.version, "spacetime.payload_hash.v1");
    assert_eq!(fixture.algorithm, "sha256");
    assert!(!fixture.vectors.is_empty(), "vectors must not be empty");

    for vector in fixture.vectors {
        let derived_canonical = canonical_json(&vector.payload);
        assert_eq!(
            derived_canonical, vector.canonical_json,
            "canonical json mismatch for vector {}",
            vector.name
        );

        let derived_sha = sha256_prefixed(derived_canonical.as_str());
        assert_eq!(
            derived_sha, vector.sha256,
            "sha256 mismatch for vector {}",
            vector.name
        );
    }
}

