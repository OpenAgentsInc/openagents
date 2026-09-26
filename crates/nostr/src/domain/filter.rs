use std::collections::BTreeMap;

use serde::{
    Deserialize, Deserializer, Serialize, Serializer,
    ser::{Error as _, SerializeMap},
};
use serde_json::Value;

use super::event::is_indexed_tag_name;
use super::hex::decode_lower_hex;
use super::{DomainError, Event};

/// A single NIP-01 filter. Populated fields are ANDed; values within one
/// field are ORed. Multiple filters are ORed with [`matches_any`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Filter {
    pub ids: Option<Vec<String>>,
    pub authors: Option<Vec<String>>,
    pub kinds: Option<Vec<u16>>,
    pub tags: BTreeMap<String, Vec<String>>,
    pub since: Option<u64>,
    pub until: Option<u64>,
    pub limit: Option<usize>,
    pub search: Option<String>,
}

impl Filter {
    /// Validate exact NIP-01 ID and author selectors. Prefix selectors are
    /// intentionally rejected because current NIP-01 removed prefix matching.
    pub fn validate(&self) -> Result<(), DomainError> {
        if let Some(ids) = &self.ids {
            for id in ids {
                decode_lower_hex::<32>(id, "filter id").map_err(|_| {
                    DomainError::InvalidFilter(
                        "every id must be exactly 64 lowercase hexadecimal characters".to_owned(),
                    )
                })?;
            }
        }
        if let Some(authors) = &self.authors {
            for author in authors {
                decode_lower_hex::<32>(author, "filter author").map_err(|_| {
                    DomainError::InvalidFilter(
                        "every author must be exactly 64 lowercase hexadecimal characters"
                            .to_owned(),
                    )
                })?;
            }
        }
        if self.tags.keys().any(|key| !is_indexed_tag_name(key)) {
            return Err(DomainError::InvalidFilter(
                "tag selectors must be one ASCII letter or an indexed extension name".to_owned(),
            ));
        }
        if let Some(search) = &self.search {
            if search.chars().count() > 256 {
                return Err(DomainError::InvalidFilter(
                    "search must contain at most 256 characters".to_owned(),
                ));
            }
            if search_terms(search).is_empty() {
                return Err(DomainError::InvalidFilter(
                    "search must contain at least one non-extension term".to_owned(),
                ));
            }
        }
        Ok(())
    }

    pub fn matches(&self, event: &Event) -> bool {
        list_matches(self.ids.as_deref(), &event.id)
            && list_matches(self.authors.as_deref(), &event.pubkey)
            && self
                .kinds
                .as_ref()
                .is_none_or(|kinds| kinds.contains(&event.kind))
            && self.since.is_none_or(|since| event.created_at >= since)
            && self.until.is_none_or(|until| event.created_at <= until)
            && self.tags.iter().all(|(key, values)| {
                event.indexed_tags().any(|(event_key, event_value)| {
                    event_key == key && values.iter().any(|v| v == event_value)
                })
            })
            && self
                .search
                .as_ref()
                .is_none_or(|search| search_matches(search, event.kind, &event.content))
    }
}

fn list_matches(values: Option<&[String]>, actual: &str) -> bool {
    values.is_none_or(|values| values.iter().any(|value| value == actual))
}

pub fn matches_any(filters: &[Filter], event: &Event) -> bool {
    filters.iter().any(|filter| filter.matches(event))
}

#[derive(Deserialize)]
struct RawFilter {
    #[serde(default)]
    ids: Option<Vec<String>>,
    #[serde(default)]
    authors: Option<Vec<String>>,
    #[serde(default)]
    kinds: Option<Vec<u16>>,
    #[serde(default)]
    since: Option<u64>,
    #[serde(default)]
    until: Option<u64>,
    #[serde(default)]
    limit: Option<usize>,
    #[serde(default)]
    search: Option<String>,
    // NIP-CW extension fields are intentionally accepted and discarded on
    // WebSocket REQ. The pinned specification explicitly permits this safe
    // degradation: the clean NIP-01 filter is served, with no false bounds
    // overlay that could masquerade as a top-level page.
    #[serde(default)]
    top_level: Option<Value>,
    #[serde(default)]
    include_summaries: Option<Value>,
    #[serde(default)]
    include_aux: Option<Value>,
    #[serde(default)]
    before_id: Option<Value>,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

impl<'de> Deserialize<'de> for Filter {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = RawFilter::deserialize(deserializer)?;
        let _channel_window_extensions = (
            &raw.top_level,
            &raw.include_summaries,
            &raw.include_aux,
            &raw.before_id,
        );
        let mut tags = BTreeMap::new();
        for (field, value) in raw.extra {
            let selector = field
                .strip_prefix('#')
                .filter(|name| is_indexed_tag_name(name));
            let Some(selector) = selector else {
                return Err(serde::de::Error::custom(format!(
                    "unsupported filter field {field:?}"
                )));
            };
            let values =
                serde_json::from_value::<Vec<String>>(value).map_err(serde::de::Error::custom)?;
            tags.insert(selector.to_owned(), values);
        }
        Ok(Self {
            ids: raw.ids,
            authors: raw.authors,
            kinds: raw.kinds,
            tags,
            since: raw.since,
            until: raw.until,
            limit: raw.limit,
            search: raw.search,
        })
    }
}

