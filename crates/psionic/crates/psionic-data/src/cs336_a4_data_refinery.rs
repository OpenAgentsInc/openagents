use std::collections::{BTreeMap, BTreeSet};

use fancy_regex::Regex;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Stable lane identifier for the bounded CS336 A4 data-refinery lane.
pub const CS336_A4_REFERENCE_LANE_ID: &str = "psion_cs336_a4_data_refinery_reference_v1";
/// Claim boundary for the bounded CS336 A4 data-refinery lane.
pub const CS336_A4_REFERENCE_CLAIM_BOUNDARY: &str = "bounded deterministic reference \
     implementations of the model-free Stanford CS336 A4 surface (PII masking, Gopher rules, \
     exact line dedup, MinHash dedup) over in-memory documents only; heuristic scanners are \
     unverified against Stanford fixtures, and HTML extraction, language identification, and \
     model-backed quality/NSFW/toxicity classification are not implemented or claimed";

/// Replacement token for masked email addresses.
pub const CS336_A4_EMAIL_MASK: &str = "|||EMAIL_ADDRESS|||";
/// Replacement token for masked phone numbers.
pub const CS336_A4_PHONE_MASK: &str = "|||PHONE_NUMBER|||";
/// Replacement token for masked IP addresses.
pub const CS336_A4_IP_MASK: &str = "|||IP_ADDRESS|||";

/// Failure for one bounded A4 refinery computation.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum Cs336A4RefineryError {
    /// The corpus is empty where documents are required.
    #[error("empty corpus")]
    EmptyCorpus,
    /// One MinHash parameter is invalid.
    #[error("invalid minhash parameter `{parameter}`: {reason}")]
    InvalidMinhashParameter {
        /// Offending parameter name.
        parameter: &'static str,
        /// Violated requirement.
        reason: &'static str,
    },
}

fn mask_with_pattern(text: &str, pattern: &str, mask: &str) -> (String, usize) {
    let Ok(regex) = Regex::new(pattern) else {
        return (text.to_string(), 0);
    };
    let mut output = String::with_capacity(text.len());
    let mut cursor = 0_usize;
    let mut count = 0_usize;
    for capture in regex.find_iter(text).flatten() {
        output.push_str(&text[cursor..capture.start()]);
        output.push_str(mask);
        cursor = capture.end();
        count += 1;
    }
    output.push_str(&text[cursor..]);
    (output, count)
}

/// Masks email addresses, returning the masked text and the match count.
#[must_use]
pub fn cs336_a4_mask_emails(text: &str) -> (String, usize) {
    mask_with_pattern(
        text,
        r"[A-Za-z0-9._%+\-]+@[A-Za-z0-9.\-]+\.[A-Za-z]{2,}",
        CS336_A4_EMAIL_MASK,
    )
}

/// Masks US-format phone numbers, returning the masked text and count.
#[must_use]
pub fn cs336_a4_mask_phone_numbers(text: &str) -> (String, usize) {
    mask_with_pattern(
        text,
        r"(?<!\d)(\+?1[\s.\-]?)?(\(\d{3}\)|\d{3})[\s.\-]?\d{3}[\s.\-]?\d{4}(?!\d)",
        CS336_A4_PHONE_MASK,
    )
}

/// Masks valid dotted-quad IPv4 addresses, returning the masked text and
/// count. Quads outside 0-255 are left unmasked.
#[must_use]
pub fn cs336_a4_mask_ips(text: &str) -> (String, usize) {
    let Ok(regex) = Regex::new(r"(?<!\d)(\d{1,3})\.(\d{1,3})\.(\d{1,3})\.(\d{1,3})(?!\d)") else {
        return (text.to_string(), 0);
    };
    let mut output = String::with_capacity(text.len());
    let mut cursor = 0_usize;
    let mut count = 0_usize;
    for capture in regex.captures_iter(text).flatten() {
        let Some(whole) = capture.get(0) else {
            continue;
        };
        let valid = (1..=4).all(|index| {
            capture
                .get(index)
                .and_then(|quad| quad.as_str().parse::<u16>().ok())
                .is_some_and(|value| value <= 255)
        });
        output.push_str(&text[cursor..whole.start()]);
        if valid {
            output.push_str(CS336_A4_IP_MASK);
            count += 1;
        } else {
            output.push_str(whole.as_str());
        }
        cursor = whole.end();
    }
    output.push_str(&text[cursor..]);
    (output, count)
}

