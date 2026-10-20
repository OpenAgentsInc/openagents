//! Mentions in a message's Markdown — files, folders, chats, skills,
//! commands, and attachments — and the chip labels they show as. The
//! composer edits over this projection; the transcript draws sent messages
//! with it.

use std::collections::{HashMap, HashSet};
use std::ops::Range;

use gpui::SharedString;
use openagents_proto::file_mentions::FILE_MENTION_SCHEME;
use unicode_segmentation::UnicodeSegmentation;

use crate::composer_chips::{CHIP_ICON_SLOT, CHIP_TRAILING_PAD, ChipKind};
use crate::composer_markdown;

// Nonbreaking spaces give chips their side inset, shaped in Geist like the
// rest of the padding, while preserving the chip's atomic wrapping and
// source/caret projection.
pub const MENTION_SIDE_PAD: &str = "\u{00A0}";

/// A strict, local-only Markdown representation of a file mention. The
/// underlying prompt always contains this form; the editor projects it to a
/// chip for display without leaking a second data model into submission.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileMentionLink {
    pub range: Range<usize>,
    pub basename: String,
    pub path: String,
    pub kind: ChipKind,
    /// The draft number when this chip references a staged attachment.
    pub attachment: Option<u32>,
}

impl FileMentionLink {
    /// The character that marks this kind of reference.
    pub fn prefix(&self) -> char {
        match self.kind {
            ChipKind::Skill => '$',
            ChipKind::Command => '/',
            _ => '@',
        }
    }

    pub fn is_dir(&self) -> bool {
        self.kind == ChipKind::Directory
    }
}

pub fn file_mention_links(text: &str) -> Vec<FileMentionLink> {
    openagents_proto::file_mentions::file_mention_links(text)
        .into_iter()
        .map(|link| FileMentionLink {
            range: link.range,
            basename: link.basename,
            path: link.path,
            kind: if link.is_dir {
                ChipKind::Directory
            } else {
                ChipKind::File
            },
            attachment: None,
        })
        .collect()
}

#[derive(Debug, Clone, Default)]
pub struct TextProjection {
    pub display: String,
    pub mentions: Vec<(FileMentionLink, Range<usize>)>,
    pub mappings: Vec<(Range<usize>, Range<usize>)>,
}

/// A chip is a compact identity, with the complete path available in its tooltip.
/// Truncate by grapheme so emoji and combining marks remain intact.
pub fn compact_chip_label(label: &str) -> String {
    compact_chip_label_with_limit(label, 32)
}

pub fn compact_chip_label_with_limit(label: &str, limit: usize) -> String {
    let graphemes: Vec<_> = label.graphemes(true).collect();
    if graphemes.len() <= limit {
        return label.to_string();
    }
    format!(
        "{}…{}",
        graphemes[..limit - 14].concat(),
        graphemes[graphemes.len() - 12..].concat()
    )
}

/// Preserve distinctions established by path disambiguation when shortening.
pub fn compact_chip_labels(labels: &[String]) -> Vec<String> {
    let mut shortened: Vec<_> = labels
        .iter()
        .map(|label| compact_chip_label(label))
        .collect();
    let mut limit = 32usize;
    loop {
        let mut distinct: HashMap<&str, std::collections::HashSet<&str>> = HashMap::new();
        for (shown, full) in shortened.iter().zip(labels) {
            distinct.entry(shown).or_default().insert(full);
        }
        let ambiguous: Vec<_> = shortened
            .iter()
            .enumerate()
            .filter_map(|(ix, shown)| (distinct[shown.as_str()].len() > 1).then_some(ix))
            .collect();
        if ambiguous.is_empty() {
            return shortened;
        }
        limit = limit.saturating_mul(2);
        for ix in ambiguous {
            shortened[ix] = compact_chip_label_with_limit(&labels[ix], limit);
        }
    }
}

impl TextProjection {
    pub fn new(raw: &str) -> Self {
        Self::project(raw, None, false, None)
    }

    /// `live` names the attachments the draft still holds; a chip outside it is
    /// a dangling reference and shows as its plain label.
    pub fn rich(raw: &str, active: Option<Range<usize>>, live: Option<&HashSet<u32>>) -> Self {
        Self::project(raw, active, true, live)
    }

