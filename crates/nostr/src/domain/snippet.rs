//! NIP-C0 code snippets.
//!
//! Kind `1337` stores source text in `content`. `l` is the programming
//! language in lowercase. `name`, `extension`, `description`, and `runtime`
//! are optional. `license` is an SPDX identifier and may repeat. `dep` names
//! a dependency. `repo` is an `http://` or `https://` URL, or a kind `30617`
//! repository address with an optional relay hint.
//!
//! An `l` tag is also a NIP-32 self-label. The relay does not replace the
//! snippet, run it, or fetch the repository. Kind `1337` is not added to
//! the NIP-11 list.

use std::str::FromStr;

use super::{DomainError, Event, ReplacementAddress};

const KIND: u16 = 1_337;
const REPO_KIND: u16 = 30_617;

/// Where a snippet says it came from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SnippetRepo {
    Url(String),
    Announcement {
        address: ReplacementAddress,
        relay: Option<String>,
    },
}

/// One SPDX license. `text` is an optional URL of the license text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SnippetLicense {
    pub spdx: String,
    pub text: Option<String>,
}

/// A kind `1337` code snippet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Snippet {
    pub content: String,
    pub languages: Vec<String>,
    pub name: Option<String>,
    pub extension: Option<String>,
    pub description: Option<String>,
    pub runtime: Option<String>,
    pub licenses: Vec<SnippetLicense>,
    pub dependencies: Vec<String>,
    pub repos: Vec<SnippetRepo>,
}

fn invalid(reason: &str) -> DomainError {
    DomainError::InvalidEvent(reason.to_owned())
}

fn is_relay(value: &str) -> bool {
    (value.starts_with("ws://") || value.starts_with("wss://"))
        && value.len() <= 2_048
        && !value.chars().any(char::is_whitespace)
}

fn valid_http_url(value: &str) -> bool {
    let Some(rest) = value
        .strip_prefix("http://")
        .or_else(|| value.strip_prefix("https://"))
    else {
        return false;
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    !authority.is_empty() && value.len() <= 2_048 && !value.chars().any(char::is_whitespace)
}

fn language_ok(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"+#-_.".contains(&byte)
        })
}

fn spdx_ok(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b".-+".contains(&byte))
        && value
            .bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_alphabetic())
}

fn one<'a>(event: &'a Event, name: &str, reason: &str) -> Result<Option<&'a str>, DomainError> {
    let tags = event
        .tags
        .iter()
        .filter(|tag| tag.name() == Some(name))
        .collect::<Vec<_>>();
    if tags.len() > 1 {
        return Err(invalid(reason));
    }
    match tags.first() {
        None => Ok(None),
        Some(tag) => match tag.value() {
            Some(value) if !value.is_empty() => Ok(Some(value)),
            _ => Err(invalid(reason)),
        },
    }
}

