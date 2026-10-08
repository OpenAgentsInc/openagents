use super::*;
use crate::task::sales::{expenses, paul as controller, qualification as q, training};
use gym::suite::Partition;
struct Decisions {
    source: expenses::Source,
    identity: gym::row::DoorIdentity,
    calls: usize,
}
impl q::DecisionModel for Decisions {
    fn source(&self) -> &expenses::Source {
        &self.source
    }
    fn identity(&self) -> &gym::row::DoorIdentity {
        &self.identity
    }
    fn judge(
        &mut self,
        state: &serde_json::Value,
        question: &serde_json::Value,
        _: &expenses::Source,
    ) -> Result<q::DecisionReply> {
        self.calls += 1;
        let text = state["draft"]["text"].as_str().unwrap_or("");
        let synthetic = text.starts_with("Declared synthetic");
        let selected = if synthetic {
            text.split_whitespace()
                .last()
                .unwrap()
                .parse::<usize>()
                .unwrap()
        } else if question["type"] == "score" {
            2
        } else {
            0
        };
        let answer = if question["type"] == "score" {
            let mut probs = serde_json::Map::new();
            for i in 0..3 {
                probs.insert(
                    i.to_string(),
                    serde_json::json!(if i == selected { 0.7 } else { 0.15 }),
                );
            }
            serde_json::json!({"type":"score","probabilities":probs,"score":(0..3).map(|i|i as f64*if i==selected {0.7} else {0.15}).sum::<f64>(),"confidence":0.7,"legend":{}})
        } else {
            serde_json::json!({"type":"noul","noul":if selected==1 {0.7} else {0.3}})
        };
        Ok(q::DecisionReply::Answer {
            response: serde_json::json!({"model":self.identity.model,"answers":{"check":answer}}),
        })
    }
}
fn consume_measure(
    f: &mut Fixture,
    id: &str,
    partition: Partition,
    model: &mut Decisions,
) -> Result<q::Package> {
    let temporary =
        Store::open_with_clock(&f.dir.path().join("unused-qualification"), now).unwrap();
    let store = std::mem::replace(&mut f.store, temporary);
    let result = store.measure_sales_qualification(&f.owner, id, partition, model);
    f.store = Store::open_with_clock(&f.dir.path().join("host"), now).unwrap();
    result
}
#[test]
fn original_measurements_freeze_and_locked_partition_cannot_be_reused() {
    let (mut f, claims) = super::helpers::prepared_with(Fixture::with_execution_budget(100000));
    let mut sales_policy = f.policy.clone();
    sales_policy.version = 2;
    sales_policy.read_fields.insert(ReadField::Permission);
    f.owner_apply(
        "explicit-grade-disclosure",
        OwnerOperation::PublishPolicy {
            policy: sales_policy.clone(),
        },
        None,
    )
    .unwrap();
    let credential = f.dir.path().join("measured-agent-credential");
    f.owner_apply(
        "explicit-grade-assignment",
        OwnerOperation::Assign {
            lead: f.lead.clone(),
            expected_lead_revision: 2,
            agent: f.anchor.clone(),
            policy_sha256: sales_policy.sha256().unwrap(),
            expires_at: now() + 800,
        },
        Some(credential.clone()),
    )
    .unwrap();
    f.credential = credential;
    f.policy = sales_policy;
    let access = f.access();
    assert!(
        f.store
            .apply_sales_agent(&access, &f.draft("new-hire-real-draft", 3))
            .unwrap_err()
            .contains("training")
    );
    let source = expenses::Source {
        basis: expenses::Basis::ListPrice,
        kind: expenses::Kind::Jev,
        source_revision: digest(b"synthetic-decision-source"),
        price_revision: digest(b"fixture-price"),
        recipient: "human:operator".into(),
        max_input_bytes: 65536,
        max_output_tokens: 4096,
        max_attempts: 1,
        max_elapsed_secs: 30,
        input_usd_millionths_per_million: 1,
        output_usd_millionths_per_million: 1,
    };
    let practice_source = expenses::Source {
        basis: expenses::Basis::LocalDeterministic,
        kind: expenses::Kind::Training,
        source_revision: digest(b"conservative-practice-script-v1"),
        price_revision: digest(b"local-original-zero-cost"),
        recipient: "human:operator".into(),
        max_input_bytes: 65536,
        max_output_tokens: 1024,
        max_attempts: 1,
        max_elapsed_secs: 30,
        input_usd_millionths_per_million: 0,
        output_usd_millionths_per_million: 0,
    };
    let policy = expenses::Policy {
        schema: "openagents.sales-model-policy.v1".into(),
        revision: 2,
        floor_daily_usd_millionths: 5000000,
        agent_daily_usd_millionths: 1000000,
        request_usd_millionths: 100000,
        sources: vec![
            source.clone(),
            practice_source.clone(),
            claims::helpers::source(claims::helpers::Query::CitedAnswer, "human:operator"),
        ],
    };
    f.store
        .publish_sales_model_policy(&f.owner, &policy, &policy.sha256().unwrap())
        .unwrap();
    let mut suites = vec![];
    for dimension in [
        q::Dimension::Claims,
        q::Dimension::Compliance,
        q::Dimension::Tone,
    ] {
        let mut suite = q::tests::marked(dimension);
        let original = suite.clone();
        suite.suite.items.clear();
        suite.marks.clear();
        let copies = if dimension == q::Dimension::Tone {
            10
        } else {
            15
        };
        for (item, mark) in original.suite.items.iter().zip(&original.marks) {
            for copy in 0..copies {
                let mut item = item.clone();
                let mut mark = mark.clone();
                item.id = format!("{}-{copy}", item.id);
                item.state["fixture_replica"] = serde_json::json!(copy);
                mark.item = item.id.clone();
                mark.group = item.id.clone();
                mark.mark.reference = format!("mark-{}", item.id);
                mark.mark.sha256 = digest(item.id.as_bytes());
                suite.suite.items.push(item);
                suite.marks.push(mark);
            }
        }
        suite.suite.digest = suite.suite.compute_digest().unwrap();
        suites.push(suite);
    }
    let identity =
        gym::row::DoorIdentity::published("fixture-model", "fixture-content", "fixture-adapter");
    let candidate = q::Candidate {
        id: "original-measurement".into(),
        agent: f.anchor.clone(),
        playbook: f.policy.playbook.clone(),
        release: claims.release.clone(),
        claims: claims.claims.clone(),
        suites,
        source: source.clone(),
        identity: identity.clone(),
        expires_at: now() + 600,
    };
    f.store
        .publish_sales_qualification(&f.owner, &candidate)
        .unwrap();
    let mut model = Decisions {
        source,
        identity,
        calls: 0,
    };
    consume_measure(&mut f, &candidate.id, Partition::Calibration, &mut model).unwrap();
    consume_measure(&mut f, &candidate.id, Partition::Development, &mut model).unwrap();
    let frozen = f
        .store
        .freeze_sales_qualification(&f.owner, &candidate.id)
        .unwrap();
    assert_eq!(frozen.phase, q::Phase::Frozen);
    let reopened: q::Package =
        serde_json::from_slice(&serde_json::to_vec(&frozen).unwrap()).unwrap();
    assert_eq!(
        serde_json::to_value(&frozen.frozen).unwrap(),
        serde_json::to_value(&reopened.frozen).unwrap()
    );
    assert_eq!(frozen.freeze_sha256, reopened.freeze_sha256);
    let passed = consume_measure(&mut f, &candidate.id, Partition::Locked, &mut model).unwrap();
    assert_eq!(passed.phase, q::Phase::LockedPassed);
    assert_eq!(model.calls, 360);
    assert!(consume_measure(&mut f, &candidate.id, Partition::Locked, &mut model).is_err());
    assert_eq!(model.calls, 360);
    assert!(!passed.locked_reads.is_empty());
    assert!(
        f.store
            .state
            .qualification
            .rows
            .iter()
            .all(|r| r["receipt"].as_str().is_some())
    );
    assert!(f.store.state.agents.certificates.is_empty());
    let mut marks = vec![];
    for cycle in 0..2 {
        for persona in training::personas() {
            let schedule = training::Schedule {
                id: format!(
                    "practice-{cycle}-{}",
                    serde_json::to_value(persona.situation)
                        .unwrap()
                        .as_str()
                        .unwrap()
                ),
                agent: f.anchor.clone(),
                playbook: f.policy.playbook.clone(),
                situation: persona.situation,
                release: claims.release.clone(),
                claims: claims.claims.clone(),
                source: practice_source.clone(),
                bounds: training::Bounds {
                    max_turns: 6,
                    max_output_bytes: 1024,
                    max_total_output_bytes: 4096,
                    max_elapsed_secs: 60,
                },
            };
            f.store
                .schedule_sales_roleplay(&f.owner, &schedule)
                .unwrap();
            let temporary =
                Store::open_with_clock(&f.dir.path().join("unused-practice"), now).unwrap();
            let store = std::mem::replace(&mut f.store, temporary);
            let mut practice = ConservativePractice {
                source: practice_source.clone(),
            };
            let run = store
                .run_sales_roleplay(&f.owner, &schedule.id, &mut practice)
                .unwrap();
            f.store = Store::open_with_clock(&f.dir.path().join("host"), now).unwrap();
            assert!(matches!(
                run.stop,
                training::Stop::Completed | training::Stop::OptOut
            ));
            for (index, _) in run
                .turns
                .iter()
                .enumerate()
                .filter(|(_, t)| t.speaker == training::Speaker::Student)
            {
                let subject = q::GradeSubject::Practice {
                    run: run.schedule.id.clone(),
                    student_turn: index,
                };
                let temporary =
                    Store::open_with_clock(&f.dir.path().join("unused-grade"), now).unwrap();
                let store = std::mem::replace(&mut f.store, temporary);
                let grade = store
                    .grade_sales_subject(&f.owner, &candidate.id, &subject, None, &mut model)
                    .unwrap();
                f.store = Store::open_with_clock(&f.dir.path().join("host"), now).unwrap();
                assert!(grade.passed(), "{:?}", grade);
                assert_eq!(grade.judgments.len(), 4);
                let mark = format!("accepted-{}-{index}", schedule.id);
                f.store
                    .accept_sales_grade(
                        &f.owner,
                        &mark,
                        &grade.reference,
                        &artifact("exact-owner-review"),
                    )
                    .unwrap();
                marks.push(mark);
            }
        }
    }
    assert_eq!(marks.len(), 22);
    assert_eq!(model.calls, 448);
    assert!(
        f.store
            .certify_sales_agent(
                &f.owner,
                "measured",
                &candidate.id,
                &marks[..19],
                &artifact("owner-certificate")
            )
            .is_err()
    );
    let cert = f
        .store
        .certify_sales_agent(
            &f.owner,
            "measured",
            &candidate.id,
            &marks,
            &artifact("owner-certificate"),
        )
        .unwrap();
    assert_eq!(cert.certification.state, CertState::Qualified);
    assert!(cert.measured_qualified);
    assert!(!cert.outbound_authority);
    let access = f.access();
    let snapshot = f.store.check_sales_agent_qualification(&access).unwrap();
    assert_eq!(snapshot.certification_reference, "measured:1");
    // Retained verdicts cannot be altered independently of their original reply.
    let reference = f.store.state.qualification.draft_marks[&marks[0]]
        .grade_reference
        .clone();
    let original = f.store.state.qualification.grades[&reference].clone();
    f.store
        .state
        .qualification
        .grades
        .get_mut(&reference)
        .unwrap()
        .judgments[0]
        .selected = Some("yes".into());
    assert!(f.store.check_sales_agent_qualification(&access).is_err());
    f.store
        .state
        .qualification
        .grades
        .insert(reference, original);
    let temporary =
        Store::open_with_clock(&f.dir.path().join("unused-content-helper"), now).unwrap();
    let store = std::mem::replace(&mut f.store, temporary);
    let helper = store
        .run_sales_claim_helper(&f.owner, &access, &claims, "original-real-content")
        .unwrap();
    f.store = Store::open_with_clock(&f.dir.path().join("host"), now).unwrap();
    let binding = controller::Binding {
        schema: controller::SCHEMA.into(),
        revision: 1,
        anchor: f.anchor.clone(),
        owner_credential: f.dir.path().join("owner"),
        assignments: vec![f.credential.clone()],
        permitted_requesters: vec!["owner".into()],
    };
    f.store
        .configure_paul(&f.owner, &binding, &binding.sha256().unwrap())
        .unwrap();
    let mut real_drafts = vec![];
    let mut original_draft_refs = vec![];
    for index in 0..5 {
        let revision = f.store.read_sales_agent(&access).unwrap().revision;
        let bytes = f.command(
            &format!("measured-real-draft-{index}"),
            revision,
            AgentOperation::ProposeDraft {
                body: helper.answer.draft_body.clone().unwrap(),
                template: artifact("reviewed-template"),
                check_refs: vec![helper.artifact.clone()],
                recommendation: Some(helper.artifact.clone()),
            },
        );
        if index == 0 {
            let request = controller::DraftRequest {
                lead: f.lead.clone(),
                expected_lead_revision: revision,
                helper_reference: helper.artifact.reference.clone(),
            };
            let first = f
                .store
                .ask_paul_draft("owner", "measured-paul-draft", &request)
                .unwrap();
            let repeated = f
                .store
                .ask_paul_draft("owner", "measured-paul-draft", &request)
                .unwrap();
            assert_eq!(first.command_digest, repeated.command_digest);
            assert_eq!(first.sequence, repeated.sequence);
        } else {
            f.store.apply_sales_agent(&access, &bytes).unwrap();
            f.store.apply_sales_agent(&access, &bytes).unwrap();
        }
        let draft = f
            .store
            .read_sales_agent(&access)
            .unwrap()
            .drafts
            .iter()
            .find(|d| !original_draft_refs.contains(&d.reference))
            .unwrap()
            .clone();
        original_draft_refs.push(draft.reference.clone());
        real_drafts.push(draft);
    }
    let revision = f.store.read_sales_agent(&access).unwrap().revision;
    let extra = f.command(
        "over-review-cap",
        revision,
        AgentOperation::ProposeDraft {
            body: helper.answer.draft_body.clone().unwrap(),
            template: artifact("reviewed-template"),
            check_refs: vec![helper.artifact.clone()],
            recommendation: Some(helper.artifact.clone()),
        },
    );
    assert!(
        f.store
            .apply_sales_agent(&access, &extra)
            .unwrap_err()
            .contains("cap reached")
    );
    assert_eq!(
        f.store.state.agents.draft_days.values().next().unwrap()[&f.anchor.pubkey],
        5
    );
    f.reopen(now);
    assert_eq!(
        f.store.state.agents.draft_days.values().next().unwrap()[&f.anchor.pubkey],
        5
    );
    for (index, draft) in real_drafts.into_iter().take(3).enumerate() {
        let subject = q::GradeSubject::Draft {
            lead: f.lead.clone(),
            draft_reference: draft.reference.clone(),
        };
        let temporary =
            Store::open_with_clock(&f.dir.path().join("unused-real-grade"), now).unwrap();
        let store = std::mem::replace(&mut f.store, temporary);
        let grade = if index == 0 {
            store
                .grade_sales_subject(&f.owner, &candidate.id, &subject, Some(&access), &mut model)
                .unwrap()
        } else {
            let mut refusing = RefusingDecision {
                source: model.source.clone(),
                identity: model.identity.clone(),
            };
            store
                .grade_sales_subject(
                    &f.owner,
                    &candidate.id,
                    &subject,
                    Some(&access),
                    &mut refusing,
                )
                .unwrap()
        };
        f.store = Store::open_with_clock(&f.dir.path().join("host"), now).unwrap();
        assert_eq!(grade.passed(), index == 0);
        if index == 0 {
            let revision = f.store.state.leads[&f.lead].revision;
            f.owner_apply(
                "review-real-measured-draft",
                OwnerOperation::ReviewDraft {
                    lead: f.lead.clone(),
                    expected_lead_revision: revision,
                    draft: draft.reference.clone(),
                    state: DraftState::OwnerReviewed,
                    reference: artifact("owner-review-real"),
                },
                None,
            )
            .unwrap();
            let snapshot = f
                .store
                .qualified_sales_draft(
                    &f.owner,
                    &f.lead,
                    &f.anchor,
                    &access.assignment,
                    &f.policy.sha256().unwrap(),
                    "measured:1",
                    &draft.reference,
                )
                .unwrap();
            assert_eq!(
                snapshot.original_expense_reference,
                helper.expense_reference
            );
            assert_eq!(snapshot.grade_sha256, grade.sha256().unwrap());
        } else if index == 1 {
            assert!(f.store.check_sales_agent_qualification(&access).is_ok());
        }
    }
    assert!(f.store.check_sales_agent_qualification(&access).is_err());
    assert_eq!(
        f.store.state.agents.certificates["measured:1"]
            .certification
            .state,
        CertState::Suspended
    );
    assert!(
        f.store
            .certify_sales_agent(
                &f.owner,
                "reset-must-fail",
                &candidate.id,
                &marks,
                &artifact("new-id-old-samples")
            )
            .is_err()
    );
    let complaint = f
        .store
        .record_sales_complaint(
            &f.owner,
            "original-attributed-complaint",
            &helper.expense_reference,
            &artifact("owner-complaint-evidence"),
        )
        .unwrap();
    assert_eq!(complaint.kind, "attributed_complaint");
    assert_eq!(
        f.store
            .record_sales_complaint(
                &f.owner,
                "original-attributed-complaint",
                &helper.expense_reference,
                &artifact("owner-complaint-evidence")
            )
            .unwrap()
            .reference,
        complaint.reference
    );
    // A new package name cannot reveal the same already-spent locked examples.
    let mut replay = candidate.clone();
    replay.id = "renamed-locked-exposure".into();
    assert!(
        f.store
            .publish_sales_qualification(&f.owner, &replay)
            .is_err()
    );
}

