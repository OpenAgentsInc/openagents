use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::parity::scorecard::ParityScorecard;
use crate::{CadError, CadResult};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReferenceTableSnapshot {
    pub rows: Vec<Value>,
    pub contracts: Vec<String>,
}

impl ReferenceTableSnapshot {
    pub fn new(mut rows: Vec<Value>, mut contracts: Vec<String>) -> Self {
        sort_rows(&mut rows);
        contracts.sort();
        Self { rows, contracts }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct ReferenceTableFixture {
    manifest_version: u64,
    issue_id: String,
    vcad_commit: String,
    source: String,
    expected_rows: Vec<Value>,
    expected_contracts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReferenceTableParityManifest {
    pub manifest_version: u64,
    pub issue_id: String,
    pub vcad_commit: String,
    pub openagents_commit: String,
    pub generated_from_scorecard: String,
    pub reference_fixture_path: String,
    pub reference_fixture_sha256: String,
    pub reference_source: String,
    pub reference_issue_match: bool,
    pub reference_commit_match: bool,
    pub row_set_match: bool,
    pub contract_set_match: bool,
    pub deterministic_replay_match: bool,
    pub rows: Vec<Value>,
    pub contracts: Vec<String>,
    pub deterministic_signature: String,
    pub parity_contracts: Vec<String>,
}

pub fn build_reference_table_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
    issue_id: &str,
    reference_fixture_path: &str,
    reference_fixture_json: &str,
    parity_contracts: Vec<String>,
    snapshot: ReferenceTableSnapshot,
    replay_snapshot: ReferenceTableSnapshot,
) -> CadResult<ReferenceTableParityManifest> {
    let reference: ReferenceTableFixture =
        serde_json::from_str(reference_fixture_json).map_err(|error| CadError::ParseFailed {
            reason: format!(
                "failed parsing reference-table parity fixture for {issue_id}: {error}"
            ),
        })?;

    let reference_fixture_sha256 = sha256_hex(reference_fixture_json.as_bytes());
    let reference_issue_match = reference.issue_id == issue_id;
    let reference_commit_match = reference.vcad_commit == scorecard.vcad_commit;

    let mut expected_rows = reference.expected_rows;
    sort_rows(&mut expected_rows);
    let mut expected_contracts = reference.expected_contracts;
    expected_contracts.sort();

    let row_set_match = snapshot.rows == expected_rows;
    let contract_set_match = snapshot.contracts == expected_contracts;
    let deterministic_replay_match = snapshot == replay_snapshot;

    let deterministic_signature = parity_signature(
        issue_id,
        &snapshot,
        reference_issue_match,
        reference_commit_match,
        row_set_match,
        contract_set_match,
        deterministic_replay_match,
        &reference_fixture_sha256,
    );

    Ok(ReferenceTableParityManifest {
        manifest_version: 1,
        issue_id: issue_id.to_string(),
        vcad_commit: scorecard.vcad_commit.clone(),
        openagents_commit: scorecard.openagents_commit.clone(),
        generated_from_scorecard: scorecard_path.to_string(),
        reference_fixture_path: reference_fixture_path.to_string(),
        reference_fixture_sha256,
        reference_source: reference.source,
        reference_issue_match,
        reference_commit_match,
        row_set_match,
        contract_set_match,
        deterministic_replay_match,
        rows: snapshot.rows,
        contracts: snapshot.contracts,
        deterministic_signature,
        parity_contracts,
    })
}

fn parity_signature(
    issue_id: &str,
    snapshot: &ReferenceTableSnapshot,
    reference_issue_match: bool,
    reference_commit_match: bool,
    row_set_match: bool,
    contract_set_match: bool,
    deterministic_replay_match: bool,
    reference_fixture_sha256: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(
        serde_json::to_vec(&(
            issue_id,
            snapshot,
            reference_issue_match,
            reference_commit_match,
            row_set_match,
            contract_set_match,
            deterministic_replay_match,
            reference_fixture_sha256,
        ))
        .expect("serialize reference-table parity payload"),
    );
    format!("{:x}", hasher.finalize())[..16].to_string()
}

fn sort_rows(rows: &mut [Value]) {
    rows.sort_by(|left, right| row_sort_key(left).cmp(&row_sort_key(right)));
}

fn row_sort_key(value: &Value) -> String {
    value
        .get("case_id")
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| serde_json::to_string(value).unwrap_or_default())
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::ReferenceTableSnapshot;

    #[test]
    fn snapshot_constructor_sorts_rows_and_contracts() {
        let snapshot = ReferenceTableSnapshot::new(
            vec![json!({"case_id":"b"}), json!({"case_id":"a"})],
            vec!["z".to_string(), "a".to_string()],
        );
        assert_eq!(snapshot.rows[0]["case_id"], "a");
        assert_eq!(snapshot.rows[1]["case_id"], "b");
        assert_eq!(snapshot.contracts, vec!["a".to_string(), "z".to_string()]);
    }
}