    pub fn project(
        raw: &str,
        active: Option<Range<usize>>,
        compact: bool,
        live: Option<&HashSet<u32>>,
    ) -> Self {
        let mut links = file_mention_links(raw);
        links.extend(
            openagents_proto::invocation::invocation_links(raw)
                .into_iter()
                .map(|(range, invocation)| FileMentionLink {
                    range,
                    basename: if compact && invocation.prefix() == '$' {
                        skill_display_name(invocation.name())
                    } else {
                        invocation.name().to_string()
                    },
                    path: invocation.detail(),
                    kind: if invocation.prefix() == '/' {
                        ChipKind::Command
                    } else {
                        ChipKind::Skill
                    },
                    attachment: None,
                }),
        );
        links.extend(
            openagents_proto::chat_mentions::chat_mention_links(raw)
                .into_iter()
                .map(|link| FileMentionLink {
                    range: link.range,
                    basename: link.title,
                    path: link.chat_id,
                    kind: ChipKind::Chat,
                    attachment: None,
                }),
        );
        links.extend(
            openagents_proto::attachment_mentions::attachment_mentions(raw)
                .into_iter()
                .map(|mention| FileMentionLink {
                    range: mention.range,
                    // A file attachment's chip names the file, so its icon
                    // follows the name; an image has no path to follow.
                    path: if mention.is_image {
                        String::new()
                    } else {
                        mention.label.clone()
                    },
                    basename: mention.label,
                    kind: if mention.is_image {
                        ChipKind::Image
                    } else {
                        ChipKind::File
                    },
                    attachment: Some(mention.index),
                }),
        );
        links.sort_by_key(|link| link.range.start);
        let labels = mention_display_labels(&links);
        let labels = if compact {
            compact_chip_labels(&labels)
        } else {
            labels
        };
        let mut projection = Self::default();
        let mut edits: Vec<(Range<usize>, String, Option<FileMentionLink>)> = links
            .into_iter()
            .zip(labels)
            .map(|(link, label)| {
                let label = label.replace(' ', "\u{00A0}");
                if let (Some(index), Some(live)) = (link.attachment, live)
                    && !live.contains(&index)
                {
                    return (link.range.clone(), link.basename.clone(), None);
                }
                let pad = MENTION_SIDE_PAD;
                let display = format!("{pad}{CHIP_ICON_SLOT}{label}{CHIP_TRAILING_PAD}");
                (link.range.clone(), display, Some(link))
            })
            .collect();
        if let Some(active) = active {
            let links: Vec<_> = edits.iter().map(|(range, _, _)| range.clone()).collect();
            for (range, replacement) in composer_markdown::decorations(raw, active) {
                let candidate = links.partition_point(|link| link.end <= range.start);
                if !links
                    .get(candidate)
                    .is_some_and(|link| link.start < range.end)
                {
                    edits.push((range, replacement, None));
                }
            }
        }
        edits.sort_by_key(|(r, _, _)| r.start);
        let mut raw_at = 0;
        for (range, replacement, link) in edits {
            if range.start < raw_at {
                continue;
            }
            projection.display.push_str(&raw[raw_at..range.start]);
            let start = projection.display.len();
            projection.display.push_str(&replacement);
            let display = start..projection.display.len();
            if let Some(link) = link {
                projection.mentions.push((link, display.clone()));
            }
            raw_at = range.end;
            projection.mappings.push((range, display));
        }
        projection.display.push_str(&raw[raw_at..]);
        projection
    }

    pub fn raw_to_display(&self, raw: usize) -> usize {
        let ix = self.mappings.partition_point(|(range, _)| range.end <= raw);
        if let Some((range, display)) = self.mappings.get(ix) {
            if raw > range.start {
                return display.start;
            }
        }
        let (raw_at, display_at) = ix
            .checked_sub(1)
            .map(|previous| (&self.mappings[previous].0, &self.mappings[previous].1))
            .map_or((0, 0), |(range, display)| (range.end, display.end));
        display_at + raw.saturating_sub(raw_at)
    }

    pub fn display_to_raw(&self, display_offset: usize) -> usize {
        // Equal zero-width boundaries are consumed together so clicks on the
        // first rendered character land after nested hidden opening markers.
        let ix = self
            .mappings
            .partition_point(|(_, display)| display.end <= display_offset);
        if let Some((range, display)) = self.mappings.get(ix) {
            if display_offset > display.start {
                return if display_offset - display.start < display.len() / 2 {
                    range.start
                } else {
                    range.end
                };
            }
        }
        let (raw_at, display_at) = ix
            .checked_sub(1)
            .map(|previous| (&self.mappings[previous].0, &self.mappings[previous].1))
            .map_or((0, 0), |(range, display)| (range.end, display.end));
        raw_at + display_offset.saturating_sub(display_at)
    }

    pub fn normalize_range(&self, range: Range<usize>) -> Range<usize> {
        if range.is_empty() {
            for (link, _) in &self.mentions {
                if link.range.start < range.start && range.start < link.range.end {
                    let midpoint = link.range.start + link.range.len() / 2;
                    let at = if range.start < midpoint {
                        link.range.start
                    } else {
                        link.range.end
                    };
                    return at..at;
                }
            }
            return range;
        }
        let mut normalized = range;
        for (link, _) in &self.mentions {
            if normalized.start < link.range.end && normalized.end > link.range.start {
                normalized.start = normalized.start.min(link.range.start);
                normalized.end = normalized.end.max(link.range.end);
            }
        }
        normalized
    }

    pub fn previous_boundary(&self, raw: usize) -> Option<usize> {
        self.mentions
            .iter()
            .find_map(|(link, _)| (raw == link.range.end).then_some(link.range.start))
    }

