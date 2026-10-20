use super::super::super::claims;
use super::super::super::meetings::*;
use super::*;
fn fixture() -> (Fixture, Access) {
    let mut f = Fixture::new();
    let credential = f.dir.path().join("human");
    f.store
        .issue(&f.owner, "alex", Role::Reader, &credential)
        .unwrap();
    let human = f
        .store
        .authenticate(&Store::read_credential(&credential).unwrap())
        .unwrap();
    let mut details = f.store.state.leads[&f.lead].details.clone();
    details.data.recipients.push("human:alex".into());
    details.permission.reference = "customer agreed requested demo and named alex recipient".into();
    let command = Command {
        schema: COMMAND_SCHEMA.into(),
        id: "meeting-boundary".into(),
        lead: Some(f.lead.clone()),
        expected_revision: f.store.state.leads[&f.lead].revision,
        operation: Operation::Update { details },
    };
    f.store
        .apply(&f.owner, &serde_json::to_vec(&command).unwrap())
        .unwrap();
    let lead = &f.store.state.leads[&f.lead];
    let command = super::super::super::privacy::Command {
        schema: super::super::super::privacy::COMMAND_SCHEMA.into(),
        id: "meeting-fresh-business-admission".into(),
        expected_revision: f.store.state.privacy.revision,
        operation: super::super::super::privacy::Operation::Admit {
            admission: super::super::super::privacy::Admission {
                lead: f.lead.clone(),
                expected_lead_revision: lead.revision,
                customer: lead.details.account.clone(),
                jurisdiction: "US".into(),
                source_kind: super::super::super::privacy::SourceKind::GivenBusinessRole,
                permission_kind: super::super::super::privacy::PermissionKind::AcceptedIntroduction,
                source_sha256: digest(lead.source.as_bytes()),
                permission_reference_sha256: digest(lead.details.permission.reference.as_bytes()),
                owner_reference: "operator checked requested demo and named human consent".into(),
                scope_sha256: None,
                aliases: vec![lead.contact.clone()],
            },
        },
    };
    f.store
        .apply_sales_privacy(&f.owner, &serde_json::to_vec(&command).unwrap())
        .unwrap();
    f.policy.version = 2;
    f.policy.data_recipients.push("human:alex".into());
    f.owner_apply(
        "meeting-policy",
        OwnerOperation::PublishPolicy {
            policy: f.policy.clone(),
        },
        None,
    )
    .unwrap();
    f.credential = f.dir.path().join("meeting-agent");
    f.owner_apply(
        "meeting-assignment",
        OwnerOperation::Assign {
            lead: f.lead.clone(),
            expected_lead_revision: f.store.state.leads[&f.lead].revision,
            agent: f.anchor.clone(),
            policy_sha256: f.policy.sha256().unwrap(),
            expires_at: now() + 900,
        },
        Some(f.credential.clone()),
    )
    .unwrap();
    fn retained(root: &Path, name: &str, bytes: &[u8]) -> gym::sales_evidence::Reference {
        std::fs::write(root.join(name), bytes).unwrap();
        gym::sales_evidence::Reference {
            path: name.into(),
            sha256: digest(bytes),
        }
    }
    let source = claims::SourceInput {
        schema: claims::SOURCE_SCHEMA.into(),
        pin: claims::Pin {
            id: "meeting-capability".into(),
            revision: 1,
        },
        scope: claims::Scope {
            product: "Coder".into(),
            offer_version: super::super::super::intake::OFFER.into(),
            release: "a".repeat(40),
        },
        root: f.dir.path().into(),
        evidence: claims::Evidence::Capability {
            contract: retained(
                f.dir.path(),
                "contract",
                b"synthetic maintained scoped workflow",
            ),
            reviewed_fact: "Checked patch for this scoped workflow".into(),
        },
        readiness: claims::Readiness::Implemented,
        limits: vec!["Declared local workflow only".into()],
        review: retained(
            f.dir.path(),
            "source-review",
            b"synthetic exact owner review",
        ),
        expires_at: now() + 300,
        activation: None,
    };
    f.store
        .review_claim_source(&f.owner, source.clone())
        .unwrap();
    let claim = claims::ClaimInput {
        pin: claims::Pin {
            id: "meeting-demo".into(),
            revision: 1,
        },
        source: source.pin,
        purpose: claims::Purpose::Capability,
        playbook: retained(f.dir.path(), "playbook", b"bounded reviewed playbook"),
        expires_at: now() + 300,
        review: retained(
            f.dir.path(),
            "claim-review",
            b"synthetic exact claim review",
        ),
    };
    f.store.review_claim(&f.owner, claim.clone()).unwrap();
    let view = f
        .store
        .current_claims(&f.owner, &[claim.pin], &"a".repeat(40))
        .unwrap()
        .remove(0);
    f.store
        .compose_claim_draft(
            &f.owner,
            "meeting-claims",
            vec![claims::DraftPin {
                claim: view.pin,
                reviewed_sha256: view.reviewed_sha256.unwrap(),
            }],
            &"a".repeat(40),
        )
        .unwrap();
    f.store
        .publish_meeting_slot(
            &f.owner,
            &Slot {
                id: "slot".into(),
                version: 1,
                human: "alex".into(),
                start_at: now() + 50,
                end_at: now() + 80,
                expires_at: now() + 40,
                availability_reference: "explicit synthetic owner availability".into(),
            },
            0,
        )
        .unwrap();
    (f, human)
}
fn input(f: &Fixture, name: &str) -> ProposalInput {
    ProposalInput {
        id: name.into(),
        expected_revision: 0,
        lead: f.lead.clone(),
        expected_lead_revision: f.store.state.leads[&f.lead].revision,
        slot: "slot".into(),
        slot_version: 1,
        target: "alex".into(),
        brief: Some(BriefInput {
            decision_maker: Knowledge {
                known: None,
                unknown_reason: Some("Buyer authority remains unknown".into()),
            },
            current_tools: Knowledge {
                known: Some("Synthetic editor".into()),
                unknown_reason: None,
            },
            baseline_unknowns: vec!["Baseline duration remains unmeasured".into()],
            claim_draft: "meeting-claims".into(),
            release: "a".repeat(40),
            proposed_demo: "Scoped local workflow demonstration".into(),
            proposed_pilot: "Human-led pilot subject to separate agreement".into(),
            acceptance_criteria: vec!["Customer reviews declared checks".into()],
            next_action: NextAction {
                description: "Human reviews the requested demo".into(),
                due_at: now() + 200,
            },
            review_at: now() + 200,
            pilot_kit_sha256: digest(PILOT_KIT_SHA_SOURCE.as_bytes()),
        }),
    }
}
#[test]
fn meeting_acceptance_is_bounded_and_restart_preserves_original_assignment() {
    let (mut f, human) = fixture();
    let m = f
        .store
        .propose_sales_meeting(&f.owner, &input(&f, "one"))
        .unwrap();
    assert!(
        f.store
            .sales_meeting(&human, "one")
            .unwrap()
            .meeting
            .brief
            .is_none()
    );
    let access = f.access();
    let slots = f.store.sales_meeting_slots(&access).unwrap();
    assert_eq!(slots[0].availability_reference.len(), 64);
    let rec = f
        .store
        .recommend_sales_meeting_slot(&access, "one", m.revision, "slot", 1)
        .unwrap();
    assert!(rec.owner_confirmation_needed);
    assert!(
        !serde_json::to_string(&rec)
            .unwrap()
            .contains("private-buyer")
    );
    assert!(
        f.store
            .confirm_sales_meeting(
                &f.owner,
                "one",
                m.revision,
                &m.proposal_sha256,
                "synthetic request"
            )
            .is_err()
    );
    let m = f
        .store
        .confirm_sales_meeting(
            &f.owner,
            "one",
            rec.revision,
            &rec.proposal_sha256,
            "synthetic requested demo",
        )
        .unwrap();
    assert!(
        f.store
            .sales_meeting(&human, "one")
            .unwrap()
            .meeting
            .brief
            .is_some()
    );
    assert!(
        f.store
            .decide_sales_meeting(
                &f.owner,
                "one",
                m.revision,
                &m.proposal_sha256,
                true,
                "accept"
            )
            .is_err()
    );
    f.store
        .decide_sales_meeting(
            &human,
            "one",
            m.revision,
            &m.proposal_sha256,
            true,
            "synthetic human acceptance",
        )
        .unwrap();
    f.reopen(now);
    let view = f.store.sales_meeting(&human, "one").unwrap();
    assert_eq!(view.meeting.accepted_by.as_deref(), Some("alex"));
    assert_eq!(
        view.meeting.brief.unwrap().details.next_action.description,
        "Human reviews the requested demo"
    );
    assert!(
        !view.calendar_authority
            && !view.mailbox_authority
            && !view.payment_authority
            && !view.outbound_authority
            && !view.earned_revenue
    );
    assert!(
        f.store
            .readable(&human, &f.store.state.leads[&f.lead])
            .is_err()
    );
    assert_eq!(f.store.state.leads[&f.lead].responsible_human, "operator");
}
#[test]
fn meeting_missing_brief_rejection_expiry_and_double_booking_remain_pending() {
    let (mut f, human) = fixture();
    let mut missing = input(&f, "missing");
    missing.brief = None;
    let m = f.store.propose_sales_meeting(&f.owner, &missing).unwrap();
    assert!(
        f.store
            .confirm_sales_meeting(
                &f.owner,
                "missing",
                m.revision,
                &m.proposal_sha256,
                "request"
            )
            .unwrap_err()
            .contains("brief")
    );
    let a = f
        .store
        .propose_sales_meeting(&f.owner, &input(&f, "first"))
        .unwrap();
    let b = f
        .store
        .propose_sales_meeting(&f.owner, &input(&f, "second"))
        .unwrap();
    f.store
        .confirm_sales_meeting(&f.owner, "first", a.revision, &a.proposal_sha256, "request")
        .unwrap();
    assert!(
        f.store
            .confirm_sales_meeting(
                &f.owner,
                "second",
                b.revision,
                &b.proposal_sha256,
                "request"
            )
            .unwrap_err()
            .contains("already confirmed")
    );
    f.store
        .decide_sales_meeting(
            &human,
            "first",
            a.revision,
            &a.proposal_sha256,
            false,
            "declined",
        )
        .unwrap();
    assert_eq!(
        f.store
            .sales_meeting(&human, "first")
            .unwrap()
            .meeting
            .phase,
        Phase::Declined
    );
    f.store
        .confirm_sales_meeting(
            &f.owner,
            "second",
            b.revision,
            &b.proposal_sha256,
            "request",
        )
        .unwrap();
    fn stale() -> u64 {
        now() + 41
    }
    f.reopen(stale);
    assert!(
        f.store
            .decide_sales_meeting(
                &human,
                "second",
                b.revision,
                &b.proposal_sha256,
                true,
                "accept"
            )
            .unwrap_err()
            .contains("stale")
    );
    assert!(
        f.store
            .sales_meeting(&human, "second")
            .unwrap()
            .meeting
            .brief
            .is_none()
    );
}
#[test]
fn meeting_revocation_clears_customer_brief_and_never_grants_other_humans_access() {
    let (mut f, human) = fixture();
    let m = f
        .store
        .propose_sales_meeting(&f.owner, &input(&f, "one"))
        .unwrap();
    f.store
        .confirm_sales_meeting(
            &f.owner,
            "one",
            m.revision,
            &m.proposal_sha256,
            "private synthetic request text",
        )
        .unwrap();
    let credential = f.dir.path().join("other");
    f.store
        .issue(&f.owner, "other", Role::Reader, &credential)
        .unwrap();
    let other = f
        .store
        .authenticate(&Store::read_credential(&credential).unwrap())
        .unwrap();
    assert!(f.store.sales_meeting(&other, "one").is_err());
    let command = Command {
        schema: COMMAND_SCHEMA.into(),
        id: "remove-meeting-contact".into(),
        lead: Some(f.lead.clone()),
        expected_revision: f.store.state.leads[&f.lead].revision,
        operation: Operation::Delete {
            reference: "customer deletion".into(),
        },
    };
    f.store
        .apply(&f.owner, &serde_json::to_vec(&command).unwrap())
        .unwrap();
    let view = f.store.sales_meeting(&human, "one").unwrap();
    assert_eq!(view.meeting.phase, Phase::Retired);
    assert!(view.meeting.brief.is_none());
    let json = serde_json::to_string(&view).unwrap();
    assert!(!json.contains("private-buyer") && !json.contains("private synthetic request text"));
}
#[test]
fn meeting_changed_lead_and_proposal_require_fresh_confirmation() {
    let (mut f, human) = fixture();
    let m = f
        .store
        .propose_sales_meeting(&f.owner, &input(&f, "one"))
        .unwrap();
    f.store
        .confirm_sales_meeting(&f.owner, "one", m.revision, &m.proposal_sha256, "request")
        .unwrap();
    let mut details = f.store.state.leads[&f.lead].details.clone();
    details.workflow = "changed declared workflow".into();
    let c = Command {
        schema: COMMAND_SCHEMA.into(),
        id: "changed-workflow".into(),
        lead: Some(f.lead.clone()),
        expected_revision: f.store.state.leads[&f.lead].revision,
        operation: Operation::Update { details },
    };
    f.store
        .apply(&f.owner, &serde_json::to_vec(&c).unwrap())
        .unwrap();
    assert!(
        f.store
            .decide_sales_meeting(
                &human,
                "one",
                m.revision,
                &m.proposal_sha256,
                true,
                "accept"
            )
            .unwrap_err()
            .contains("revision changed")
    );
    let view = f.store.sales_meeting(&human, "one").unwrap();
    assert!(view.meeting.brief.is_none());
    assert!(!view.blockers.is_empty());
    let mut next = input(&f, "one");
    next.expected_revision = m.revision;
    next.expected_lead_revision = f.store.state.leads[&f.lead].revision;
    let newer = f.store.propose_sales_meeting(&f.owner, &next).unwrap();
    assert_eq!(newer.phase, Phase::Pending);
    assert!(newer.owner_confirmation.is_none());
    assert!(
        f.store
            .confirm_sales_meeting(&f.owner, "one", m.revision, &m.proposal_sha256, "request")
            .is_err()
    );
    f.store
        .confirm_sales_meeting(
            &f.owner,
            "one",
            newer.revision,
            &newer.proposal_sha256,
            "new requested scope",
        )
        .unwrap();
    f.store
        .decide_sales_meeting(
            &human,
            "one",
            newer.revision,
            &newer.proposal_sha256,
            true,
            "accept new scope",
        )
        .unwrap();
}
#[test]
fn meeting_original_retention_survives_extension_and_missing_permission_refuses() {
    let (mut f, human) = fixture();
    let m = f
        .store
        .propose_sales_meeting(&f.owner, &input(&f, "one"))
        .unwrap();
    let mut details = f.store.state.leads[&f.lead].details.clone();
    details.data.retain_until = now() + 3000;
    details.permission.reference = "customer provided fresh extended retention evidence".into();
    let c = Command {
        schema: COMMAND_SCHEMA.into(),
        id: "extend-retention".into(),
        lead: Some(f.lead.clone()),
        expected_revision: f.store.state.leads[&f.lead].revision,
        operation: Operation::Update { details },
    };
    f.store
        .apply(&f.owner, &serde_json::to_vec(&c).unwrap())
        .unwrap();
    fn later() -> u64 {
        now() + 2001
    }
    f.reopen(later);
    let view = f.store.sales_meeting(&human, "one").unwrap();
    assert_eq!(view.meeting.phase, Phase::Retired);
    assert!(view.meeting.brief.is_none());
    assert_eq!(view.meeting.retain_until, m.retain_until);
    assert!(f.store.state.leads.contains_key(&f.lead));
    assert!(
        f.store
            .propose_sales_meeting(&f.owner, &input(&f, "expired"))
            .is_err()
    );
}
#[test]
fn meeting_claim_withdrawal_and_current_native_authority_are_rechecked() {
    let (mut f, human) = fixture();
    let access = f.access();
    let m = f
        .store
        .propose_sales_meeting(&f.owner, &input(&f, "one"))
        .unwrap();
    f.store
        .confirm_sales_meeting(&f.owner, "one", m.revision, &m.proposal_sha256, "request")
        .unwrap();
    f.store
        .withdraw_claim_revision(
            &f.owner,
            &claims::Pin {
                id: "meeting-capability".into(),
                revision: 1,
            },
            true,
            "synthetic capability withdrawn",
        )
        .unwrap();
    assert!(
        f.store
            .decide_sales_meeting(
                &human,
                "one",
                m.revision,
                &m.proposal_sha256,
                true,
                "accept"
            )
            .is_err()
    );
    assert!(
        f.store
            .sales_meeting(&human, "one")
            .unwrap()
            .meeting
            .brief
            .is_none()
    );
    assert!(f.store.sales_meetings(&human, None, 0).is_err());
    f.store.revoke(&f.owner, "alex").unwrap();
    assert!(f.store.sales_meeting(&human, "one").is_err());
    f.owner_apply(
        "stop-meeting-agent",
        OwnerOperation::RevokePolicy {
            policy_sha256: f.policy.sha256().unwrap(),
            reference: artifact("meeting-policy-revoked"),
        },
        None,
    )
    .unwrap();
    assert!(f.store.sales_agent_meetings(&access).is_err());
}
#[test]
fn meeting_missing_permission_or_named_recipient_cannot_be_confirmed() {
    for permission_missing in [true, false] {
        let (mut f, _) = fixture();
        let mut details = f.store.state.leads[&f.lead].details.clone();
        if permission_missing {
            details.permission.state = PermissionState::Unknown;
            details.stage = Stage::New;
        } else {
            details.data.recipients.retain(|x| x != "human:alex");
            details.permission.reference =
                "customer permitted narrowed operator-only boundary".into();
        }
        let c = Command {
            schema: COMMAND_SCHEMA.into(),
            id: "changed-boundary".into(),
            lead: Some(f.lead.clone()),
            expected_revision: f.store.state.leads[&f.lead].revision,
            operation: Operation::Update { details },
        };
        f.store
            .apply(&f.owner, &serde_json::to_vec(&c).unwrap())
            .unwrap();
        let mut p = input(&f, "missing");
        p.expected_lead_revision = f.store.state.leads[&f.lead].revision;
        assert!(
            f.store
                .propose_sales_meeting(&f.owner, &p)
                .unwrap_err()
                .contains("permission or named human")
        );
    }
}
#[test]
fn native_meeting_confirmation_child() {
    let Some(base) = std::env::var_os("OA_MEETING_CONCURRENT_ROOT") else {
        return;
    };
    let id = std::env::var("OA_MEETING_CONCURRENT_ID").unwrap();
    let sha = std::env::var("OA_MEETING_CONCURRENT_SHA").unwrap();
    let mut s = Store::open_with_clock(&PathBuf::from(&base).join("host"), now).unwrap();
    let a = s
        .authenticate(&Store::read_credential(&PathBuf::from(&base).join("owner")).unwrap())
        .unwrap();
    let result = s.confirm_sales_meeting(&a, &id, 1, &sha, "synthetic requested demo");
    std::fs::write(
        PathBuf::from(base).join(format!("{id}-result")),
        if result.is_ok() {
            "confirmed"
        } else {
            "refused"
        },
    )
    .unwrap();
}
#[test]
fn concurrent_native_proposals_confirm_only_one_human_slot() {
    let (mut f, _) = fixture();
    let a = f
        .store
        .propose_sales_meeting(&f.owner, &input(&f, "first"))
        .unwrap();
    let b = f
        .store
        .propose_sales_meeting(&f.owner, &input(&f, "second"))
        .unwrap();
    let placeholder = Store::open_with_clock(&f.dir.path().join("placeholder"), now).unwrap();
    drop(std::mem::replace(&mut f.store, placeholder));
    let spawn = |m: &Meeting| {
        std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "task::sales::agents::tests::meeting_tests::native_meeting_confirmation_child",
                "--nocapture",
            ])
            .env("HOME", f.dir.path())
            .env("OA_MEETING_CONCURRENT_ROOT", f.dir.path())
            .env("OA_MEETING_CONCURRENT_ID", &m.id)
            .env("OA_MEETING_CONCURRENT_SHA", &m.proposal_sha256)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap()
    };
    let mut first = spawn(&a);
    let mut second = spawn(&b);
    assert!(first.wait().unwrap().success());
    assert!(second.wait().unwrap().success());
    let results = ["first", "second"]
        .map(|id| std::fs::read_to_string(f.dir.path().join(format!("{id}-result"))).unwrap());
    assert_eq!(
        results.iter().filter(|r| r.as_str() == "confirmed").count(),
        1
    );
    assert_eq!(
        results.iter().filter(|r| r.as_str() == "refused").count(),
        1
    );
    f.store = Store::open_with_clock(&f.dir.path().join("host"), now).unwrap();
    let queue = f.store.sales_meetings(&f.owner, None, 100).unwrap();
    assert_eq!(
        queue
            .iter()
            .filter(|v| v.meeting.phase == Phase::OwnerConfirmed)
            .count(),
        1
    );
}

#[test]
fn meeting_unknown_reason_is_protected_customer_data() {
    let (mut f, _) = fixture();
    let mut p = input(&f, "protected-unknown");
    p.brief.as_mut().unwrap().decision_maker.unknown_reason =
        Some("Synthetic-private-decision-maker uncertainty".into());
    f.store.propose_sales_meeting(&f.owner, &p).unwrap();
    let native = agent::Store::with_keys(
        &f.dir.path().join("host"),
        "paul",
        std::sync::Arc::new(FileKeys),
    )
    .unwrap();
    assert!(
        super::super::super::privacy::check_agent_copy(&native, "Opaque public workflow question")
            .is_ok()
    );
    assert_eq!(
        super::super::super::privacy::check_agent_copy(
            &native,
            "Synthetic-private-decision-maker uncertainty"
        )
        .unwrap_err(),
        "identifiable customer material stays in the canonical private sales pipeline"
    );
}
