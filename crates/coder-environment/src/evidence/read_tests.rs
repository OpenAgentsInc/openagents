use super::*;
use crate::RunLink;
use crate::evidence::{CallIdentity, Recorder, Redactor};
use serde_json::json;

fn run(job: &str) -> RunLink {
    RunLink {
        cloud_job: job.into(),
        task: None,
    }
}
fn call(id: &str) -> CallIdentity {
    CallIdentity {
        id: id.into(),
        parent: None,
        run: run("job-1"),
        tool: "shell".into(),
        request: None,
        operation: None,
    }
}
fn recorder(root: &Path, budget: u64) -> Recorder {
    Recorder::create(
        root.join("ev"),
        "ev-1",
        Some(run("job-1")),
        Redactor::new(),
        budget,
    )
    .unwrap()
}
fn exit_ok() -> boat::CommandFrame {
    boat::CommandFrame::Exit {
        exit_code: Some(0),
        success: true,
        timed_out: false,
    }
}
fn seqs(page: &EventPage) -> Vec<u64> {
    page.entries.iter().map(|e| e.seq).collect()
}
/// Every path and file content under `dir`, for side-effect checks.
fn snapshot(dir: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut out = Vec::new();
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            out.push((path.clone(), Vec::new()));
            out.extend(snapshot(&path));
        } else {
            out.push((path.clone(), fs::read(&path).unwrap()));
        }
    }
    out.sort();
    out
}
/// Ten calls started and finished in the same millisecond.
fn same_millisecond(root: &Path) -> Recorder {
    let mut r = recorder(root, 1 << 20);
    for i in 0..10 {
        let id = format!("c{i}");
        r.start_call(call(&id), &json!({"i": i}), 42).unwrap();
        r.output(&id, StreamName::Stdout, format!("out {i}\n").as_bytes())
            .unwrap();
        r.observe_boat(&id, &exit_ok(), 42).unwrap();
    }
    r
}

#[test]
fn records_sharing_a_timestamp_are_never_skipped_across_pages() {
    let tmp = tempfile::tempdir().unwrap();
    let mut r = same_millisecond(tmp.path());
    r.finish(42).unwrap();
    let reader = EvidenceReader::open(r.dir(), "ev-1").unwrap();
    let head = reader.head();
    assert!(head > 40);
    for limit in [1, 2, 3, 7] {
        let mut seen = Vec::new();
        let mut page = reader.events(EventWindow::Oldest, limit).unwrap();
        loop {
            seen.extend(seqs(&page));
            if !page.more_newer {
                break;
            }
            page = reader
                .events(EventWindow::After(page.newer.clone()), limit)
                .unwrap();
        }
        assert_eq!(seen, (1..=head).collect::<Vec<_>>(), "limit {limit}");
    }
    let page = reader.events(EventWindow::Latest, 5).unwrap();
    assert!(page.complete);
    assert_eq!(page.status, EvidenceStatus::Complete);
    assert!(page.gaps.is_empty());
}

#[test]
fn a_history_longer_than_any_window_keeps_the_newest_work_visible() {
    let tmp = tempfile::tempdir().unwrap();
    let mut r = recorder(tmp.path(), 64 << 20);
    r.start_call(call("long"), &json!({}), 1).unwrap();
    for i in 0..(MAX_PAGE_EVENTS + 200) {
        r.output("long", StreamName::Stdout, format!("line {i}\n").as_bytes())
            .unwrap();
    }
    r.observe_boat("long", &exit_ok(), 2).unwrap();
    r.finish(3).unwrap();
    let reader = EvidenceReader::open(r.dir(), "ev-1").unwrap();
    let head = reader.head();
    assert!(head as usize > MAX_PAGE_EVENTS);

    // Page bounds clamp, and the latest window ends at the head.
    let latest = reader.events(EventWindow::Latest, usize::MAX).unwrap();
    assert_eq!(latest.entries.len(), MAX_PAGE_EVENTS);
    assert_eq!(*seqs(&latest).last().unwrap(), head);
    assert!(matches!(
        latest.entries.last().unwrap().event,
        Event::Finalized { .. }
    ));
    assert!(!latest.more_newer);

    // History walks back to the first event without duplicates.
    let mut seen = seqs(&latest);
    let mut older = latest.older.clone();
    while let Some(cursor) = older {
        let page = reader.events(EventWindow::Before(cursor), 333).unwrap();
        let mut s = seqs(&page);
        s.extend(seen);
        seen = s;
        older = page.older;
    }
    assert_eq!(seen, (1..=head).collect::<Vec<_>>());

    // The latest bytes of a long stream are the newest ones.
    let tail = reader
        .stream("long", StreamName::Stdout, StreamWindow::Latest, 10)
        .unwrap();
    assert_eq!(
        tail.bytes,
        format!("line {}\n", MAX_PAGE_EVENTS + 199).as_bytes()
    );
    assert_eq!(tail.start + 10, tail.length);
    assert!(!tail.more_newer);
    let back = reader
        .stream(
            "long",
            StreamName::Stdout,
            StreamWindow::Before(tail.older.unwrap()),
            9,
        )
        .unwrap();
    assert_eq!(back.start + 9, tail.start);
}