    pub fn next_boundary(&self, raw: usize) -> Option<usize> {
        self.mentions
            .iter()
            .find_map(|(link, _)| (raw == link.range.start).then_some(link.range.end))
    }
}

/// Basenames are compact in the common case. When the same basename appears
/// more than once, use the shortest unique path suffix so chips remain
/// distinguishable without always expanding to full paths.
pub fn mention_display_labels(links: &[FileMentionLink]) -> Vec<String> {
    // Only references with the same visible name can need disambiguation.
    // Repeated references share the result instead of rescanning the draft.
    let mut groups: HashMap<(char, &str), Vec<&FileMentionLink>> = HashMap::new();
    for link in links {
        groups
            .entry((link.prefix(), &link.basename))
            .or_default()
            .push(link);
    }
    let mut labels: HashMap<(char, &str, &str), String> = HashMap::new();
    links
        .iter()
        .map(|link| {
            labels
                .entry((link.prefix(), &link.basename, &link.path))
                .or_insert_with(|| {
                    let duplicates: Vec<_> = groups[&(link.prefix(), link.basename.as_str())]
                        .iter()
                        .filter(|other| other.path != link.path)
                        .collect();
                    // A chat's "path" is its id: always show its title.
                    if duplicates.is_empty() || link.kind == ChipKind::Chat {
                        return link.basename.clone();
                    }
                    let parts: Vec<_> = link.path.split('/').collect();
                    let suffix = (1..=parts.len())
                        .map(|count| parts[parts.len() - count..].join("/"))
                        .find(|suffix| {
                            if link.prefix() != '@' {
                                duplicates.iter().all(|other| !other.path.ends_with(suffix))
                            } else {
                                let suffix: Vec<_> = suffix.split('/').collect();
                                duplicates.iter().all(|other| {
                                    !other
                                        .path
                                        .split('/')
                                        .rev()
                                        .take(suffix.len())
                                        .eq(suffix.iter().rev().copied())
                                })
                            }
                        })
                        .unwrap_or_else(|| link.path.clone());
                    if link.prefix() == '@' {
                        suffix
                    } else {
                        format!("{} · {suffix}", link.basename)
                    }
                })
                .clone()
        })
        .collect()
}

/// One chip in a *sent* message: its byte range over the projected display
/// string (`@label` between side bearings). The transcript renders these
/// read-only — no editing state, no tooltip machinery.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SentMentionSpan {
    pub range: Range<usize>,
    /// Full workspace-relative path (labels can be shortened to basenames).
    pub path: SharedString,
    pub kind: ChipKind,
    /// The draft number of an attachment chip.
    pub attachment: Option<u32>,
    /// The upload an attachment chip names, once the transcript pairs them.
    pub upload: Option<SharedString>,
}

/// Cheap probe: whether `raw` could hold a file, skill or attachment chip.
pub fn has_mention_scheme(raw: &str) -> bool {
    [
        "zeron-file:",
        "zeron-chat:",
        "zeron-image:",
        "zeron-attachment:",
        "zeron-invoke:",
    ]
    .iter()
    .any(|scheme| raw.contains(scheme))
        || raw.contains(FILE_MENTION_SCHEME)
        || raw.contains(openagents_proto::invocation::INVOCATION_SCHEME)
        || raw.contains(openagents_proto::attachment_mentions::IMAGE_MENTION_SCHEME)
        || raw.contains(openagents_proto::attachment_mentions::ATTACHMENT_MENTION_SCHEME)
        || raw.contains(openagents_proto::chat_mentions::CHAT_MENTION_SCHEME)
}

/// Project a sent message's raw Markdown for transcript display: mention links
/// collapse to the same chip labels the composer shows, everything else passes
/// through untouched. `None` when the text has no valid mention — the
/// substring probe keeps ordinary prompts on the zero-allocation path, so this
/// is safe to call for every user row.
pub fn sent_mention_display(raw: &str) -> Option<(String, Vec<SentMentionSpan>)> {
    if !has_mention_scheme(raw) {
        return None;
    }
    let projection = TextProjection::new(raw);
    if projection.mentions.is_empty() {
        return None;
    }
    let spans = projection
        .mentions
        .iter()
        .map(|(link, display)| SentMentionSpan {
            range: display.clone(),
            path: SharedString::from(format!(
                "{}{}",
                link.path,
                if link.is_dir() { "/" } else { "" }
            )),
            kind: link.kind,
            attachment: link.attachment,
            upload: None,
        })
        .collect();
    Some((projection.display, spans))
}

/// Human-readable presentation only; invocation names and paths stay canonical.
pub fn skill_display_name(name: &str) -> String {
    name.rsplit(':')
        .next()
        .unwrap_or(name)
        .split(|c: char| c == '-' || c == '_' || c.is_whitespace())
        .filter(|word| !word.is_empty())
        .map(|word| {
            let mut chars = word.chars();
            let first = chars.next().unwrap();
            first.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase()
        })
        .collect::<Vec<_>>()
        .join(" ")
}
