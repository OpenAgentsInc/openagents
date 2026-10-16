//! Exact-operation reads for an environment owner. These additive methods keep
//! the existing Boat API and its instance actions unchanged.
use super::{GceError, Rest, Result};
use serde_json::Value;
use std::future::Future;

pub trait EnvironmentCompute: Send + Sync + 'static {
    /// Submit once, returning the actual accepted operation without retrying.
    fn create_environment(
        &self,
        zone: &str,
        request: &str,
        body: Value,
    ) -> impl Future<Output = Result<Value>> + Send;
    fn environment_instance(
        &self,
        zone: &str,
        name: &str,
    ) -> impl Future<Output = Result<Option<Value>>> + Send;
    fn environment_image(&self, image: &str) -> impl Future<Output = Result<Option<Value>>> + Send;
    fn environment_disk(
        &self,
        zone: &str,
        name: &str,
    ) -> impl Future<Output = Result<Option<Value>>> + Send;
    /// Every retained operation matching the exact original client UUID or target
    /// incarnation. An empty result is not proof that an operation never ran.
    fn environment_operations(
        &self,
        zone: &str,
        field: &str,
        value: &str,
    ) -> impl Future<Output = Result<Vec<Value>>> + Send;
}
fn name(s: &str) -> Result<()> {
    if s.is_empty()
        || s.len() > 63
        || !s
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    {
        return Err(GceError::new(
            400,
            "identity",
            "invalid environment resource name",
        ));
    }
    Ok(())
}
impl EnvironmentCompute for Rest {
    async fn create_environment(&self, zone: &str, request: &str, body: Value) -> Result<Value> {
        name(zone)?;
        if request.len() != 36 || !request.bytes().all(|b| b.is_ascii_hexdigit() || b == b'-') {
            return Err(GceError::new(
                400,
                "identity",
                "invalid environment request identity",
            ));
        }
        self.call(
            reqwest::Method::POST,
            &format!("{}?requestId={request}", self.zone_url(zone, "instances")),
            Some(body),
        )
        .await
    }
    async fn environment_instance(&self, zone: &str, instance: &str) -> Result<Option<Value>> {
        name(zone)?;
        name(instance)?;
        match self
            .call(
                reqwest::Method::GET,
                &self.zone_url(zone, &format!("instances/{instance}")),
                None,
            )
            .await
        {
            Ok(v) => Ok(Some(v)),
            Err(e) if e.not_found() => Ok(None),
            Err(e) => Err(e),
        }
    }
    async fn environment_image(&self, image: &str) -> Result<Option<Value>> {
        let parts = image.split('/').collect::<Vec<_>>();
        if parts.len() != 5
            || parts[0] != "projects"
            || parts[2] != "global"
            || parts[3] != "images"
        {
            return Err(GceError::new(
                400,
                "identity",
                "an exact image path is required",
            ));
        }
        name(parts[1])?;
        name(parts[4])?;
        match self
            .call(
                reqwest::Method::GET,
                &format!("{}/{image}", super::API),
                None,
            )
            .await
        {
            Ok(v) => Ok(Some(v)),
            Err(e) if e.not_found() => Ok(None),
            Err(e) => Err(e),
        }
    }
    async fn environment_disk(&self, zone: &str, disk: &str) -> Result<Option<Value>> {
        name(zone)?;
        name(disk)?;
        match self
            .call(
                reqwest::Method::GET,
                &self.zone_url(zone, &format!("disks/{disk}")),
                None,
            )
            .await
        {
            Ok(v) => Ok(Some(v)),
            Err(e) if e.not_found() => Ok(None),
            Err(e) => Err(e),
        }
    }
    async fn environment_operations(
        &self,
        zone: &str,
        field: &str,
        value: &str,
    ) -> Result<Vec<Value>> {
        name(zone)?;
        if !matches!(field, "clientOperationId" | "targetId")
            || value.is_empty()
            || value.len() > 64
            || !value.bytes().all(|b| b.is_ascii_hexdigit() || b == b'-')
        {
            return Err(GceError::new(
                400,
                "identity",
                "invalid original operation identity",
            ));
        }
        let mut token = String::new();
        let mut seen = std::collections::BTreeSet::new();
        let mut operations = Vec::new();
        for _ in 0..64 {
            let mut url = reqwest::Url::parse(&self.zone_url(zone, "operations"))
                .map_err(|_| GceError::new(400, "identity", "invalid operation URL"))?;
            url.query_pairs_mut()
                .append_pair("filter", &format!("{field} = \"{value}\""))
                .append_pair("maxResults", "100");
            if !token.is_empty() {
                url.query_pairs_mut().append_pair("pageToken", &token);
            }
            let response = self.call(reqwest::Method::GET, url.as_str(), None).await?;
            if let Some(items) = response["items"].as_array() {
                operations.extend(items.iter().cloned());
            }
            if operations.len() > 4096 {
                return Err(GceError::new(413, "limit", "too many original operations"));
            }
            token = response["nextPageToken"].as_str().unwrap_or("").into();
            if token.is_empty() {
                return Ok(operations);
            }
            if !seen.insert(token.clone()) {
                return Err(GceError::new(502, "pagination", "operation page repeated"));
            }
        }
        Err(GceError::new(
            413,
            "limit",
            "too many original operation pages",
        ))
    }
}