#[test]
fn a_dropped_connection_resumes_without_duplicates_or_foreign_cursors() {
    let tmp = tempfile::tempdir().unwrap();
    let mut r = recorder(tmp.path(), 1 << 20);
    r.start_call(call("c1"), &json!({"cmd": "make"}), 5)
        .unwrap();
    r.output("c1", StreamName::Stdout, b"first ").unwrap();

    // A client reads while recording, then its connection drops.
    let reader = EvidenceReader::open(r.dir(), "ev-1").unwrap();
    let page = reader.events(EventWindow::Oldest, 100).unwrap();
    assert_eq!(page.status, EvidenceStatus::Recording);
    assert!(!page.complete);
    assert!(page.gaps.contains(&Gap::Recording));
    let resume = page.newer.clone();
    let bytes = reader
        .stream("c1", StreamName::Stdout, StreamWindow::Oldest, 100)
        .unwrap();
    assert_eq!(bytes.bytes, b"first ");
    assert!(bytes.more_newer, "an open stream may still grow");
    let byte_resume = bytes.newer.clone();

    r.output("c1", StreamName::Stdout, b"second").unwrap();
    r.observe_boat("c1", &exit_ok(), 6).unwrap();
    r.finish(7).unwrap();

    let reader = EvidenceReader::open(r.dir(), "ev-1").unwrap();
    let after = reader
        .events(EventWindow::After(resume.clone()), 100)
        .unwrap();
    assert_eq!(after.entries.first().unwrap().seq, resume.seq + 1);
    let mut all = seqs(&page);
    all.extend(seqs(&after));
    assert_eq!(all, (1..=reader.head()).collect::<Vec<_>>());
    // Replaying the same cursor is idempotent.
    assert_eq!(
        reader
            .events(EventWindow::After(resume.clone()), 100)
            .unwrap(),
        after
    );
    assert!(after.complete);
    // Resuming at the head yields nothing new.
    let done = reader
        .events(EventWindow::After(after.newer.clone()), 100)
        .unwrap();
    assert!(done.entries.is_empty());
    assert_eq!(done.newer, after.newer);

    let rest = reader
        .stream(
            "c1",
            StreamName::Stdout,
            StreamWindow::After(byte_resume),
            100,
        )
        .unwrap();
    assert_eq!(rest.bytes, b"second");
    assert_eq!(rest.state, Some(StreamState::Complete));
    assert!(!rest.more_newer);

    // Cursors are bound to the record, not to a position alone.
    let mut forged = resume.clone();
    forged.record = Some(crate::digest(b"other"));
    assert!(matches!(
        reader.events(EventWindow::After(forged), 10),
        Err(EvidenceError::Cursor(_))
    ));
    let mut foreign = resume.clone();
    foreign.evidence_id = "ev-2".into();
    assert!(matches!(
        reader.events(EventWindow::After(foreign), 10),
        Err(EvidenceError::Cursor(_))
    ));
    let mut future = after.newer.clone();
    future.seq += 1;
    assert!(reader.events(EventWindow::After(future), 10).is_err());
    // A byte cursor inside a chunk is valid; one naming another chunk is not.
    let mut mid_chunk = rest.newer.clone();
    mid_chunk.offset -= 1;
    let tail = reader
        .stream("c1", StreamName::Stdout, StreamWindow::After(mid_chunk), 10)
        .unwrap();
    assert_eq!(tail.bytes, b"d");
    let mut forged = rest.newer.clone();
    forged.chunk_digest = Some(crate::digest(b"other"));
    assert!(matches!(
        reader.stream("c1", StreamName::Stdout, StreamWindow::After(forged), 10),
        Err(EvidenceError::Cursor(_))
    ));
}

