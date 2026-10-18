//! Region lookup and S3 endpoint derivation.

use crate::error::{Error, Result};

/// Known regions and their control-plane base URLs.
pub const REGION_URLS: &[(&str, &str)] = &[
    (
        "aws-us-east-1",
        "https://control.green.us-east-1.aws.prod.archil.com",
    ),
    (
        "aws-us-west-2",
        "https://control.green.us-west-2.aws.prod.archil.com",
    ),
    (
        "aws-eu-west-1",
        "https://control.green.eu-west-1.aws.prod.archil.com",
    ),
    (
        "gcp-us-central1",
        "https://control.blue.us-central1.gcp.prod.archil.com",
    ),
];

/// Resolve a region name to its control-plane base URL.
pub fn resolve_base_url(region: &str) -> Result<String> {
    REGION_URLS
        .iter()
        .find(|(name, _)| *name == region)
        .map(|(_, url)| (*url).to_string())
        .ok_or_else(|| {
            let valid: Vec<&str> = REGION_URLS.iter().map(|(n, _)| *n).collect();
            Error::Config(format!(
                "Unknown region \"{region}\". Valid regions: {}",
                valid.join(", ")
            ))
        })
}

/// Derive the S3-compatible endpoint from a control-plane base URL by swapping
/// a leading `control.` hostname segment for `s3.`. Returns `None` if the URL
/// cannot be parsed. A host without a `control.` prefix is returned unchanged.
pub fn derive_s3_base_url(control_base_url: &str) -> Option<String> {
    let mut url = url::Url::parse(control_base_url).ok()?;
    let host = url.host_str()?.to_string();
    if let Some(rest) = host.strip_prefix("control.") {
        url.set_host(Some(&format!("s3.{rest}"))).ok()?;
    }
    Some(url.to_string().trim_end_matches('/').to_string())
}
