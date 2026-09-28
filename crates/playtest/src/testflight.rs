//! TestFlight feedback from App Store Connect, as triage inbox entries.
//!
//! A TestFlight tester sends feedback with **Send Beta Feedback** or by
//! sharing a screenshot, and a crash can carry a comment too. App Store
//! Connect keeps both as beta feedback submissions (screenshot and crash).
//! `openagents playtest testflight` reads them with the App Store Connect
//! API; this module turns one API page into [`Feedback`] values and each
//! into an issue draft, with no network.
//!
//! A submission names the tester's Apple account (email and name). None of
//! it is kept: [`Feedback`] has no field for it. A TestFlight tester has no
//! Nostr key, so an accepted TestFlight entry is recorded in the triage log
//! but backs no playtest award ([`crate::triage::Log::acceptances`] leaves
//! it out). The tester's comment was written to OpenAgents through Apple,
//! not for a public issue, so a draft never quotes it: the triager writes
//! the issue in their own words.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::triage::{Draft, LABEL, PARAPHRASE};

/// The label TestFlight drafts carry besides `playtest` and the build.
pub const SOURCE_LABEL: &str = "source:testflight";

/// Which kind of submission it is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Source {
    /// Feedback with one or more screenshots.
    Screenshot,
    /// A crash the tester chose to send, with its crash log.
    Crash,
}

impl Source {
    /// The App Store Connect resource type.
    #[must_use]
    pub fn resource(self) -> &'static str {
        match self {
            Self::Screenshot => "betaFeedbackScreenshotSubmissions",
            Self::Crash => "betaFeedbackCrashSubmissions",
        }
    }
}

/// One screenshot's temporary download link.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Image {
    pub url: String,
    pub width: u32,
    pub height: u32,
}

/// One TestFlight submission, without the tester's identity.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Feedback {
    /// The submission's App Store Connect ID.
    pub id: String,
    pub source: Source,
    /// ISO 8601, as App Store Connect gives it.
    pub created: String,
    /// The tester's comment, private.
    pub comment: Option<String>,
    /// `iPhone18_2`.
    pub device: String,
    pub os_version: String,
    /// The build number (`CFBundleVersion`), such as `16`.
    pub build: Option<String>,
    /// The App Store Connect build ID, to look up its version.
    #[serde(skip)]
    pub build_id: Option<String>,
    /// The marketing version, such as `1.0.0`, once looked up.
    pub app_version: Option<String>,
    /// Screenshot links; they expire, so the command downloads them.
    pub images: Vec<Image>,
}

impl Feedback {
    /// `1.0.0 (16)`, or as much of it as is known.
    #[must_use]
    pub fn build_label(&self) -> String {
        match (&self.app_version, &self.build) {
            (Some(version), Some(build)) => format!("{version} ({build})"),
            (None, Some(build)) => format!("build {build}"),
            _ => "an unknown build".into(),
        }
    }

    /// `TF-1A2B3C4D`: from the SHA-256 of the submission ID, so it never
    /// collides with a report code by accident and repeats for the same
    /// submission.
    #[must_use]
    pub fn code(&self) -> String {
        code(&self.id)
    }
}

/// The triage code for the submission with App Store Connect ID `id`.
#[must_use]
pub fn code(id: &str) -> String {
    let hash = Sha256::digest(id.as_bytes());
    format!(
        "TF-{}",
        hash.iter()
            .take(4)
            .map(|b| format!("{b:02X}"))
            .collect::<String>()
    )
}

fn text(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .filter(|s| !s.trim().is_empty())
}

/// One page of submissions: the feedback on it and the next page's link.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Page {
    pub feedback: Vec<Feedback>,
    pub next: Option<String>,
}

