//! Who may spend a Kiln runner's time (#12449).
//!
//! A runner reads an admission list, one key a line, hex or `npub`, with an
//! optional tier after it:
//!
//! ```text
//! # kiln-runner admission list
//! npub1...                 # 30 jobs a day (the runner's member quota)
//! npub1... jobs=200        # 200 jobs a day
//! npub1... unlimited       # no daily quota (maintainers, the Grid Foreman)
//! npub1... each=10         # each agent this key authorizes: 10 jobs a day
//! ```
//!
//! `each=N` is for a key that vouches for many agents, such as the site's
//! Kiln key, which authorizes each signed-in player's browser key at the
//! Grid terminal (#12488): every agent gets its own N jobs a day instead of
//! sharing one quota.
//!
//! A job is admitted when its request is signed by a listed key, or by an
//! agent key whose request carries a valid NIP-OA `auth` tag and a mandate
//! allowing `kiln.test` from a listed owner ([`crate::identity`]); an agent
//! draws from its owner's quota. Keys on the runner's admin
//! list are unlimited. A runner started with `--open` also takes jobs from
//! unlisted keys, each within the small anonymous quota, which is how the
//! local demo runs.
//!
//! Quotas count jobs per UTC day per holder (the owner for an agent), and
//! the count lives in the runner's state directory so a restart keeps it.

use std::collections::{BTreeMap, BTreeSet};

use nostr::domain::Event;
use serde::{Deserialize, Serialize};

/// The daily quota of one listed key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tier {
    /// The runner's member quota.
    Member,
    /// A stated number of jobs a day.
    Jobs(u32),
    /// No daily quota.
    Unlimited,
    /// A stated number of jobs a day for each agent the key authorizes,
    /// counted per agent.
    Each(u32),
}

/// The admission list: listed keys (lowercase hex) and their tiers.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Roster {
    pub keys: BTreeMap<String, Tier>,
}

/// Lowercase hex for a hex key or an `npub`.
#[must_use]
pub fn key_hex(text: &str) -> Option<String> {
    let text = text.trim();
    if text.len() == 64 && text.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Some(text.to_lowercase());
    }
    nostr::nip19::decode_npub(text)
        .ok()
        .map(|bytes| bytes.iter().map(|b| format!("{b:02x}")).collect())
}

impl Roster {
    /// Parse the list's text.
    ///
    /// # Errors
    ///
    /// The first line that names no key or an unknown tier.
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut keys = BTreeMap::new();
        for (number, line) in text.lines().enumerate() {
            let line = line.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            let mut words = line.split_whitespace();
            let first = words.next().unwrap_or_default();
            let key = key_hex(first)
                .ok_or_else(|| format!("line {}: {first} is not a key", number + 1))?;
            let tier = match words.next() {
                None => Tier::Member,
                Some("unlimited") => Tier::Unlimited,
                Some(word) => {
                    let number_of = |prefix: &str| {
                        word.strip_prefix(prefix)
                            .and_then(|n| n.parse::<u32>().ok())
                    };
                    match (number_of("jobs="), number_of("each=")) {
                        (Some(n), _) => Tier::Jobs(n),
                        (_, Some(n)) => Tier::Each(n),
                        _ => {
                            return Err(format!(
                                "line {}: {word} is not a tier; use jobs=N, each=N, or unlimited",
                                number + 1
                            ));
                        }
                    }
                }
            };
            if let Some(extra) = words.next() {
                return Err(format!("line {}: unexpected {extra}", number + 1));
            }
            keys.insert(key, tier);
        }
        Ok(Self { keys })
    }
}

/// How a runner admits jobs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Policy {
    /// The admission list, read again for every job.
    pub roster: Roster,
    /// Keys on the self-edit admin list: always admitted, unlimited.
    pub admins: BTreeSet<String>,
    /// Whether unlisted keys get the anonymous quota.
    pub open: bool,
    /// Jobs a day for a listed key without a stated tier.
    pub member_jobs: u32,
    /// Jobs a day for an unlisted key on an open runner.
    pub anonymous_jobs: u32,
}

