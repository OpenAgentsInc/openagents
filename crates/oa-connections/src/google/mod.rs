//! Google as an integration: its OAuth endpoints and scopes, the Drive,
//! Gmail, and Calendar tools, and the [`Integration`] value that ties
//! them together.

pub mod calendar;
pub mod drive;
pub mod gmail;
pub mod oauth;
pub mod rest;

use crate::core::{AuthTemplate, Integration, SpecKind};

/// The integration's slug.
pub const SLUG: &str = "google";
/// Read every file in the person's Drive (and export Docs and Sheets).
pub const DRIVE_READONLY: &str = "https://www.googleapis.com/auth/drive.readonly";
/// Read every tab of the person's spreadsheets.
pub const SHEETS_READONLY: &str = "https://www.googleapis.com/auth/spreadsheets.readonly";
/// Read the person's mail and labels (search, read). Asked for on its
/// own, when a chat first needs it.
pub const GMAIL_READONLY: &str = "https://www.googleapis.com/auth/gmail.readonly";
/// Make drafts (and send one, only after the person confirms it).
pub const GMAIL_COMPOSE: &str = "https://www.googleapis.com/auth/gmail.compose";
/// Read, label, and change mail; covers reading and drafts too.
pub const GMAIL_MODIFY: &str = "https://www.googleapis.com/auth/gmail.modify";
/// Everything in Gmail; covers every Gmail tool.
pub const GMAIL_FULL: &str = "https://mail.google.com/";
/// Read the person's calendars and events.
pub const CALENDAR_READONLY: &str = "https://www.googleapis.com/auth/calendar.readonly";
/// Make events (only after the person confirms each one).
pub const CALENDAR_EVENTS: &str = "https://www.googleapis.com/auth/calendar.events";
/// Everything in Calendar; covers every Calendar tool.
pub const CALENDAR_FULL: &str = "https://www.googleapis.com/auth/calendar";
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
    /// The Gmail API's root (`/gmail/v1/...` is appended).
    pub gmail: String,
    /// The Calendar API's root (`/calendar/v3/...` is appended).
    pub calendar: String,
}

impl Default for Endpoints {
    fn default() -> Self {
        Self {
            authorize: "https://accounts.google.com/o/oauth2/v2/auth".into(),
            token: "https://oauth2.googleapis.com/token".into(),
            revoke: "https://oauth2.googleapis.com/revoke".into(),
            drive: "https://www.googleapis.com".into(),
            sheets: "https://sheets.googleapis.com".into(),
            gmail: "https://gmail.googleapis.com".into(),
            calendar: "https://www.googleapis.com".into(),
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
            gmail: root.to_owned(),
            calendar: root.to_owned(),
        }
    }
}

/// The incremental-consent features a person can allow one at a time, by
/// the word `/auth/google/connect?scope=` takes, with the scopes each asks
/// for.
pub const FEATURES: [(&str, &[&str]); 6] = [
    ("drive", &[DRIVE_READONLY]),
    ("sheets", &[SHEETS_READONLY]),
    ("gmail", &[GMAIL_READONLY]),
    ("gmail_compose", &[GMAIL_COMPOSE]),
    ("calendar", &[CALENDAR_READONLY]),
    ("calendar_events", &[CALENDAR_EVENTS]),
];

/// The scopes the feature word `word` asks for.
#[must_use]
pub fn feature_scopes(word: &str) -> Option<&'static [&'static str]> {
    FEATURES
        .iter()
        .find(|(name, _)| *name == word)
        .map(|(_, scopes)| *scopes)
}

/// The feature word that grants `tool` when it isn't granted: the
/// narrowest scope that lets it run.
#[must_use]
pub fn feature_for(tool: &crate::core::ToolSpec) -> Option<&'static str> {
    FEATURES
        .iter()
        .find(|(_, scopes)| scopes.iter().any(|s| tool.scopes.iter().any(|t| t == s)))
        .map(|(name, _)| *name)
}

/// Google (Drive and Sheets, Gmail, Calendar) as one integration: one
/// connection covers them all, each tool runs only with a scope it needs,
/// and each scope is asked for when a feature first needs it. Its tools
/// are curated; a whole Google API can be derived from its Discovery
/// document with [`crate::discovery::tools`].
#[must_use]
pub fn integration(endpoints: &Endpoints) -> Integration {
    let mut tools = drive::tool_specs();
    tools.extend(gmail::tool_specs());
    tools.extend(calendar::tool_specs());
    Integration {
        slug: SLUG.into(),
        name: "Google".into(),
        description: "The person's Google Drive files (Docs, Sheets, PDFs, folders), Gmail, and Google Calendar.".into(),
        spec: SpecKind::GoogleDiscovery {
            url: "https://www.googleapis.com/discovery/v1/apis/drive/v3/rest".into(),
        },
        auth: AuthTemplate::OAuth2 {
            authorize_url: endpoints.authorize.clone(),
            token_url: endpoints.token.clone(),
            scopes: FEATURES
                .iter()
                .flat_map(|(_, scopes)| scopes.iter().map(|s| (*s).to_owned()))
                .collect(),
        },
        tools,
    }
}
