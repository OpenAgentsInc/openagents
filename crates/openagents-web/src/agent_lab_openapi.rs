//! Published routes for the account-private Agent Lab provider.
use serde_json::{Value, json};
pub(crate) fn extend(doc: &mut Value) {
    for (path, methods) in [
        ("/v1/labs/kinds", vec!["get"]),
        ("/v1/labs/hosts", vec!["get", "post"]),
        ("/v1/labs/documents", vec!["get", "post"]),
        ("/v1/labs/documents/{id}", vec!["get", "put", "delete"]),
        ("/v1/labs/documents/{id}/jobs", vec!["post"]),
        ("/v1/labs/documents/{id}/promote", vec!["post"]),
        ("/v1/labs/jobs", vec!["get"]),
        ("/v1/labs/jobs/{id}", vec!["get"]),
        ("/v1/labs/jobs/{id}/actions", vec!["post"]),
        ("/v1/labs/jobs/{id}/host", vec!["post"]),
    ] {
        for method in methods {
            let parameters = if path.contains("{id}") {
                vec![json!({"name":"id","in":"path","required":true,"schema":{"type":"string"}})]
            } else {
                vec![]
            };
            let mut op = json!({"tags":["Labs"],"security":[{"appBearer":[]}],"parameters":parameters,"description":"Account-private Agent Lab documents, finite test plans and reports. See docs/agent-lab/README.md and the Rust SDK labs module for typed bodies. No execution on reads, document save, card rendering or promotion. A supporting host prepares isolated inputs before using the normal agent assignment service. Mutations retain request_id receipts; changed retries conflict. Updates require expected_revision. Maximum request 192 KiB; per-account storage 16 MiB, 64 documents, 64 versions each, 128 runs, 8 active jobs and 8192 receipts. Trials are bounded to 32 and 30 minutes. Cost may be unknown. Host evidence is an account claim, not remote attestation.","responses":{"200":{"description":"Account-owned result; Cache-Control: no-store, private."},"400":{"description":"Invalid fields or bounds."},"401":{"description":"Sign in required."},"403":{"description":"Access refused."},"404":{"description":"Unknown account-owned reference."},"409":{"description":"Version or request conflict."},"413":{"description":"Body too large."},"429":{"description":"Storage full."},"503":{"description":"Service unavailable."}}});
            if method == "get" && (path.ends_with("documents") || path.ends_with("jobs")) {
                op["parameters"] = json!([{"name":"after","in":"query","schema":{"type":"string"}},{"name":"host","in":"query","schema":{"type":"string"}}]);
            }
            if method != "get" {
                op["requestBody"] = json!({"required":true,"content":{"application/json":{"schema":{"type":"object"}}}});
            }
            if doc["paths"][path].is_null() {
                doc["paths"][path] = json!({});
            }
            doc["paths"][path][method] = op;
        }
    }
}