/// Who a job's time is charged to, and its daily limit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Holder {
    /// The key the quota counts against: the requester, or its owner.
    pub key: String,
    /// Jobs a day; `None` is unlimited.
    pub limit: Option<u32>,
    /// How the requester got in, for the runner's log.
    pub via: &'static str,
}

/// The owner a request names, when its `auth` tag and mandate are valid
/// and the mandate allows `kiln.test` ([`crate::identity`]).
#[must_use]
pub fn request_owner(request: &Event) -> Option<String> {
    crate::identity::authorized_owner(request).ok()
}

impl Policy {
    fn limit(&self, tier: Tier) -> Option<u32> {
        match tier {
            Tier::Member => Some(self.member_jobs),
            Tier::Jobs(n) | Tier::Each(n) => Some(n),
            Tier::Unlimited => None,
        }
    }

    /// Decide whether `requester`, optionally an agent of `owner`, may run
    /// a job here.
    ///
    /// # Errors
    ///
    /// A plain reason for a refused key.
    pub fn admit(&self, requester: &str, owner: Option<&str>) -> Result<Holder, String> {
        if self.admins.contains(requester) {
            return Ok(Holder {
                key: requester.to_string(),
                limit: None,
                via: "admin",
            });
        }
        if let Some(tier) = self.roster.keys.get(requester) {
            return Ok(Holder {
                key: requester.to_string(),
                limit: self.limit(*tier),
                via: "listed",
            });
        }
        if let Some(owner) = owner {
            if self.admins.contains(owner) {
                return Ok(Holder {
                    key: owner.to_string(),
                    limit: None,
                    via: "agent of an admin",
                });
            }
            if let Some(Tier::Each(n)) = self.roster.keys.get(owner) {
                return Ok(Holder {
                    key: requester.to_string(),
                    limit: Some(*n),
                    via: "agent of a listed key, counted alone",
                });
            }
            if let Some(tier) = self.roster.keys.get(owner) {
                return Ok(Holder {
                    key: owner.to_string(),
                    limit: self.limit(*tier),
                    via: "agent of a listed key",
                });
            }
        }
        if self.open {
            return Ok(Holder {
                key: requester.to_string(),
                limit: Some(self.anonymous_jobs),
                via: "open",
            });
        }
        let npub = hex_npub(requester);
        Err(format!(
            "this runner only takes jobs from keys on its list; ask an admin to add {npub}"
        ))
    }
}

fn hex_npub(hex: &str) -> String {
    let mut bytes = [0u8; 32];
    for (i, b) in bytes.iter_mut().enumerate() {
        *b = u8::from_str_radix(hex.get(2 * i..2 * i + 2).unwrap_or("00"), 16).unwrap_or(0);
    }
    nostr::nip19::encode_npub(&bytes)
}

/// Jobs run today, by holder.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Usage {
    /// The UTC day (`unix / 86400`) the counts are for.
    pub day: u64,
    pub counts: BTreeMap<String, u32>,
}

