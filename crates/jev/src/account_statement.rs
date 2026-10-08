//! Joined reads reuse the current workspace usage and private export surface.
use super::Account;
use crate::{Error, Result};
use receipts::funding_units::Unit;
use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[derive(Clone, Debug, Default)]
pub struct JoinedStatementQuery {
    pub cursor: Option<String>,
    pub limit: Option<usize>,
    pub after_earning: Option<i64>,
    pub after_payout: Option<i64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JoinedCustomerStatement {
    pub schema: String,
    pub origin: String,
    pub customer: String,
    pub workspace: String,
    pub unit: Unit,
    pub unit_scale: u64,
    pub balance: Option<Value>,
    pub snapshot: String,
    pub rows: Vec<Value>,
    pub next: Option<String>,
    pub scanned: usize,
    pub disclosure: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JoinedStatementView {
    pub native_workspace: String,
    pub statement: JoinedCustomerStatement,
    pub payee: Option<Value>,
    pub payee_disclosure: String,
    pub source_attribution: Vec<Value>,
    pub attribution_disclosure: String,
    pub native_projection: Vec<Value>,
    pub native_projection_disclosure: String,
}
impl Account<'_> {
    /// Read or export one bounded page under current source and earnings rights.
    pub async fn joined_statement(
        &self,
        workspace: &str,
        query: &JoinedStatementQuery,
        export: bool,
    ) -> Result<JoinedStatementView> {
        if !receipts::team_policy::identifier(workspace)
            || matches!(workspace, "." | "..")
            || query.limit.is_some_and(|n| n == 0 || n > 100)
            || query.cursor.as_ref().is_some_and(|c| c.len() > 4096)
            || query.after_earning.is_some_and(|n| n < 0)
            || query.after_payout.is_some_and(|n| n < 0)
        {
            return Err(Error::Config("Invalid bounded statement selection.".into()));
        }
        let mut params = url::form_urlencoded::Serializer::new(String::new());
        params.append_pair("joined", "true");
        if let Some(c) = &query.cursor {
            params.append_pair("cursor", c);
        }
        if let Some(n) = query.limit {
            params.append_pair("limit", &n.to_string());
        }
        if let Some(n) = query.after_earning {
            params.append_pair("after_earning", &n.to_string());
        }
        if let Some(n) = query.after_payout {
            params.append_pair("after_payout", &n.to_string());
        }
        let path = format!(
            "/v1/workspaces/{workspace}/usage{}?{}",
            if export { "/export" } else { "" },
            params.finish()
        );
        let raw = self
            .client
            .request_private_headers(Method::GET, &path, None, &reqwest::header::HeaderMap::new())
            .await?;
        let invalid = || Error::ResponseValidation {
            status: 200,
            field_path: "joined-statement".into(),
            body: None,
            request_id: None,
        };
        if raw.bytes.len() > 128 * 1024 {
            return Err(invalid());
        }
        let view: JoinedStatementView =
            serde_json::from_slice(&raw.bytes).map_err(|_| invalid())?;
        if view.native_workspace != workspace
            || view.statement.schema != "openagents.joined-statement.v1"
            || view.statement.rows.len() > query.limit.unwrap_or(50)
            || view.statement.scanned > 10_000
            || view.native_projection.len() > view.statement.rows.len()
            || view.source_attribution.len() > 128
            || view.statement.unit != Unit::Millisatoshis
            || view.statement.unit_scale != Unit::Millisatoshis.scale()
            || view.statement.snapshot.len() != 64
            || !view
                .statement
                .snapshot
                .bytes()
                .all(|b| b.is_ascii_hexdigit())
            || view.statement.next.as_ref().is_some_and(|c| c.len() > 4096)
        {
            return Err(invalid());
        }
        Ok(view)
    }
}
