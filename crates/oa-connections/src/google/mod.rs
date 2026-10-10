//! Google as an integration: its OAuth endpoints and scopes, the Drive
//! tools, and the [`Integration`] value that ties them together.

pub mod drive;
pub mod oauth;

use crate::core::{AuthTemplate, Integration, SpecKind};

/// The integration's slug.
pub const SLUG: &str = "google";
/// Read every file in the person's Drive (and export Docs and Sheets).
pub const DRIVE_READONLY: &str = "https://www.googleapis.com/auth/drive.readonly";
/// Read every tab of the person's spreadsheets.
pub const SHEETS_READONLY: &str = "https://www.googleapis.com/auth/spreadsheets.readonly";
/// Who the person is, to show which Google account is connected.
pub const IDENTITY: [&str; 2] = ["openid", "email"];

/// Where Google is. Tests point every field at a fake.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Endpoints {
    pub authorize: String,
    pub token: String,
    pub revoke: String,
    /// The Drive API's root (`/drive/v3/...` is appended).
    pub drive: String,
    /// The Sheets API's root (`/v4/...` is appended).
    pub sheets: String,
}

impl Default for Endpoints {
    fn default() -> Self {
        Self {
            authorize: "https://accounts.google.com/o/oauth2/v2/auth".into(),
            token: "https://oauth2.googleapis.com/token".into(),
            revoke: "https://oauth2.googleapis.com/revoke".into(),
            drive: "https://www.googleapis.com".into(),
            sheets: "https://sheets.googleapis.com".into(),
        }
    }
}

impl Endpoints {
    /// Every endpoint under one root, as a fake Google serves them.
    #[must_use]
    pub fn at(root: &str) -> Self {
        let root = root.trim_end_matches('/');
        Self {
            authorize: format!("{root}/o/oauth2/v2/auth"),
            token: format!("{root}/token"),
            revoke: format!("{root}/revoke"),
            drive: root.to_owned(),
            sheets: root.to_owned(),
        }
    }
}

/// Google Drive (and Sheets) as one integration: one connection covers
/// both. Its tools are the curated read-only Drive tools; the whole Drive
/// API can be derived from its Discovery document with
/// [`crate::discovery::tools`].
#[must_use]
pub fn integration(endpoints: &Endpoints) -> Integration {
    Integration {
        slug: SLUG.into(),
        name: "Google Drive".into(),
        description: "Files in the person's Google Drive: Docs, Sheets, PDFs, and folders.".into(),
        spec: SpecKind::GoogleDiscovery {
            url: "https://www.googleapis.com/discovery/v1/apis/drive/v3/rest".into(),
        },
        auth: AuthTemplate::OAuth2 {
            authorize_url: endpoints.authorize.clone(),
            token_url: endpoints.token.clone(),
            scopes: vec![DRIVE_READONLY.into(), SHEETS_READONLY.into()],
        },
        tools: drive::tool_specs(),
    }
}