impl Usage {
    /// Count one job for `holder` at `unix`, unless it is over its limit.
    ///
    /// # Errors
    ///
    /// A plain reason when the day's quota is spent.
    pub fn charge(&mut self, holder: &Holder, unix: u64) -> Result<(), String> {
        let day = unix / 86_400;
        if day != self.day {
            self.day = day;
            self.counts.clear();
        }
        let count = self.counts.entry(holder.key.clone()).or_insert(0);
        if let Some(limit) = holder.limit
            && *count >= limit
        {
            return Err(format!(
                "this key has used its {limit} runs for today; try again after midnight UTC"
            ));
        }
        *count += 1;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nostr::domain::{Tag, mint_owner_attestation};

    const A: &str = "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";

    fn policy(text: &str, open: bool) -> Policy {
        Policy {
            roster: Roster::parse(text).unwrap(),
            admins: BTreeSet::new(),
            open,
            member_jobs: 2,
            anonymous_jobs: 1,
        }
    }

    #[test]
    fn the_list_reads_keys_and_tiers() {
        let npub = hex_npub(A);
        let roster = Roster::parse(&format!(
            "# runners\n{npub} jobs=5\n{} unlimited\n{}\n",
            "b".repeat(64),
            "C".repeat(64)
        ))
        .unwrap();
        assert_eq!(roster.keys[A], Tier::Jobs(5));
        assert_eq!(roster.keys[&"b".repeat(64)], Tier::Unlimited);
        assert_eq!(roster.keys[&"c".repeat(64)], Tier::Member);
        assert!(Roster::parse("nope").is_err());
        assert!(Roster::parse(&format!("{A} forever")).is_err());
    }

    #[test]
    fn only_listed_keys_and_their_agents_get_in() {
        let p = policy(&format!("{A}\n"), false);
        assert_eq!(p.admit(A, None).unwrap().limit, Some(2));
        let stranger = "d".repeat(64);
        let refused = p.admit(&stranger, None).unwrap_err();
        assert!(refused.contains("npub1"), "{refused}");
        let agent = p.admit(&stranger, Some(A)).unwrap();
        assert_eq!(agent.key, A, "an agent draws from its owner's quota");
        assert!(p.admit(&stranger, Some(&"e".repeat(64))).is_err());
        let open = policy("", true);
        assert_eq!(open.admit(&stranger, None).unwrap().limit, Some(1));
        let mut admin = policy("", false);
        admin.admins.insert(stranger.clone());
        assert_eq!(admin.admit(&stranger, None).unwrap().limit, None);
    }

    #[test]
    fn a_vouching_key_gives_each_agent_its_own_quota() {
        let p = policy(&format!("{A} each=3\n"), false);
        assert_eq!(
            Roster::parse(&format!("{A} each=3")).unwrap().keys[A],
            Tier::Each(3)
        );
        let (one, two) = ("d".repeat(64), "e".repeat(64));
        let first = p.admit(&one, Some(A)).unwrap();
        let second = p.admit(&two, Some(A)).unwrap();
        assert_eq!((first.key.as_str(), first.limit), (one.as_str(), Some(3)));
        assert_ne!(first.key, second.key, "each agent is counted alone");
        // The vouching key still can't run jobs for a stranger without
        // vouching for it.
        assert!(p.admit(&one, None).is_err());
    }

    #[test]
    fn a_quota_runs_out_and_resets_the_next_day() {
        let p = policy(&format!("{A}\n"), false);
        let holder = p.admit(A, None).unwrap();
        let mut usage = Usage::default();
        let day = 1_792_540_800;
        assert!(usage.charge(&holder, day).is_ok());
        assert!(usage.charge(&holder, day + 1).is_ok());
        assert!(usage.charge(&holder, day + 2).is_err());
        assert!(usage.charge(&holder, day + 86_400).is_ok());
    }

    #[test]
    fn a_request_names_its_owner_through_nip_oa() {
        let owner = secp256k1::SecretKey::from_byte_array([7; 32]).unwrap();
        let agent = crate::cj::Key::from_bytes([9; 32]).unwrap();
        let minted =
            mint_owner_attestation(&owner, agent.pubkey(), "created_at<4000000000").unwrap();
        let bare_auth = agent.sign(25_900, vec![minted.tag()], String::new(), 1_792_540_800);
        assert_eq!(
            request_owner(&bare_auth),
            None,
            "an auth tag alone isn't a mandate"
        );
        let credential = crate::identity::mint(
            &owner,
            agent.pubkey(),
            &[crate::identity::KILN_TEST],
            4_000_000_000,
        )
        .unwrap();
        let signed = agent.sign(25_900, credential.tags(), String::new(), 1_792_540_800);
        assert_eq!(
            request_owner(&signed).as_deref(),
            Some(minted.owner_pubkey.as_str())
        );
        let bare = agent.sign(
            25_900,
            vec![Tag::new(vec!["p".into(), A.into()])],
            String::new(),
            1,
        );
        assert_eq!(request_owner(&bare), None);
    }
}