#[test]
fn a_corrupted_spool_is_disclosed_and_its_bytes_are_never_served() {
    let tmp = tempfile::tempdir().unwrap();
    let mut r = recorder(tmp.path(), 1 << 20);
    r.start_call(call("c1"), &json!({}), 1).unwrap();
    for chunk in [b"aaaa", b"bbbb", b"cccc"] {
        r.output("c1", StreamName::Stdout, chunk).unwrap();
    }
    r.observe_boat("c1", &exit_ok(), 2).unwrap();
    r.finish(3).unwrap();
    let reader = EvidenceReader::open(r.dir(), "ev-1").unwrap();
    assert!(reader.verify().complete);

    let path = r.dir().join("streams/c1.stdout");
    let mut bytes = fs::read(&path).unwrap();
    bytes[5] = b'X';
    fs::write(&path, &bytes).unwrap();

    let reader = EvidenceReader::open(r.dir(), "ev-1").unwrap();
    let first = reader
        .stream("c1", StreamName::Stdout, StreamWindow::Oldest, 100)
        .unwrap();
    assert_eq!(first.bytes, b"aaaa");
    assert!(!first.complete);
    assert_eq!(first.status, EvidenceStatus::Incomplete);
    let corrupt = Gap::Corrupt {
        seq: 3,
        call: "c1".into(),
        stream: StreamName::Stdout,
        start: 4,
        end: 8,
    };
    assert!(first.gaps.contains(&corrupt));
    // Paging continues past the corrupt range, disclosing it again.
    let last = reader
        .stream(
            "c1",
            StreamName::Stdout,
            StreamWindow::After(first.newer.clone()),
            100,
        )
        .unwrap();
    assert_eq!((last.start, last.bytes.as_slice()), (8, &b"cccc"[..]));
    assert!(last.gaps.contains(&corrupt));
    // Backward reads also skip the corrupt range and keep moving.
    let latest = reader
        .stream("c1", StreamName::Stdout, StreamWindow::Latest, 100)
        .unwrap();
    assert_eq!((latest.start, latest.bytes.as_slice()), (8, &b"cccc"[..]));
    let back = reader
        .stream(
            "c1",
            StreamName::Stdout,
            StreamWindow::Before(latest.older.clone().unwrap()),
            4,
        )
        .unwrap();
    assert!(back.bytes.is_empty());
    assert_eq!(back.start, 4);
    assert!(back.gaps.contains(&corrupt));
    let front = reader
        .stream(
            "c1",
            StreamName::Stdout,
            StreamWindow::Before(back.older.clone().unwrap()),
            100,
        )
        .unwrap();
    assert_eq!((front.start, front.bytes.as_slice()), (0, &b"aaaa"[..]));
    for page in [&first, &last, &latest, &back, &front] {
        assert!(!page.bytes.contains(&b'X'));
    }

    // The structural summary trusts the log; verification reads the bytes.
    assert!(reader.summary().complete);
    let verified = reader.verify();
    assert!(!verified.complete);
    assert_eq!(verified.status, EvidenceStatus::Incomplete);
    assert!(verified.gaps.contains(&corrupt));
}

#[test]
fn export_marks_gaps_and_never_claims_complete_when_it_is_not() {
    let tmp = tempfile::tempdir().unwrap();
    // Budget exhaustion, an unfinished call, and an unarchived child.
    let mut r = recorder(tmp.path(), 64);
    r.start_call(call("c1"), &json!({}), 1).unwrap();
    r.output("c1", StreamName::Stdout, &[b'z'; 200]).unwrap();
    r.observe_boat("c1", &exit_ok(), 2).unwrap();
    r.start_call(call("c2"), &json!({}), 3).unwrap();
    let _child = r.child("c2", "kid", run("job-2"), 8).unwrap();
    let sealed = r.finish(4).unwrap();
    assert_eq!(sealed.status, EvidenceStatus::Incomplete);

    let reader = EvidenceReader::open(r.dir(), "ev-1").unwrap();
    let out = tmp.path().join("bundle");
    let exported = reader.export(&out).unwrap();
    assert!(!exported.complete);
    assert_eq!(exported.status, EvidenceStatus::Incomplete);
    let kinds: Vec<_> = exported
        .gaps
        .iter()
        .map(|g| serde_json::to_value(g).unwrap()["kind"].clone())
        .collect();
    for kind in [
        "budget_exhausted",
        "dropped",
        "stream_gap",
        "unresolved",
        "child_unarchived",
    ] {
        assert!(kinds.contains(&json!(kind)), "{kind} in {kinds:?}");
    }
    let on_disk: ExportManifest =
        serde_json::from_slice(&fs::read(out.join("export.json")).unwrap()).unwrap();
    assert_eq!(on_disk, exported);
    for file in &exported.files {
        let bytes = fs::read(out.join(&file.path)).unwrap();
        assert_eq!(bytes.len() as u64, file.length, "{}", file.path);
        assert_eq!(crate::digest(&bytes), file.digest, "{}", file.path);
    }
    // The export is itself a readable evidence record with the same gaps.
    let again = EvidenceReader::open(&out, "ev-1").unwrap().summary();
    assert_eq!(again.sealed, Some(sealed));
    assert!(!again.complete);
    assert!(reader.export(&out).is_err(), "exports never overwrite");
}

