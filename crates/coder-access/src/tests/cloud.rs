//! Synthetic native rights, operator-policy, and retained recovery checks.
use super::*;

struct CloudRecorder {
    ordinary: Recorder,
    allowed: bool,
    effects: usize,
}
impl Dispatch for CloudRecorder {
    fn dispatch(
        &mut self,
        request: &str,
        device: &str,
        op: &Operation,
    ) -> std::result::Result<Receipt, Code> {
        self.ordinary.dispatch(request, device, op)
    }
    fn cloud_admit_recovery(
        &mut self,
        _: &str,
        _: &crate::cloud::Admission,
    ) -> std::result::Result<(), Code> {
        if self.allowed {
            Ok(())
        } else {
            Err(Code::Forbidden)
        }
    }
    fn cloud(
        &mut self,
        request: &str,
        _: &str,
        _: Option<(&str, u64)>,
        op: &Operation,
    ) -> std::result::Result<Outcome, Code> {
        if !self.allowed {
            return Err(Code::Forbidden);
        }
        match op {
            Operation::CloudSubmit { intent } => {
                self.effects += 1;
                Ok(Outcome::CloudAccepted {
                    accepted: crate::cloud::Accepted {
                        request: request.into(),
                        scope: crate::cloud::Scope {
                            workspace: intent.workspace.clone(),
                            project: intent.project.clone(),
                            job: request.into(),
                            revision: format!("sha256:{}", "c".repeat(64)),
                            attempt: 1,
                            profile: intent.profile.clone(),
                            profile_revision: intent.profile_revision.clone(),
                            source_digest: intent.source_digest.clone(),
                        },
                        action: "submit".into(),
                        state: "accepted".into(),
                    },
                })
            }
            _ => Err(Code::Unsupported),
        }
    }
}
fn enrollment(f: &Fixture, rights: &str, r: &mut CloudRecorder) -> (SecretKey, Client) {
    let device = key();
    let invitation = HostInvitation::parse(&f.invite(rights), now(), POLICY).unwrap();
    let pending = client::prepare_redeem(&invitation, &device, now(), POLICY).unwrap();
    let reply = f.host().handle(&pending.event, &f.relay, now(), r).unwrap();
    let access =
        client::finish_redeem(&invitation, &pending, &reply, &device, now(), POLICY).unwrap();
    (device, Client::device(access, device, POLICY).unwrap())
}
fn submit() -> Operation {
    Operation::CloudSubmit {
        intent: crate::cloud::Submit {
            workspace: "checkout".into(),
            project: "synthetic".into(),
            profile: "fixture".into(),
            profile_revision: format!("sha256:{}", "a".repeat(64)),
            source_digest: format!("sha256:{}", "b".repeat(64)),
            prompt: "Synthetic explicit operator intent.".into(),
            timeout_seconds: 60,
        },
    }
}
#[test]
fn native_grants_and_current_operator_policy_gate_effects_and_cached_recovery() {
    let f = Fixture::local();
    let mut recorder = CloudRecorder {
        ordinary: Recorder::default(),
        allowed: true,
        effects: 0,
    };
    let (_, observe) = enrollment(&f, "observe", &mut recorder);
    let pending = observe.prepare(submit(), now()).unwrap();
    let reply = f
        .host()
        .handle(&pending.event, &f.relay, now(), &mut recorder)
        .unwrap();
    assert_eq!(
        observe
            .verify_reply(&pending, &reply, now())
            .unwrap_err()
            .missing,
        Some(Right::Operate)
    );
    assert_eq!(recorder.effects, 0);
    let (device, operator) = enrollment(&f, "standard", &mut recorder);
    let original = operator.prepare(submit(), now()).unwrap();
    let reply = f
        .host()
        .handle(&original.event, &f.relay, now(), &mut recorder)
        .unwrap();
    let accepted = operator.verify_reply(&original, &reply, now()).unwrap();
    assert!(matches!(accepted, Outcome::CloudAccepted { .. }));
    assert_eq!(recorder.effects, 1);
    assert_eq!(
        f.host()
            .handle(&original.event, &f.relay, now(), &mut recorder)
            .unwrap(),
        reply
    );
    assert_eq!(recorder.effects, 1);
    recorder.allowed = false;
    let refused = f
        .host()
        .handle(&original.event, &f.relay, now(), &mut recorder)
        .unwrap();
    assert_eq!(
        operator
            .verify_reply(&original, &refused, now())
            .unwrap_err()
            .code,
        Code::Forbidden
    );
    let recovery = operator
        .prepare(
            Operation::RequestOperation {
                request: original.request.request.clone(),
                request_event: original.event.id.clone(),
            },
            now(),
        )
        .unwrap();
    let refused = f
        .host()
        .handle(&recovery.event, &f.relay, now(), &mut recorder)
        .unwrap();
    assert_eq!(
        operator
            .verify_reply(&recovery, &refused, now())
            .unwrap_err()
            .code,
        Code::Forbidden
    );
    assert_eq!(recorder.effects, 1);
    recorder.allowed = true;
    let recovered = f
        .host()
        .handle(&recovery.event, &f.relay, now(), &mut recorder)
        .unwrap();
    assert!(matches!(
        operator.verify_reply(&recovery, &recovered, now()).unwrap(),
        Outcome::RequestOperation {
            result: Some(_),
            ..
        }
    ));
    f.host().revoke(&pubkey(&device), now()).unwrap();
    let refused = f
        .host()
        .handle(&recovery.event, &f.relay, now(), &mut recorder)
        .unwrap();
    assert_eq!(
        operator
            .verify_reply(&recovery, &refused, now())
            .unwrap_err()
            .code,
        Code::Revoked
    );
    assert_eq!(recorder.effects, 1);
}
#[test]
fn cloud_aliases_bounds_and_read_semantics_are_closed() {
    for value in [".", "..", "...", "../private", "/private"] {
        assert!(crate::cloud::alias(value).is_err());
    }
    for op in [
        Operation::CloudProjects {
            workspace: "checkout".into(),
        },
        Operation::CloudCatalog {
            query: crate::cloud::CatalogQuery {
                workspace: "checkout".into(),
                project: "synthetic".into(),
            },
        },
    ] {
        assert!(op.reads_only());
        assert!(!op.retains_reply());
        assert_eq!(op.required(), Some(Right::Observe));
    }
    let op = submit();
    assert!(!op.reads_only());
    assert!(op.retains_reply());
    assert_eq!(op.required(), Some(Right::Operate));
    assert!(op.validate().is_ok());
}