struct ConservativePractice {
    source: expenses::Source,
}
impl training::Model for ConservativePractice {
    fn source(&self) -> &expenses::Source {
        &self.source
    }
    fn turn(&mut self, input: &training::TurnInput, caps: &expenses::Source) -> Result<String> {
        assert_eq!(caps, &self.source);
        if input.role == training::Speaker::Buyer {
            assert!(input.buyer_situation.is_some());
            return Ok(input.public_profile.clone());
        }
        assert!(input.buyer_situation.is_none());
        let buyer = &input
            .history
            .iter()
            .rev()
            .find(|t| t.speaker == training::Speaker::Buyer)
            .unwrap()
            .text;
        // The student reads the buyer's stated concern, never the hidden persona field.
        Ok(if buyer.contains("opt") || buyer.contains("stop") { "I will stop. No further contact is permitted." }
        else if buyer.contains("jurisdiction") { "Jurisdiction is unknown. No contact or commercial activation is permitted." }
        else if buyer.contains("ambiguous") || buyer.contains("consent") { "Consent is ambiguous. No new contact or recipient is authorized." }
        else if buyer.contains("price") || buyer.contains("cost") || buyer.contains("subscription") { "The full cost is unavailable. I will not describe subscriptions or resource use as free." }
        else if buyer.contains("pressure") || buyer.contains("payment") { "No payment or customer success is established. I will not invent revenue." }
        else { "I cannot promise an unsupported feature. Review the documented limits." }.into())
    }
}

struct RefusingDecision {
    source: expenses::Source,
    identity: gym::row::DoorIdentity,
}
impl q::DecisionModel for RefusingDecision {
    fn source(&self) -> &expenses::Source {
        &self.source
    }
    fn identity(&self) -> &gym::row::DoorIdentity {
        &self.identity
    }
    fn judge(
        &mut self,
        _: &serde_json::Value,
        _: &serde_json::Value,
        _: &expenses::Source,
    ) -> Result<q::DecisionReply> {
        Ok(q::DecisionReply::Refusal {
            code: gym::row::RefusalCode::Other("declared-synthetic-refusal".into()),
        })
    }
}