/// One Gopher-rule verdict with per-rule outcomes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cs336A4GopherVerdict {
    /// Whether the document passes all rules.
    pub passes: bool,
    /// Word count within [50, 100000].
    pub word_count_ok: bool,
    /// Mean word length within [3, 10].
    pub mean_word_length_ok: bool,
    /// Fewer than 30% of lines end with an ellipsis.
    pub ellipsis_lines_ok: bool,
    /// At least 80% of words contain an alphabetic character.
    pub alphabetic_words_ok: bool,
}

/// Applies the bounded Gopher quality rules to one document.
#[must_use]
pub fn cs336_a4_gopher_quality_filter(text: &str) -> Cs336A4GopherVerdict {
    let words: Vec<&str> = text.split_whitespace().collect();
    let word_count = words.len();
    let word_count_ok = (50..=100_000).contains(&word_count);
    let mean_word_length = if word_count == 0 {
        0.0
    } else {
        words.iter().map(|word| word.chars().count()).sum::<usize>() as f64 / word_count as f64
    };
    let mean_word_length_ok = (3.0..=10.0).contains(&mean_word_length);
    let lines: Vec<&str> = text.lines().collect();
    let ellipsis_lines = lines
        .iter()
        .filter(|line| line.trim_end().ends_with("..."))
        .count();
    let ellipsis_lines_ok = if lines.is_empty() {
        true
    } else {
        (ellipsis_lines as f64) / (lines.len() as f64) < 0.30
    };
    let alphabetic_words = words
        .iter()
        .filter(|word| word.chars().any(char::is_alphabetic))
        .count();
    let alphabetic_words_ok = if word_count == 0 {
        false
    } else {
        (alphabetic_words as f64) / (word_count as f64) >= 0.80
    };
    Cs336A4GopherVerdict {
        passes: word_count_ok && mean_word_length_ok && ellipsis_lines_ok && alphabetic_words_ok,
        word_count_ok,
        mean_word_length_ok,
        ellipsis_lines_ok,
        alphabetic_words_ok,
    }
}

/// Removes every line that occurs more than once across the corpus from
/// every document, preserving order otherwise.
pub fn cs336_a4_exact_line_deduplication(
    documents: &[String],
) -> Result<Vec<String>, Cs336A4RefineryError> {
    if documents.is_empty() {
        return Err(Cs336A4RefineryError::EmptyCorpus);
    }
    let mut frequencies: BTreeMap<&str, usize> = BTreeMap::new();
    for document in documents {
        for line in document.lines() {
            *frequencies.entry(line).or_insert(0) += 1;
        }
    }
    Ok(documents
        .iter()
        .map(|document| {
            document
                .lines()
                .filter(|line| frequencies.get(line).copied().unwrap_or(0) == 1)
                .collect::<Vec<&str>>()
                .join("\n")
        })
        .collect())
}

/// One MinHash deduplication report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cs336A4MinhashReport {
    /// Indices of retained documents, ascending.
    pub retained: Vec<usize>,
    /// Indices of removed documents, ascending.
    pub removed: Vec<usize>,
    /// Number of duplicate clusters found.
    pub cluster_count: usize,
}

fn splitmix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = value;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn shingle_set(text: &str, ngrams: usize) -> BTreeSet<u64> {
    let normalized: String = text
        .chars()
        .map(|c| {
            if c.is_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                ' '
            }
        })
        .collect();
    let words: Vec<&str> = normalized.split_whitespace().collect();
    let mut shingles = BTreeSet::new();
    if words.len() < ngrams {
        if !words.is_empty() {
            let mut hash = 0xCBF2_9CE4_8422_2325_u64;
            for word in &words {
                for byte in word.as_bytes() {
                    hash = splitmix64(hash ^ u64::from(*byte));
                }
                hash = splitmix64(hash ^ 0x20);
            }
            shingles.insert(hash);
        }
        return shingles;
    }
    for window in words.windows(ngrams) {
        let mut hash = 0xCBF2_9CE4_8422_2325_u64;
        for word in window {
            for byte in word.as_bytes() {
                hash = splitmix64(hash ^ u64::from(*byte));
            }
            hash = splitmix64(hash ^ 0x20);
        }
        shingles.insert(hash);
    }
    shingles
}

fn exact_jaccard(a: &BTreeSet<u64>, b: &BTreeSet<u64>) -> f64 {
    if a.is_empty() && b.is_empty() {
        return 1.0;
    }
    let intersection = a.intersection(b).count();
    let union = a.len() + b.len() - intersection;
    if union == 0 {
        0.0
    } else {
        intersection as f64 / union as f64
    }
}