#[test]
fn cloud_original_replies_bound_encoded_bytes_and_exact_requested_limits() {
    use base64::Engine;
    let scope = crate::cloud::Scope {
        workspace: "checkout".into(),
        project: "synthetic".into(),
        job: "first".into(),
        revision: format!("sha256:{}", "c".repeat(64)),
        attempt: 1,
        profile: "fixture".into(),
        profile_revision: format!("sha256:{}", "a".repeat(64)),
        source_digest: format!("sha256:{}", "b".repeat(64)),
    };
    let original = crate::cloud::Original {
        source: "result".into(),
        digest: format!("sha256:{}", "d".repeat(64)),
        bytes: 2,
        media_type: "application/json".into(),
    };
    let mut chunk = crate::cloud::OriginalChunk {
        scope: scope.clone(),
        original: original.clone(),
        start: 0,
        data: base64::engine::general_purpose::STANDARD.encode(b"{}"),
        next: None,
        more_available: false,
    };
    let query = crate::cloud::OriginalQuery {
        scope,
        original,
        cursor: None,
        limit: 1,
    };
    assert!(chunk.validate().is_ok());
    assert!(!chunk.answers(&query));
    chunk.data = "a".repeat(32 * 1024);
    assert_eq!(chunk.validate().unwrap_err().code, Code::Bounds);
}
