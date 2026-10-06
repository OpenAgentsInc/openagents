//! Signed quest versions and recomputable XP decisions. Reading grants no authority.
use nostr::{domain::Event, xp};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use xp_ledger::{XpTrust, eval::Documents};

#[cfg(feature = "host")]
pub mod host;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Quest {
    pub title: String,
    pub objective: String,
    pub task: String,
    pub trainer_xp: u64,
    pub trainer_level: u32,
    pub award_decisions: BTreeMap<String, Vec<String>>,
    pub event: String,
    pub referee: String,
    pub address: String,
    pub rule: String,
    pub season: String,
    pub eligibility: String,
    pub requirements: Vec<String>,
    pub counted_xp: u64,
    pub awards: Vec<String>,
    pub evidence: Vec<String>,
    pub decisions: Vec<String>,
}
impl Quest {
    pub fn lines(&self) -> String {
        let mut text = format!(
            "{} / {}\nEligibility: {}\nCounted trainer XP: {}\nXP is not money and grants no execution rights.\nSeason: {}\nReferee: {}\n",
            self.address, self.rule, self.eligibility, self.counted_xp, self.season, self.referee
        );
        text = format!(
            "{}\nTask: {}\n{}\nTrainer: {} XP, level {} ({})\n{text}",
            clean(&self.title, 100),
            clean(&self.task, 100),
            clean(&self.objective, 180),
            self.trainer_xp,
            self.trainer_level,
            xp_ledger::levels::CURVE
        );
        for line in self.decisions.iter().chain(self.requirements.iter()) {
            let bounded: String = line.chars().filter(|c| !c.is_control()).take(200).collect();
            text.push_str(&bounded);
            text.push('\n');
        }
        let path = match self.rule.as_str() {
            "reproduce" => "microcoder xp reproduce (docs/coder/guides/xp.md)",
            "kb-transfer" => {
                "openagents kb publish; historical evidence remains inconclusive (docs/coder/guides/xp.md)"
            }
            _ => {
                "openagents ext eval check / publish; operator adoption uses existing defaults authority (docs/extensions/evaluation.md)"
            }
        };
        text.push_str(&format!("Authorized path: {path}\n"));
        text.push_str("Exact evidence and award panes are opened alongside this quest. Reading or repeated activity earns no XP. Use existing authorized reproduce/check/publication paths; referee acceptance is required.\n");
        if text.len() > 2048 {
            while text.len() > 2000 {
                text.pop();
            }
            text.push_str("\nAdditional details truncated.");
        }
        text
    }
}

fn clean(text: &str, limit: usize) -> String {
    text.chars()
        .filter(|c| !c.is_control())
        .take(limit)
        .collect()
}