/// Read a kind `1337` snippet.
///
/// # Errors
///
/// Returns a sentence when the kind, language, license, or repository is refused.
pub fn open_snippet(event: &Event) -> Result<Snippet, DomainError> {
    if event.kind != KIND {
        return Err(invalid("a code snippet has kind 1337"));
    }
    let mut languages = Vec::new();
    for tag in event.tags.iter().filter(|tag| tag.name() == Some("l")) {
        let Some(value) = tag.value().filter(|value| language_ok(value)) else {
            return Err(invalid(
                "a snippet language is a lowercase name such as javascript or rust",
            ));
        };
        languages.push(value.to_owned());
    }
    let extension = one(
        event,
        "extension",
        "a snippet extension is one value without a leading dot",
    )?;
    if let Some(extension) = extension
        && (extension.starts_with('.')
            || extension.len() > 16
            || !extension
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit()))
    {
        return Err(invalid(
            "a snippet extension is one value without a leading dot",
        ));
    }
    let name = one(event, "name", "a snippet name is one non-empty value")?;
    if let Some(name) = name
        && (name.len() > 256 || name.chars().any(char::is_whitespace))
    {
        return Err(invalid("a snippet name is one non-empty value"));
    }
    let mut licenses = Vec::new();
    for tag in event
        .tags
        .iter()
        .filter(|tag| tag.name() == Some("license"))
    {
        let values = tag.as_slice();
        if values.len() < 2 || values.len() > 3 || !spdx_ok(&values[1]) {
            return Err(invalid(
                "a snippet license is an SPDX identifier and an optional text URL",
            ));
        }
        let text = match values.get(2) {
            None => None,
            Some(value) if valid_http_url(value) => Some(value.clone()),
            Some(_) => {
                return Err(invalid(
                    "a snippet license is an SPDX identifier and an optional text URL",
                ));
            }
        };
        licenses.push(SnippetLicense {
            spdx: values[1].clone(),
            text,
        });
    }
    let mut dependencies = Vec::new();
    for tag in event.tags.iter().filter(|tag| tag.name() == Some("dep")) {
        let Some(value) = tag
            .value()
            .filter(|value| !value.is_empty() && value.len() <= 256)
        else {
            return Err(invalid("a snippet dependency is non-empty"));
        };
        dependencies.push(value.to_owned());
    }
    let mut repos = Vec::new();
    for tag in event.tags.iter().filter(|tag| tag.name() == Some("repo")) {
        let values = tag.as_slice();
        if values.len() < 2 || values.len() > 3 {
            return Err(invalid(
                "a snippet repo is an HTTP URL or a 30617 repository address",
            ));
        }
        if valid_http_url(&values[1]) {
            if values.len() != 2 {
                return Err(invalid(
                    "a snippet repo is an HTTP URL or a 30617 repository address",
                ));
            }
            repos.push(SnippetRepo::Url(values[1].clone()));
            continue;
        }
        let address = ReplacementAddress::from_str(&values[1])
            .map_err(|_| invalid("a snippet repo is an HTTP URL or a 30617 repository address"))?;
        if address.kind != REPO_KIND || address.identifier.is_empty() {
            return Err(invalid(
                "a snippet repo is an HTTP URL or a 30617 repository address",
            ));
        }
        let relay = match values.get(2) {
            None => None,
            Some(value) if is_relay(value) => Some(value.clone()),
            Some(_) => {
                return Err(invalid(
                    "a repository announcement relay hint must be ws:// or wss://",
                ));
            }
        };
        repos.push(SnippetRepo::Announcement { address, relay });
    }
    Ok(Snippet {
        content: event.content.clone(),
        languages,
        name: name.map(str::to_owned),
        extension: extension.map(str::to_owned),
        description: one(
            event,
            "description",
            "a snippet description is one non-empty value",
        )?
        .map(str::to_owned),
        runtime: one(event, "runtime", "a snippet runtime is one non-empty value")?
            .map(str::to_owned),
        licenses,
        dependencies,
        repos,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        DomainError, EventClass, RelaySigner, Tag, compare_replacement, open_labeling,
    };

    fn signer() -> RelaySigner {
        RelaySigner::from_secret_hex(&"c0".repeat(32)).unwrap()
    }

    #[test]
    fn a_snippet_keeps_its_source_and_does_not_replace() {
        let text = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../nips/official/C0.md"
        ))
        .unwrap();
        assert!(text.contains("kind:1337"));
        assert!(text.contains("javascript"));
        assert!(text.contains("hello-world.js"));
        assert!(text.contains("MIT"));
        assert!(text.contains("30617:"));
        let row = crate::lane::PROVEN
            .iter()
            .find(|row| row.file == "C0.md")
            .unwrap();
        assert_eq!(row.status, "configured-and-proven");
        assert!(
            crate::lane::SHAPES
                .iter()
                .all(|shape| shape.file != "C0.md")
        );

        let content =
            "function helloWorld() {\n  console.log('Hello, Nostr!');\n}\n\nhelloWorld();";
        let event = signer().sign(
            1_700_000_000,
            KIND,
            vec![
                Tag::new(vec!["l".into(), "javascript".into()]),
                Tag::new(vec!["extension".into(), "js".into()]),
                Tag::new(vec!["name".into(), "hello-world.js".into()]),
                Tag::new(vec![
                    "description".into(),
                    "A basic JavaScript function that prints 'Hello, Nostr!' to the console".into(),
                ]),
                Tag::new(vec!["runtime".into(), "node v18.15.0".into()]),
                Tag::new(vec!["license".into(), "MIT".into()]),
                Tag::new(vec![
                    "repo".into(),
                    "https://github.com/nostr-protocol/nostr".into(),
                ]),
            ],
            content.into(),
        );
        event.validate_structure().unwrap();
        assert_eq!(event.class(), EventClass::Regular);
        assert!(matches!(
            compare_replacement(&event, &event),
            Err(DomainError::NotReplaceable)
        ));
        let snippet = open_snippet(&event).unwrap();
        assert_eq!(snippet.content, content);
        assert_eq!(snippet.languages, vec!["javascript".to_owned()]);
        assert_eq!(snippet.extension.as_deref(), Some("js"));
        assert_eq!(snippet.name.as_deref(), Some("hello-world.js"));
        assert_eq!(snippet.runtime.as_deref(), Some("node v18.15.0"));
        assert_eq!(snippet.licenses[0].spdx, "MIT");
        assert!(matches!(
            &snippet.repos[0],
            SnippetRepo::Url(url) if url == "https://github.com/nostr-protocol/nostr"
        ));
        let labeling = open_labeling(&event).unwrap();
        assert!(labeling.self_labeled);
        assert_eq!(labeling.labels[0].value, "javascript");
        assert_eq!(labeling.labels[0].namespace, "ugc");

        let pubkey = "34".repeat(32);
        let announced = signer().sign(
            1_700_000_100,
            KIND,
            vec![
                Tag::new(vec!["l".into(), "rust".into()]),
                Tag::new(vec!["extension".into(), "rs".into()]),
                Tag::new(vec![
                    "license".into(),
                    "Apache-2.0".into(),
                    "https://www.apache.org/licenses/LICENSE-2.0".into(),
                ]),
                Tag::new(vec!["license".into(), "MIT".into()]),
                Tag::new(vec!["dep".into(), "serde".into()]),
                Tag::new(vec![
                    "repo".into(),
                    format!("30617:{pubkey}:nostr"),
                    "wss://relay.example".into(),
                ]),
            ],
            "fn main() {}\n".into(),
        );
        announced.validate_structure().unwrap();
        let announced = open_snippet(&announced).unwrap();
        assert_eq!(announced.licenses.len(), 2);
        assert_eq!(announced.dependencies, vec!["serde".to_owned()]);
        assert!(matches!(
            &announced.repos[0],
            SnippetRepo::Announcement { address, relay }
                if address.kind == 30_617
                    && address.identifier == "nostr"
                    && relay.as_deref() == Some("wss://relay.example")
        ));

        let upper = signer().sign(
            1_700_000_200,
            KIND,
            vec![Tag::new(vec!["l".into(), "JavaScript".into()])],
            content.into(),
        );
        assert!(upper.validate_structure().is_err());
        let dotted = signer().sign(
            1_700_000_300,
            KIND,
            vec![Tag::new(vec!["extension".into(), ".js".into()])],
            content.into(),
        );
        assert!(dotted.validate_structure().is_err());
    }
}