struct UnionFind {
    parent: Vec<usize>,
}

impl UnionFind {
    fn new(size: usize) -> Self {
        Self {
            parent: (0..size).collect(),
        }
    }

    fn find(&mut self, node: usize) -> usize {
        let mut root = node;
        while self.parent[root] != root {
            root = self.parent[root];
        }
        let mut walker = node;
        while self.parent[walker] != root {
            let next = self.parent[walker];
            self.parent[walker] = root;
            walker = next;
        }
        root
    }

    fn union(&mut self, a: usize, b: usize) {
        let root_a = self.find(a);
        let root_b = self.find(b);
        if root_a != root_b {
            self.parent[root_a.max(root_b)] = root_a.min(root_b);
        }
    }
}

/// Deduplicates documents by MinHash with LSH banding and exact-Jaccard
/// verification, keeping the lowest-index representative per cluster.
pub fn cs336_a4_minhash_deduplication(
    documents: &[String],
    num_hashes: usize,
    num_bands: usize,
    ngrams: usize,
    jaccard_threshold: f64,
) -> Result<Cs336A4MinhashReport, Cs336A4RefineryError> {
    if documents.is_empty() {
        return Err(Cs336A4RefineryError::EmptyCorpus);
    }
    if num_hashes == 0 || num_bands == 0 || !num_hashes.is_multiple_of(num_bands) {
        return Err(Cs336A4RefineryError::InvalidMinhashParameter {
            parameter: "num_bands",
            reason: "num_hashes must be a positive multiple of num_bands",
        });
    }
    if ngrams == 0 {
        return Err(Cs336A4RefineryError::InvalidMinhashParameter {
            parameter: "ngrams",
            reason: "ngrams must be positive",
        });
    }
    if !(0.0..=1.0).contains(&jaccard_threshold) {
        return Err(Cs336A4RefineryError::InvalidMinhashParameter {
            parameter: "jaccard_threshold",
            reason: "threshold must lie in [0, 1]",
        });
    }
    let shingles: Vec<BTreeSet<u64>> = documents
        .iter()
        .map(|document| shingle_set(document, ngrams))
        .collect();
    // Deterministic seeded hash family: signature[h] = min over shingles of
    // splitmix64(shingle ^ seed_h).
    let signatures: Vec<Vec<u64>> = shingles
        .iter()
        .map(|set| {
            (0..num_hashes)
                .map(|hash_index| {
                    let seed = splitmix64(0xA11C_E000_0000_0000_u64 ^ hash_index as u64);
                    set.iter()
                        .map(|shingle| splitmix64(shingle ^ seed))
                        .min()
                        .unwrap_or(u64::MAX)
                })
                .collect()
        })
        .collect();
    let rows_per_band = num_hashes / num_bands;
    let mut union_find = UnionFind::new(documents.len());
    for band in 0..num_bands {
        let mut buckets: BTreeMap<Vec<u64>, Vec<usize>> = BTreeMap::new();
        for (document_index, signature) in signatures.iter().enumerate() {
            let start = band * rows_per_band;
            let key = signature[start..start + rows_per_band].to_vec();
            buckets.entry(key).or_default().push(document_index);
        }
        for candidates in buckets.values() {
            for pair in candidates.windows(2) {
                let (a, b) = (pair[0], pair[1]);
                if exact_jaccard(&shingles[a], &shingles[b]) >= jaccard_threshold {
                    union_find.union(a, b);
                }
            }
        }
    }
    let mut retained = Vec::new();
    let mut removed = Vec::new();
    let mut roots: BTreeSet<usize> = BTreeSet::new();
    for index in 0..documents.len() {
        let root = union_find.find(index);
        roots.insert(root);
        if root == index {
            retained.push(index);
        } else {
            removed.push(index);
        }
    }
    let cluster_count = roots
        .iter()
        .filter(|root| union_find.parent.iter().filter(|p| *p == *root).count() > 1)
        .count();
    Ok(Cs336A4MinhashReport {
        retained,
        removed,
        cluster_count,
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    #[test]
    fn emails_are_masked_with_counts() {
        let (masked, count) =
            cs336_a4_mask_emails("Contact alice@example.com or bob.smith+tag@mail.co.uk.");
        assert_eq!(count, 2);
        assert_eq!(
            masked,
            format!("Contact {CS336_A4_EMAIL_MASK} or {CS336_A4_EMAIL_MASK}.")
        );
    }

    #[test]
    fn phone_numbers_in_common_us_formats_are_masked() {
        let (masked, count) = cs336_a4_mask_phone_numbers(
            "Call (415) 555-2671, 415-555-2671, 415.555.2671, or +1 415 555 2671.",
        );
        assert_eq!(count, 4);
        assert!(!masked.contains("555"));
        let (unchanged, none) = cs336_a4_mask_phone_numbers("Order #123456789012 stays.");
        assert_eq!(none, 0);
        assert!(unchanged.contains("123456789012"));
    }

    #[test]
    fn only_valid_ipv4_addresses_are_masked() {
        let (masked, count) = cs336_a4_mask_ips("Server 192.168.1.1 not 999.1.1.1 nor 1.2.3.999.");
        assert_eq!(count, 1);
        assert!(masked.contains(CS336_A4_IP_MASK));
        assert!(masked.contains("999.1.1.1"));
        assert!(masked.contains("1.2.3.999"));
    }

    #[test]
    fn gopher_rules_pass_ordinary_prose_and_fail_degenerate_documents() {
        let good = "the quick brown fox jumps over the lazy dog and keeps running through fields "
            .repeat(8);
        assert!(cs336_a4_gopher_quality_filter(&good).passes);
        let too_short = "tiny document";
        let verdict = cs336_a4_gopher_quality_filter(too_short);
        assert!(!verdict.passes);
        assert!(!verdict.word_count_ok);
        let ellipsis_heavy = (0..10)
            .map(|i| {
                if i < 5 {
                    "this line trails off into nothing at all..."
                } else {
                    "this line ends cleanly with proper words here"
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
            + &" filler words to reach the count".repeat(10);
        let verdict = cs336_a4_gopher_quality_filter(&ellipsis_heavy);
        assert!(!verdict.ellipsis_lines_ok);
        let numeric = "12345 67890 ".repeat(40);
        let verdict = cs336_a4_gopher_quality_filter(&numeric);
        assert!(!verdict.alphabetic_words_ok);
    }

    #[test]
    fn exact_line_dedup_removes_corpus_repeated_lines_only() {
        let documents = vec![
            "unique first line\nshared boilerplate\nanother unique line".to_string(),
            "shared boilerplate\ndifferent content here".to_string(),
        ];
        let deduplicated = cs336_a4_exact_line_deduplication(&documents).expect("deduplicates");
        assert_eq!(deduplicated[0], "unique first line\nanother unique line");
        assert_eq!(deduplicated[1], "different content here");
    }

    #[test]
    fn minhash_clusters_near_duplicates_and_keeps_distinct_documents() {
        let base = "the quick brown fox jumps over the lazy dog while the sun sets slowly over \
                    the quiet hills and the river runs cold";
        let near = format!("{base} tonight");
        let distinct = "completely different text about compilers schedulers and slot \
                        allocation in integer machines with channels and gates";
        let documents = vec![base.to_string(), near, distinct.to_string()];
        let report =
            cs336_a4_minhash_deduplication(&documents, 64, 16, 3, 0.6).expect("deduplicates");
        assert_eq!(report.retained, vec![0, 2]);
        assert_eq!(report.removed, vec![1]);
        assert_eq!(report.cluster_count, 1);
    }

    #[test]
    fn minhash_parameters_refuse_invalid_configurations() {
        let documents = vec!["text".to_string()];
        assert!(matches!(
            cs336_a4_minhash_deduplication(&documents, 10, 3, 2, 0.5),
            Err(Cs336A4RefineryError::InvalidMinhashParameter {
                parameter: "num_bands",
                ..
            })
        ));
        assert!(matches!(
            cs336_a4_minhash_deduplication(&documents, 8, 2, 0, 0.5),
            Err(Cs336A4RefineryError::InvalidMinhashParameter {
                parameter: "ngrams",
                ..
            })
        ));
        assert!(matches!(
            cs336_a4_minhash_deduplication(&documents, 8, 2, 2, 1.5),
            Err(Cs336A4RefineryError::InvalidMinhashParameter {
                parameter: "jaccard_threshold",
                ..
            })
        ));
    }

    #[test]
    fn minhash_is_deterministic() {
        let documents = vec![
            "alpha beta gamma delta epsilon zeta".to_string(),
            "alpha beta gamma delta epsilon eta".to_string(),
        ];
        let a = cs336_a4_minhash_deduplication(&documents, 32, 8, 2, 0.4).expect("runs");
        let b = cs336_a4_minhash_deduplication(&documents, 32, 8, 2, 0.4).expect("runs");
        assert_eq!(a, b);
    }
}
