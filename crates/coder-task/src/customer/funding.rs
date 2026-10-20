//! Private selected-customer custody for gateway funding intents and observations.
use super::*;
use jev::{DecisionFunding, DecisionFundingRequest};
use serde_json::json;

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Entry {
    id: String,
    selection: Selection,
    attempted: bool,
    view: Option<DecisionFunding>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[tokio::test]
    async fn lost_invoice_reply_retains_the_original_private_intent_after_restart() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let context: Context = serde_json::from_value(json!({"schema":receipts::purchase::SCHEMA,"account":"customer","workspace":"workspace","payer_workspace":"workspace","tenant":"tenant","credential_reference":"key","membership_epoch":1,"workspace_members_epoch":1,"role":"owner","door":"decision","registry_digest":format!("sha256:{}","a".repeat(64)),"artifact_digest":format!("sha256:{}","b".repeat(64)),"price":{"version":"price","currency":"BTC","policy":"observed-usage-v1","terms_digest":format!("sha256:{}","c".repeat(64)),"maximum_usage_digest":format!("sha256:{}","d".repeat(64)),"maximum_charge":1},"can_invoke":true})).unwrap();
        let quote = json!({"id":"one","context":context,"amount_msat":200_000});
        let digest = digest_request(&quote);
        let view = json!({"schema":"openagents.decision-funding.v1","quote_digest":digest,"record":{"quote":quote,"phase":"quoted","invoice":null,"observation":null},"balance":{"currency":"BTC","credited":0,"reserved":0,"settled":0,"refunded":0,"available":0,"spend_remaining":100,"price_versions":[],"funding_policy_versions":["btc-v1"],"purchased_funding":0,"promotional_credit":0,"reversed_credit":0,"expired_credit":0,"restricted_credit":0,"operator_loss":0,"uncovered_holds":0},"wallet_liquidity":"unknown","earned_usage":false,"production_qualification":"owner_required_O5_O8"});
        let server_context = context.clone();
        let server = tokio::spawn(async move {
            for index in 0..4 {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut request = Vec::new();
                loop {
                    let mut bytes = [0; 2048];
                    let n = socket.read(&mut bytes).await.unwrap();
                    assert!(n > 0);
                    request.extend_from_slice(&bytes[..n]);
                    if let Some(end) = request.windows(4).position(|w| w == b"\r\n\r\n") {
                        let header = String::from_utf8_lossy(&request[..end]).to_ascii_lowercase();
                        let length = header
                            .lines()
                            .find_map(|line| {
                                line.strip_prefix("content-length:")
                                    .and_then(|s| s.trim().parse::<usize>().ok())
                            })
                            .unwrap_or(0);
                        if request.len() >= end + 4 + length {
                            break;
                        }
                    }
                }
                let header = String::from_utf8_lossy(&request);
                assert!(
                    header
                        .to_ascii_lowercase()
                        .contains("authorization: bearer oak_fixture.only")
                );
                let (status, body) = match index {
                    0 | 2 => {
                        assert!(header.starts_with(
                            "GET /v1/workspaces/workspace/purchase-context/decision "
                        ));
                        ("200 OK", serde_json::to_string(&server_context).unwrap())
                    }
                    1 => {
                        assert!(header.starts_with(
                            "POST /v1/workspaces/workspace/decision-funding/decision "
                        ));
                        ("200 OK", view.to_string())
                    }
                    _ => {
                        assert!(header.contains("\"op\":\"issue\""));
                        ("503 Service Unavailable", json!({"error":{"code":"funding_unavailable","message":"fixture-private-invoice-proof"}}).to_string())
                    }
                };
                socket.write_all(format!("HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes()).await.unwrap();
            }
        });
        let dir = tempfile::tempdir().unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        let mut store = Store::open(dir.path()).unwrap();
        store
            .import_credential("fixture", &jev::ApiKey::new("oak_fixture.only"))
            .unwrap();
        store
            .bind(Selection {
                origin,
                credential_alias: "fixture".into(),
                context,
            })
            .unwrap();
        store
            .decision_funding(&DecisionFundingRequest::Quote {
                id: "one".into(),
                amount_msat: 200_000,
            })
            .await
            .unwrap();
        let error = store
            .decision_funding(&DecisionFundingRequest::Issue {
                id: "one".into(),
                approved: digest,
            })
            .await
            .unwrap_err();
        assert!(!error.contains("fixture-private-invoice-proof"));
        server.await.unwrap();
        drop(store);
        let recovered = Store::open(dir.path()).unwrap();
        let history = recovered.funding_history();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0]["outcome_uncertain"], true);
        assert_eq!(
            history[0]["last_verified_view"]["record"]["phase"],
            "quoted"
        );
        assert_eq!(history[0]["selection"]["context"]["workspace"], "workspace");
        assert!(
            !serde_json::to_string(&history)
                .unwrap()
                .contains("fixture-private-invoice-proof")
        );
    }
}
fn key(selection: &Selection, id: &str) -> String {
    digest_request(
        &json!({"origin":selection.origin,"account":selection.context.account,"workspace":selection.context.workspace,"payer":selection.context.payer_workspace,"tenant":selection.context.tenant,"id":id}),
    )
}
pub(super) fn check(entries: &BTreeMap<String, Entry>) -> Result<()> {
    if entries.len() > MAX_PURCHASES {
        return Err("Customer funding history exceeds its bound.".into());
    }
    for (identity, entry) in entries {
        selection_valid(&entry.selection)?;
        if !alias(&entry.id) || *identity != key(&entry.selection, &entry.id) {
            return Err("Customer funding identity changed.".into());
        }
        if let Some(view) = &entry.view {
            check_view(&entry.selection, &entry.id, view)?;
        }
    }
    Ok(())
}
fn check_view(selection: &Selection, id: &str, view: &DecisionFunding) -> Result<()> {
    let context: Context = serde_json::from_value(view.record["quote"]["context"].clone())
        .map_err(|_| "Invalid retained funding context.")?;
    let returned = Selection {
        context: context.clone(),
        ..selection.clone()
    };
    if !same_customer(selection, &returned)
        || context.door != selection.context.door
        || view.record["quote"]["id"] != id
        || view.quote_digest != digest_request(&view.record["quote"])
        || !view.record["observation"]["preimage"].is_null()
    {
        return Err("Funding changed its original customer, resource, or quote.".into());
    }
    Ok(())
}
impl Store {
    /// The server retains invoice creation; uncertain client effects require a read.
    pub async fn decision_funding(
        &mut self,
        request: &DecisionFundingRequest,
    ) -> Result<DecisionFunding> {
        let selected = self.current_selection().await?;
        let id = match request {
            DecisionFundingRequest::Quote { id, .. }
            | DecisionFundingRequest::Issue { id, .. }
            | DecisionFundingRequest::Read { id }
            | DecisionFundingRequest::Reconcile { id } => id,
        };
        if !alias(id) {
            return Err("Funding needs a bounded immutable identity.".into());
        }
        let identity = key(&selected, id);
        let existing = self.book.funding.get(&identity).cloned();
        match request {
            DecisionFundingRequest::Quote { amount_msat, .. } => {
                if let Some(entry) = &existing {
                    if entry.attempted {
                        return Err(
                            "Funding outcome is uncertain; inspect its original identity.".into(),
                        );
                    }
                    if let Some(view) = &entry.view {
                        if view.record["quote"]["amount_msat"] != *amount_msat {
                            return Err("Funding identity has other terms.".into());
                        }
                        return Ok(view.clone());
                    }
                }
            }
            DecisionFundingRequest::Issue { approved, .. } => {
                let entry = existing
                    .as_ref()
                    .ok_or("Review a retained funding quote before approval.")?;
                let view = entry
                    .view
                    .as_ref()
                    .ok_or("Funding quote is unavailable; inspect its original identity.")?;
                let quote_context: Context =
                    serde_json::from_value(view.record["quote"]["context"].clone())
                        .map_err(|_| "Funding quote is unavailable.")?;
                if entry.attempted
                    || *approved != view.quote_digest
                    || quote_context != selected.context
                {
                    return Err(
                        "Funding approval or current rights changed; inspect before continuing."
                            .into(),
                    );
                }
                if view.record["phase"] != "quoted" {
                    return Ok(view.clone());
                }
                if self
                    .book
                    .funding
                    .values()
                    .any(|other| other.attempted && same_payer(&other.selection, &selected))
                {
                    return Err(
                        "This payer has unresolved funding; inspect it before another invoice."
                            .into(),
                    );
                }
            }
            DecisionFundingRequest::Read { .. } | DecisionFundingRequest::Reconcile { .. } => {}
        }
        if existing.is_none() && self.book.funding.len() >= MAX_PURCHASES {
            return Err("Customer funding history reached its bound.".into());
        }
        let mut next = self.book.clone();
        let entry = next.funding.entry(identity.clone()).or_insert(Entry {
            id: id.clone(),
            selection: selected.clone(),
            attempted: false,
            view: None,
        });
        entry.attempted = true;
        self.persist(next)?;
        let view = self
            .client(&selected.origin, &selected.credential_alias)?
            .account()
            .decision_funding(&selected.context.workspace, &selected.context.door, request)
            .await
            .map_err(
                |_| "Funding response is unavailable; its original intent remains uncertain.",
            )?;
        check_view(&selected, id, &view)?;
        let mut next = self.book.clone();
        let entry = next.funding.get_mut(&identity).unwrap();
        entry.attempted = matches!(view.record["phase"].as_str(), Some("unknown" | "issuing"));
        entry.view = Some(view.clone());
        self.persist(next)?;
        Ok(view)
    }
    /// Retained observations remain visible when current credentials are unavailable.
    pub fn funding_history(&self) -> Vec<Value> {
        let Some(selected) = self.selected() else {
            return Vec::new();
        };
        self.book.funding.values().filter(|e|same_customer(&e.selection,selected)).map(|e|json!({"id":e.id,"selection":e.selection,"outcome_uncertain":e.attempted,"last_verified_view":e.view})).collect()
    }
}