/// Credit is derived by the owner; current opportunity never changes historical awards.
pub fn project(
    events: &[Event],
    documents: &Documents,
    trust: &XpTrust,
    trainer: &str,
    now: u64,
) -> Vec<Quest> {
    let events: Vec<Event> = events
        .iter()
        .filter(|e| e.created_at <= now && e.validate_crypto().is_ok())
        .map(|e| (e.id.clone(), e.clone()))
        .collect::<BTreeMap<_, _>>()
        .into_values()
        .collect();
    let current: Vec<Event> = events
        .iter()
        .filter(|e| !xp::parse_award(e).is_ok_and(|a| a.accepted_at > now))
        .cloned()
        .collect();
    let ledger = xp_ledger::derive_with(&current, documents, trust);
    let trainers = xp_ledger::Trainers::read(&events);
    let trainer = trainers.trainer_of(trainer).unwrap_or(trainer);
    let keys: BTreeSet<String> = trainers.keys_of(trainer).into_iter().collect();
    let trainer_xp = ledger
        .credits
        .iter()
        .filter(|c| c.rule != "playtest" && keys.contains(&c.pubkey))
        .map(|c| c.xp)
        .sum();
    let mut seen = BTreeSet::new();
    events
        .iter()
        .filter_map(|event| {
            let quest = xp::parse_quest(event).ok()?;
            if !["reproduce", "eval-check", "eval-adopt", "kb-transfer"]
                .contains(&quest.acceptance.rule.as_str())
                || !seen.insert(event.id.clone())
            {
                return None;
            }
            let conflict = events.iter().any(|other| {
                other.id != event.id
                    && other.pubkey == event.pubkey
                    && xp::parse_quest(other).is_ok_and(|q| q.address == quest.address)
            });
            let related: Vec<_> = events
                .iter()
                .filter_map(|e| {
                    xp::parse_award(e)
                        .ok()
                        .filter(|a| a.quest.id == event.id && a.quest.pubkey == event.pubkey)
                        .map(|a| (e, a))
                })
                .collect();
            let credits: Vec<_> = ledger
                .credits
                .iter()
                .filter(|c| related.iter().any(|(e, _)| e.id == c.award))
                .collect();
            let counted_xp = credits
                .iter()
                .filter(|c| keys.contains(&c.pubkey))
                .map(|c| c.xp)
                .sum();
            let missing_pin = quest.acceptance.claim.as_ref().is_some_and(|p| {
                !events.iter().any(|e| {
                    e.id == p.id && e.pubkey == p.pubkey && e.kind == nostr::kb::EVIDENCE_KIND
                })
            }) || quest.acceptance.eval.as_ref().is_some_and(|pins| {
                !events.iter().any(|e| {
                    e.id == pins.subject.id
                        && e.pubkey == pins.subject.pubkey
                        && e.kind == pins.subject.kind
                }) || pins.suite.as_ref().is_some_and(|p| {
                    !events
                        .iter()
                        .any(|e| e.id == p.id && e.pubkey == p.pubkey && e.kind == p.kind)
                })
            });
            let eligibility = if !trust.referees.contains(&event.pubkey) {
                "Unavailable: referee is not trusted"
            } else if missing_pin {
                "Unavailable: exact pinned evidence is not retained"
            } else if conflict {
                "Unavailable: frozen version has conflicting signed events"
            } else if now < quest.season.opens_at {
                "Unavailable: season has not opened"
            } else if now > quest.season.closes_at {
                "Unavailable: season closed; historical awards remain"
            } else if credits.iter().any(|c| {
                keys.contains(&c.pubkey)
                    && (quest.acceptance.rule == "eval-check" && c.role == "checker"
                        || xp::keyed_role(&quest.acceptance.rule) == Some(c.role.as_str()))
            }) {
                "Unavailable: trainer completion already credited for this version"
            } else if ["reproduce", "kb-transfer"].contains(&quest.acceptance.rule.as_str())
                && !quest.per_awardee()
                && !credits.is_empty()
            {
                "Unavailable: first completion already accepted"
            } else if quest.award_limit().is_some_and(|n| {
                credits
                    .iter()
                    .map(|c| &c.award)
                    .collect::<BTreeSet<_>>()
                    .len() as u64
                    >= n
            }) {
                "Unavailable: award limit reached"
            } else {
                "Season open: exact evidence and owner authorization still required"
            }
            .into();
            let mut requirements = vec![
                format!(
                    "Minimum pass rate: {}; maximum USD/run: {:?}",
                    quest.acceptance.min_pass_rate, quest.acceptance.max_usd_per_run
                ),
                format!(
                    "Roles: {:?}; completion policy: {}",
                    quest.award, quest.completions
                ),
            ];
            let mut evidence = Vec::new();
            if let Some(recipe) = &quest.acceptance.recipe {
                requirements.push(format!("Pinned recipe SHA-256: {recipe}"));
            }
            if let Some(claim) = &quest.acceptance.claim {
                evidence.push(claim.id.clone());
            }
            if let Some(pins) = &quest.acceptance.eval {
                evidence.push(pins.subject.id.clone());
                if let Some(suite) = &pins.suite {
                    evidence.push(suite.id.clone());
                }
                if let Some(defaults) = &pins.defaults {
                    requirements.push(format!("Pinned defaults: {defaults}"));
                }
            }
            let mut decisions = Vec::new();
            let mut award_decisions = BTreeMap::new();
            for (award, parsed) in &related {
                let start = decisions.len();
                evidence.extend(parsed.evidence.iter().map(|p| p.id.clone()));
                if let Some(entry) = &parsed.entry {
                    evidence.push(entry.id.clone());
                }
                let counted: Vec<_> = credits.iter().filter(|c| c.award == award.id).collect();
                let status = if parsed.accepted_at > now {
                    "not credited: acceptance is future-dated"
                } else if ledger.revoked.contains(&award.id) {
                    "revoked"
                } else if counted.is_empty() {
                    "not credited: owner verification refused or unavailable"
                } else {
                    "credited by owner verification"
                };
                decisions.push(format!("Award {}: {status}", &award.id[..12]));
                for revocation in &events {
                    if xp::parse_revocation(revocation).is_ok_and(|r| {
                        r.award.id == award.id
                            && r.award.pubkey == award.pubkey
                            && r.key == parsed.key
                    }) {
                        evidence.push(revocation.id.clone());
                        decisions.push(format!("Signed revocation: {}", revocation.id));
                    }
                }
                for credit in &counted {
                    decisions.push(format!(
                        "{} {}: {} XP",
                        credit.role, credit.pubkey, credit.xp
                    ));
                }
                decisions.extend(
                    ledger
                        .refused
                        .iter()
                        .filter(|r| r.starts_with(&format!("award {}:", &award.id[..12])))
                        .cloned(),
                );
                if counted.is_empty() && !ledger.revoked.contains(&award.id) {
                    decisions.extend(
                        ledger
                            .conflicts
                            .iter()
                            .filter(|r| r.contains(&parsed.key) || r.contains(&quest.address))
                            .cloned(),
                    );
                }
                award_decisions.insert(award.id.clone(), decisions[start..].to_vec());
            }
            if related.is_empty() {
                decisions.push("No signed award: completion and XP unavailable".into());
            }
            evidence.sort();
            evidence.dedup();
            Some(Quest {
                title: quest.title,
                objective: quest.objective,
                task: quest.acceptance.task.clone(),
                trainer_xp,
                trainer_level: xp_ledger::levels::level_of(trainer_xp),
                award_decisions,
                event: event.id.clone(),
                referee: event.pubkey.clone(),
                address: quest.address,
                rule: quest.acceptance.rule,
                season: format!(
                    "{} [{}..{}]",
                    quest.season.id, quest.season.opens_at, quest.season.closes_at
                ),
                eligibility,
                requirements,
                counted_xp,
                awards: related.iter().map(|(e, _)| e.id.clone()).collect(),
                evidence,
                decisions,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use xp_ledger::eval::fixture::{Run, pubkey, published, release, signer};
    const AT: u64 = 1_790_000_000;
    pub(super) fn fixture() -> (Vec<Event>, String) {
        let referee = signer("quest-referee");
        let suite = release(&signer("quest-suite"), "suite", AT - 100);
        let subject = release(&signer("quest-author"), "tool", AT - 100);
        let parts=xp::quest(&json!({"id":"workbench.check","version":1,"season":{"id":"s1","opens_at":AT-200,"closes_at":AT+100},"title":"Check an exact release","objective":"Independently check the pinned release.","acceptance":{"rule":"eval-check","suite":{"id":suite.id,"pubkey":suite.pubkey,"kind":suite.kind},"subject":{"id":subject.id,"pubkey":subject.pubkey,"kind":subject.kind},"max_awards":20},"reference":null,"award":{"checker":50,"evaluator":25,"suite-author":25}})).unwrap();
        let quest = referee.sign(AT - 150, parts.kind, parts.tags, parts.content);
        let result = published(
            &signer("quest-evaluator"),
            &Run::better(&suite, &subject),
            None,
            AT,
        );
        let check = published(
            &signer("quest-checker"),
            &Run::better(&suite, &subject),
            Some(&result.id),
            AT + 10,
        );
        let awards = xp::eval_check_awards(&quest, &result, &check, &[], AT + 11)
            .unwrap()
            .into_iter()
            .map(|p| referee.sign(AT + 11, p.kind, p.tags, p.content));
        let mut events = vec![suite, subject, quest, result, check];
        events.extend(awards);
        (events, pubkey("quest-referee"))
    }
    #[test]
    fn exact_signed_completion_recomputes_without_activity_credit() {
        let (events, referee) = fixture();
        let trust = XpTrust {
            referees: BTreeSet::from([referee]),
            runners: BTreeSet::new(),
        };
        let documents = Documents::default();
        let trainer = pubkey("quest-checker");
        let row = project(&events, &documents, &trust, &trainer, AT + 20).remove(0);
        assert_eq!(row.counted_xp, 50);
        assert_eq!(row.awards.len(), 3);
        assert!(row.evidence.len() >= 4);
        let mut duplicates = events.clone();
        duplicates.extend(events.clone());
        assert_eq!(
            project(&duplicates, &documents, &trust, &trainer, AT + 20),
            vec![row.clone()]
        );
        for _ in 0..8 {
            assert_eq!(
                project(&events, &documents, &trust, &trainer, AT + 20)[0].counted_xp,
                50
            );
        }
        let closed = project(&events, &documents, &trust, &trainer, AT + 200).remove(0);
        assert_eq!(closed.counted_xp, 50);
        assert!(closed.eligibility.contains("closed"));
        assert_eq!(
            project(&events, &documents, &trust, &trainer, AT + 5)[0].counted_xp,
            0
        );
        assert!(row.lines().len() <= 2048);
    }
    #[test]
    fn revoked_untrusted_and_wrong_version_are_not_credit() {
        let (mut events, referee) = fixture();
        let trust = XpTrust {
            referees: BTreeSet::from([referee]),
            runners: BTreeSet::new(),
        };
        let award = events
            .iter()
            .find(|e| {
                xp::parse_award(e).is_ok_and(|a| {
                    a.awardees
                        .iter()
                        .any(|r| r.pubkey == pubkey("quest-checker"))
                })
            })
            .unwrap();
        let parts = xp::revocation(award, "Fixture revocation").unwrap();
        events.push(signer("quest-referee").sign(AT + 12, parts.kind, parts.tags, parts.content));
        let row = project(
            &events,
            &Documents::default(),
            &trust,
            &pubkey("quest-checker"),
            AT + 20,
        )
        .remove(0);
        assert_eq!(row.counted_xp, 0);
        assert!(row.decisions.iter().any(|d| d.contains("revoked")));
        let row = project(
            &events,
            &Documents::default(),
            &XpTrust::default(),
            &pubkey("quest-evaluator"),
            AT + 20,
        )
        .remove(0);
        assert_eq!(row.counted_xp, 0);
        assert!(row.eligibility.contains("not trusted"));
    }
    #[test]
    fn expired_acceptance_wrong_version_and_inconclusive_evidence_refuse_credit() {
        let (events, referee) = fixture();
        let trust = XpTrust {
            referees: BTreeSet::from([referee]),
            runners: BTreeSet::new(),
        };
        let trainer = pubkey("quest-checker");
        let mut expired = events.clone();
        for event in &mut expired {
            if xp::parse_award(event).is_ok() {
                let mut body: serde_json::Value = serde_json::from_str(&event.content).unwrap();
                body["accepted_at"] = json!(AT + 101);
                *event = signer("quest-referee").sign(
                    AT + 101,
                    event.kind,
                    event.tags.clone(),
                    body.to_string(),
                );
            }
        }
        assert_eq!(
            project(&expired, &Documents::default(), &trust, &trainer, AT + 200)[0].counted_xp,
            0
        );
        let mut future = events.clone();
        for event in &mut future {
            if xp::parse_award(event).is_ok() {
                let mut body: serde_json::Value = serde_json::from_str(&event.content).unwrap();
                body["accepted_at"] = json!(AT + 50);
                *event = signer("quest-referee").sign(
                    AT + 11,
                    event.kind,
                    event.tags.clone(),
                    body.to_string(),
                );
            }
        }
        let row = project(&future, &Documents::default(), &trust, &trainer, AT + 20).remove(0);
        assert_eq!(row.counted_xp, 0);
        assert!(row.decisions.iter().any(|d| d.contains("future-dated")));
        let mut wrong = events.clone();
        let quest = wrong
            .iter_mut()
            .find(|e| xp::parse_quest(e).is_ok())
            .unwrap();
        let mut spec: serde_json::Value = serde_json::from_str(&quest.content).unwrap();
        spec["version"] = json!(2);
        for key in ["v", "requires", "type"] {
            spec.as_object_mut().unwrap().remove(key);
        }
        let parts = xp::quest(&spec).unwrap();
        *quest = signer("quest-referee").sign(AT - 150, parts.kind, parts.tags, parts.content);
        assert_eq!(
            project(&wrong, &Documents::default(), &trust, &trainer, AT + 20)[0].counted_xp,
            0
        );
        let suite = &events[0];
        let subject = &events[1];
        let result = &events[3];
        let mut run = Run::better(suite, subject);
        run.verdict = "inconclusive";
        let inconclusive = published(&signer("quest-checker"), &run, Some(&result.id), AT + 10);
        assert!(xp::eval_check_awards(&events[2], result, &inconclusive, &[], AT + 11).is_err());
        let mut incomplete = events.clone();
        incomplete.retain(|e| e.id != events[4].id);
        assert_eq!(
            project(
                &incomplete,
                &Documents::default(),
                &trust,
                &trainer,
                AT + 20
            )[0]
            .counted_xp,
            0
        );
        let mut reversed = events.clone();
        reversed.reverse();
        assert_eq!(
            project(&events, &Documents::default(), &trust, &trainer, AT + 20),
            project(&reversed, &Documents::default(), &trust, &trainer, AT + 20)
        );
    }
    #[test]
    fn linked_self_and_distinct_duplicate_awards_show_owner_refusal() {
        let (mut events, referee) = fixture();
        let trust = XpTrust {
            referees: BTreeSet::from([referee]),
            runners: BTreeSet::new(),
        };
        let evaluator = signer("quest-evaluator");
        let checker = signer("quest-checker");
        for (by, parts) in [
            (
                &evaluator,
                xp::profile(evaluator.pubkey(), true, &[checker.pubkey().to_owned()]).unwrap(),
            ),
            (
                &checker,
                xp::link(checker.pubkey(), Some(evaluator.pubkey())).unwrap(),
            ),
        ] {
            events.push(by.sign(AT, parts.kind, parts.tags, parts.content));
        }
        let rows = project(
            &events,
            &Documents::default(),
            &trust,
            checker.pubkey(),
            AT + 20,
        );
        assert_eq!(rows[0].counted_xp, 0);
        assert_eq!(rows[0].trainer_xp, 0);
        assert!(rows[0].decisions.iter().any(|d| d.contains("linked")));
        let (mut events, _) = fixture();
        let award = events
            .iter()
            .find(|e| {
                xp::parse_award(e)
                    .is_ok_and(|a| a.awardees.iter().any(|r| r.pubkey == checker.pubkey()))
            })
            .unwrap();
        events.push(signer("quest-referee").sign(
            AT + 12,
            award.kind,
            award.tags.clone(),
            award.content.clone(),
        ));
        let row = project(
            &events,
            &Documents::default(),
            &trust,
            checker.pubkey(),
            AT + 20,
        )
        .remove(0);
        assert_eq!(row.counted_xp, 0);
        assert!(row.decisions.iter().any(|d| d.contains("uniqueness")));
    }
}