#[test]
fn a_complete_record_with_a_child_exports_complete_and_corruption_flips_it() {
    let tmp = tempfile::tempdir().unwrap();
    let mut r = recorder(tmp.path(), 1 << 20);
    r.start_call(call("verify"), &json!({}), 1).unwrap();
    let mut child = r.child("verify", "kid", run("job-2"), 4096).unwrap();
    child.start_call(call("k1"), &json!({}), 1).unwrap();
    child.output("k1", StreamName::Stdout, b"ok\n").unwrap();
    child.observe_boat("k1", &exit_ok(), 2).unwrap();
    r.archive_child(child, 3).unwrap();
    r.observe_boat("verify", &exit_ok(), 4).unwrap();
    r.finish(5).unwrap();

    let reader = EvidenceReader::open(r.dir(), "ev-1").unwrap();
    assert!(reader.verify().complete);
    let good = reader.export(&tmp.path().join("good")).unwrap();
    assert!(good.complete, "{:?}", good.gaps);
    assert!(
        good.files
            .iter()
            .any(|f| f.path == "children/kid/export.json")
    );
    let kid = reader.child("kid").unwrap();
    let page = kid
        .stream("k1", StreamName::Stdout, StreamWindow::Oldest, 10)
        .unwrap();
    assert_eq!(page.bytes, b"ok\n");

    fs::write(r.dir().join("children/kid/streams/k1.stdout"), b"no\n").unwrap();
    let reader = EvidenceReader::open(r.dir(), "ev-1").unwrap();
    let bad = reader.export(&tmp.path().join("bad")).unwrap();
    assert!(!bad.complete);
    assert!(bad.gaps.iter().any(|g| matches!(
        g,
        Gap::ChildIncomplete { evidence_id, .. } if evidence_id == "kid"
    )));
    assert!(!reader.verify().complete);

    // A missing child is a gap, not an error.
    fs::remove_dir_all(r.dir().join("children/kid")).unwrap();
    let reader = EvidenceReader::open(r.dir(), "ev-1").unwrap();
    assert!(reader.summary().gaps.iter().any(|g| matches!(
        g,
        Gap::ChildMissing { evidence_id, .. } if evidence_id == "kid"
    )));
}

#[test]
fn reads_have_no_side_effects_and_a_damaged_log_is_a_gap() {
    let tmp = tempfile::tempdir().unwrap();
    assert!(EvidenceReader::open(tmp.path().join("absent"), "ev-1").is_err());
    assert!(!tmp.path().join("absent").exists());

    let r = same_millisecond(tmp.path());
    // Still recording: no manifest yet, and no `children/` directory.
    let before = snapshot(r.dir());
    let reader = EvidenceReader::open(r.dir(), "ev-1").unwrap();
    reader.events(EventWindow::Latest, 3).unwrap();
    reader
        .stream("c3", StreamName::Stdout, StreamWindow::Latest, 3)
        .unwrap();
    reader.summary();
    reader.verify();
    assert_eq!(snapshot(r.dir()), before);

    // A torn tail is read up to the damage and disclosed.
    let log = r.dir().join("events.jsonl");
    let mut bytes = fs::read(&log).unwrap();
    let head = bytes.iter().filter(|&&b| b == b'\n').count() as u64;
    bytes.extend_from_slice(b"{\"seq\":");
    fs::write(&log, &bytes).unwrap();
    let reader = EvidenceReader::open(r.dir(), "ev-1").unwrap();
    assert_eq!(reader.head(), head);
    let page = reader.events(EventWindow::Latest, 2).unwrap();
    assert!(page.gaps.iter().any(|g| matches!(
        g,
        Gap::Log { after_seq, .. } if *after_seq == head
    )));
    assert!(!page.complete);
}
