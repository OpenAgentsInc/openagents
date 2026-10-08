use super::*;
use crate::Right;
use crate::protocol::{Operation, Outcome};
use serde_json::json;

fn hash() -> String {
    format!("sha256:{}", "a".repeat(64))
}
fn scope() -> Scope {
    Scope {
        workspace: "fixture".into(),
        task: "studio-g1-child".into(),
        revision: 2,
        attempt: Some(1),
        intent_digest: hash(),
    }
}
fn original() -> Original {
    Original {
        source: "trace:1".into(),
        digest: hash(),
        bytes: 80_000,
        media_type: "application/x-ndjson".into(),
    }
}
fn cursor() -> Cursor {
    Cursor {
        scope: scope(),
        source: "trace:1".into(),
        source_digest: hash(),
        source_bytes: 80_000,
        next_step: 1,
        prefix_digest: hash(),
    }
}
pub(super) fn page() -> Page {
    Page {
        scope: scope(),
        title: "Original fixture".into(),
        prompt: "Original prompt\nwith markdown".into(),
        phase: Phase::Running,
        execution: "running".into(),
        verification: "not_run".into(),
        integration: "not_attempted".into(),
        termination: "unknown".into(),
        delivery: "unknown".into(),
        cleanup: "unknown".into(),
        cost_microusd: None,
        cost_status: "unknown".into(),
        evidence: Evidence {
            state: "unsealed".into(),
            original: Some(original()),
            total_steps: 2,
            steps: vec![Step::Original {
                index: 0,
                step: json!({"message":"**original markdown**","future_field":{"original":true}}),
            }],
            faults: vec![],
            more_faults: false,
        },
        artifacts: vec![],
        children: vec![],
        more_children: false,
        next: Some(cursor()),
        more_available: true,
    }
}

#[test]
fn query_and_reply_pins_reject_cross_scope_and_unknown_fields() {
    let mut query = PageQuery {
        workspace: "fixture".into(),
        task: "studio-g1-child".into(),
        revision: Some(2),
        cursor: Some(cursor()),
        limit: 4,
    };
    query.validate().unwrap();
    query.revision = None;
    assert!(query.validate().is_err());
    query.cursor = None;
    query.validate().unwrap();
    let page = page();
    page.validate().unwrap();
    let mut encoded = serde_json::to_value(&page).unwrap();
    assert_eq!(encoded["phase"], "running");
    assert_eq!(
        serde_json::from_value::<Page>(encoded.clone()).unwrap(),
        page
    );
    encoded["phase"] = json!("invented");
    assert!(serde_json::from_value::<Page>(encoded).is_err());
    assert!(page.answers(&query));
    query.workspace = "another".into();
    assert!(!page.answers(&query));
    let mut wire = serde_json::to_value(&query).unwrap();
    wire["arbitrary_path"] = json!("/private/file");
    assert!(serde_json::from_value::<PageQuery>(wire).is_err());
    let mut pin = original();
    for bad in [
        "../file",
        "/private/file",
        "https://example.com/file",
        "artifact:bad",
        "trace:0",
    ] {
        pin.source = bad.into();
        assert!(pin.validate().is_err());
    }
}

#[test]
fn an_oversized_original_step_requires_an_explicit_gap() {
    let mut page = page();
    page.evidence.steps = vec![Step::Original {
        index: 0,
        step: json!({"message":"x".repeat(MAX_INLINE_STEP_BYTES+1)}),
    }];
    assert_eq!(page.validate().unwrap_err().code, Code::Bounds);
    page.evidence.steps = vec![Step::Gap {
        index: 0,
        reason: GapReason::Oversized,
        original: original(),
    }];
    page.validate().unwrap();
    let Step::Gap { original, .. } = &mut page.evidence.steps[0] else {
        panic!("gap")
    };
    original.digest = format!("sha256:{}", "b".repeat(64));
    assert_eq!(page.validate().unwrap_err().code, Code::Malformed);
    assert_eq!(
        bounded(
            &json!({"message":"x".repeat(MAX_REPLY_BYTES)}),
            MAX_REPLY_BYTES
        )
        .unwrap_err()
        .code,
        Code::Bounds
    );
}

#[test]
fn original_chunks_bind_scope_source_digest_offset_and_encoded_bytes() {
    let original = Original {
        bytes: 3,
        ..original()
    };
    let mut query = OriginalQuery {
        scope: scope(),
        original: original.clone(),
        cursor: None,
        limit: 3,
    };
    let chunk = OriginalChunk {
        scope: scope(),
        original,
        start: 0,
        data: STANDARD.encode(b"raw"),
        next: None,
        more_available: false,
    };
    query.validate().unwrap();
    chunk.validate().unwrap();
    assert!(chunk.answers(&query));
    query.scope.attempt = Some(2);
    assert!(!chunk.answers(&query));
    let mut oversized = chunk;
    oversized.data = STANDARD.encode(vec![0; MAX_CHUNK_BYTES + 1]);
    assert_eq!(oversized.validate().unwrap_err().code, Code::Bounds);
}

#[test]
fn native_task_reads_require_only_observe_and_never_retain_replies() {
    let list = Operation::ListTasks {
        query: ListQuery {
            workspace: "fixture".into(),
            cursor: None,
            limit: 1,
        },
    };
    let read = Operation::ReadTask {
        query: PageQuery {
            workspace: "fixture".into(),
            task: "studio-g1-child".into(),
            revision: None,
            cursor: None,
            limit: 1,
        },
    };
    let original = Operation::ReadTaskOriginal {
        query: OriginalQuery {
            scope: scope(),
            original: original(),
            cursor: None,
            limit: 1,
        },
    };
    for operation in [list, read, original] {
        operation.validate().unwrap();
        assert_eq!(operation.required(), Some(Right::Observe));
        assert!(operation.reads_only() && operation.local_task());
        assert!(!operation.retains_reply());
    }
    let operation = Operation::ReadTask {
        query: PageQuery {
            workspace: "fixture".into(),
            task: "studio-g1-child".into(),
            revision: Some(2),
            cursor: None,
            limit: 1,
        },
    };
    let outcome = Outcome::Task {
        task: Box::new(page()),
    };
    outcome.validate().unwrap();
    assert!(outcome.answers(&operation));
}

#[test]
fn the_encoded_reply_bound_includes_its_typed_outcome() {
    let mut page = page();
    page.prompt.clear();
    let base = serde_json::to_vec(&page).unwrap().len();
    page.prompt = "x".repeat(MAX_REPLY_BYTES - base);
    page.validate().unwrap();
    assert_eq!(
        Outcome::Task {
            task: Box::new(page)
        }
        .validate()
        .unwrap_err()
        .code,
        Code::Bounds
    );
}
