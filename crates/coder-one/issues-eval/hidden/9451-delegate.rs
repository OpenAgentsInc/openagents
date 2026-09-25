    // Added by the issue-flow evaluation: the test the fix for #9451 added,
    // renamed so it can't collide with a candidate's own tests. It names
    // the fix's `answer()` and `transcript()`, which the issue asks for as
    // `answer` and `transcript`.
    #[tokio::test]
    async fn issue_eval_a_narrating_delegate_is_graded_on_its_answer() {
        if !boundary_supported() {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let binary = stub(
            dir.path(),
            "devin",
            "printf 'Reading the file.Counting: the answer is not 4.Committed.Final answer: done'",
        );
        let delegator = Delegator::new(executor(&binary)).in_directory(dir.path());
        let graded = delegator
            .run(Task::reading("do it", "crates/atif/src/document.rs").expecting("done"))
            .await;
        assert_eq!(graded.status, Status::Answered);
        assert_eq!(graded.answer(), "done");
        assert_eq!(graded.recorded_output(), "done");
        assert_eq!(
            graded.transcript(),
            Some("Reading the file.Counting: the answer is not 4.Committed.")
        );
        assert_eq!(graded.correct(), Some(true));
        assert_eq!(graded.verdict(), Verdict::Passed);
    }