impl Serialize for Filter {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let field_count = usize::from(self.ids.is_some())
            + usize::from(self.authors.is_some())
            + usize::from(self.kinds.is_some())
            + usize::from(self.since.is_some())
            + usize::from(self.until.is_some())
            + usize::from(self.limit.is_some())
            + usize::from(self.search.is_some())
            + self.tags.len();
        let mut map = serializer.serialize_map(Some(field_count))?;
        if let Some(ids) = &self.ids {
            map.serialize_entry("ids", ids)?;
        }
        if let Some(authors) = &self.authors {
            map.serialize_entry("authors", authors)?;
        }
        if let Some(kinds) = &self.kinds {
            map.serialize_entry("kinds", kinds)?;
        }
        for (key, values) in &self.tags {
            if !is_indexed_tag_name(key) {
                return Err(S::Error::custom(
                    "tag selector must be one ASCII letter or an indexed extension name",
                ));
            }
            map.serialize_entry(&format!("#{key}"), values)?;
        }
        if let Some(since) = self.since {
            map.serialize_entry("since", &since)?;
        }
        if let Some(until) = self.until {
            map.serialize_entry("until", &until)?;
        }
        if let Some(limit) = self.limit {
            map.serialize_entry("limit", &limit)?;
        }
        if let Some(search) = &self.search {
            map.serialize_entry("search", search)?;
        }
        map.end()
    }
}

/// NIP-50 extension tokens are ignored; remaining words are matched using
/// Postgres' simple text-search configuration by the store.
/// Event kinds whose content never enters search, live or historical:
/// gift-wrap ciphertext and access-gated Block records. This list is the
/// one the relay's `search_vector` migration and query predicates use.
pub const SEARCH_EXCLUDED_KINDS: [u16; 15] = [
    78, 1059, 3187, 3188, 21059, 30078, 30174, 30175, 30178, 30179, 30186, 30300, 30350, 30622,
    44200,
];

pub fn search_excludes_kind(kind: u16) -> bool {
    SEARCH_EXCLUDED_KINDS.contains(&kind)
}

/// The NIP-50 search contract this relay implements, for both replay and
/// live delivery: every whitespace-separated term that is not a
/// `key:value` extension must occur as a substring of the content. Letters
/// `A`–`Z` fold to lowercase in the terms and the content; non-ASCII text
/// matches exactly. Kinds in [`SEARCH_EXCLUDED_KINDS`] never match.
pub fn search_matches(search: &str, kind: u16, content: &str) -> bool {
    if search_excludes_kind(kind) {
        return false;
    }
    let terms = search_terms(search);
    if terms.is_empty() {
        return false;
    }
    let content = content.to_ascii_lowercase();
    terms.iter().all(|term| content.contains(term))
}

/// The search terms a NIP-50 `search` string names, ASCII-lowercased, with
/// `key:value` extensions removed.
pub fn search_terms(search: &str) -> Vec<String> {
    search
        .split_whitespace()
        .filter(|term| !term.contains(':'))
        .map(str::to_ascii_lowercase)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{search_matches, search_terms};

    #[test]
    fn search_is_ascii_case_insensitive_substring_matching() {
        assert!(search_matches("cat", 1, "cat"));
        assert!(search_matches("cat", 1, "catwalk"));
        assert!(search_matches("cat", 1, "Cat, sat."));
        assert!(search_matches("CAT walk", 1, "the catwalk"));
        assert!(!search_matches("cat", 1, "dog"));
        assert!(!search_matches("cat", 1, "chát"));
        assert!(!search_matches("cat", 1, "ÇAT"));
        assert!(search_matches("chát", 1, "un chát"));
        assert!(!search_matches("chát", 1, "CHÁT"));
    }

    #[test]
    fn search_never_reads_excluded_kinds() {
        assert!(!search_matches("cat", 1_059, "cat"));
        assert!(!search_matches("cat", 30_078, "cat"));
        assert!(search_matches("cat", 30_023, "cat"));
    }

    #[test]
    fn search_terms_drop_extensions() {
        assert_eq!(search_terms("Cat  nsfw:false Dog"), vec!["cat", "dog"]);
        assert!(search_terms("nsfw:false").is_empty());
    }
}