/// Reads one App Store Connect list response of `source` submissions,
/// requested with `include=build`, keeping no tester identity.
///
/// # Errors
///
/// When the response isn't a JSON:API list of that resource.
pub fn parse_page(source: Source, body: &Value) -> Result<Page, String> {
    let data = body
        .get("data")
        .and_then(Value::as_array)
        .ok_or("the App Store Connect response has no data list")?;
    let builds: Vec<(&str, String)> = body
        .get("included")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|item| item["type"] == "builds")
        .filter_map(|item| {
            Some((
                item["id"].as_str()?,
                item["attributes"]["version"].as_str()?.to_owned(),
            ))
        })
        .collect();
    let mut feedback = Vec::new();
    for item in data {
        if item["type"] != source.resource() {
            return Err(format!("expected {} items", source.resource()));
        }
        let id = text(item, "id").ok_or("a submission has no id")?;
        let attributes = &item["attributes"];
        let build_id = item["relationships"]["build"]["data"]["id"]
            .as_str()
            .map(str::to_owned);
        let build = build_id.as_deref().and_then(|b| {
            builds
                .iter()
                .find(|(id, _)| *id == b)
                .map(|(_, v)| v.clone())
        });
        let images = attributes["screenshots"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|shot| {
                Some(Image {
                    url: shot["url"].as_str()?.to_owned(),
                    width: u32::try_from(shot["width"].as_u64()?).ok()?,
                    height: u32::try_from(shot["height"].as_u64()?).ok()?,
                })
            })
            .collect();
        feedback.push(Feedback {
            id,
            source,
            created: text(attributes, "createdDate").unwrap_or_default(),
            comment: text(attributes, "comment"),
            device: text(attributes, "deviceModel").unwrap_or_else(|| "unknown device".into()),
            os_version: text(attributes, "osVersion").unwrap_or_default(),
            build,
            build_id,
            app_version: None,
            images,
        });
    }
    Ok(Page {
        feedback,
        next: body["links"]["next"].as_str().map(str::to_owned),
    })
}

/// `playtest`, `source:testflight`, and the build label when it is known.
#[must_use]
pub fn labels(feedback: &Feedback) -> Vec<String> {
    let mut labels = vec![LABEL.to_owned(), SOURCE_LABEL.to_owned()];
    if let (Some(version), Some(build)) = (&feedback.app_version, &feedback.build) {
        labels.push(format!("build:{version}-{build}"));
    }
    labels
}

/// Drafts the GitHub issue for a submission. It never quotes the comment
/// and never embeds a screenshot or the crash log: the triager writes the
/// title and text, and the attachments stay in the private drafts folder.
#[must_use]
pub fn draft(feedback: &Feedback) -> Draft {
    let what = match feedback.source {
        Source::Screenshot => "screenshot feedback",
        Source::Crash => "a crash",
    };
    let mut attachments = Vec::new();
    if !feedback.images.is_empty() {
        attachments.push(format!(
            "- {} screenshot(s) are in the private drafts folder; they aren't published.",
            feedback.images.len()
        ));
    }
    if feedback.source == Source::Crash {
        attachments.push(
            "- The crash log is in the private drafts folder; quote only the frames you need."
                .into(),
        );
    }
    if feedback.comment.is_some() {
        attachments.push("- The tester's comment is in the private drafts folder.".into());
    }
    let body = format!(
        "TestFlight feedback `{code}` ({what}), sent from OpenAgents {build} on iOS {os} ({device}) on {created}.\n\n\
         Severity: P? (P0 loses money, leaks a key, or bricks the app; P1 blocks a scripted task; P2 hurts with a way around; P3 polish).\n\n\
         ## What happened\n\n{PARAPHRASE}\n\n\
         ## Expected\n\n{PARAPHRASE}\n\n\
         ## Steps\n\n{PARAPHRASE}\n\n\
         ## Attachments\n\n{attachments}\n\n\
         TestFlight feedback comes through Apple without a Nostr key, so it can't back a playtest award by itself.\n",
        code = feedback.code(),
        build = feedback.build_label(),
        os = feedback.os_version,
        device = feedback.device,
        created = feedback.created,
        attachments = if attachments.is_empty() {
            "- None.".to_owned()
        } else {
            attachments.join("\n")
        },
    );
    Draft {
        code: feedback.code(),
        title: format!(
            "testflight: {what} on {} (write a title)",
            feedback.build_label()
        ),
        body,
        labels: labels(feedback),
    }
}

#[cfg(test)]
mod tests;
