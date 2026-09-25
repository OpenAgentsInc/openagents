    // Added by the issue-flow evaluation: the test the fix for #9452 added,
    // renamed so it can't collide with a candidate's own tests, and with the
    // fix's own constant and commit message left out: it checks what the
    // issue's acceptance says, that a delegate's commit in
    // `<worktree>/.coder-git` holds only the files the item changed.
    #[tokio::test]
    async fn issue_eval_a_checkout_is_seeded_with_a_scratch_git_directory() {
        let directory = repository().await;
        std::fs::write(directory.path().join("a.txt"), "a\n").unwrap();
        std::fs::write(directory.path().join("b.txt"), "b\n").unwrap();
        for args in [
            vec!["add", "a.txt", "b.txt"],
            vec![
                "-c",
                "user.name=Test",
                "-c",
                "user.email=test@example.invalid",
                "commit",
                "--quiet",
                "-m",
                "two files",
            ],
        ] {
            let result = git(directory.path()).args(args).run().await;
            assert!(result.ending.success(), "{}", result.stderr.text);
        }
        let checkout = Worktree::add(directory.path()).await.unwrap();
        let scratch = |args: &[&str]| {
            Job::new("git")
                .arg("--git-dir")
                .arg(checkout.path().join(".coder-git"))
                .arg("--work-tree")
                .arg(checkout.path())
                .arg("-c")
                .arg("user.name=Test")
                .arg("-c")
                .arg("user.email=test@example.invalid")
                .args(args.iter().copied())
                .bounded(Limits::within(Duration::from_secs(30)).keeping(64 * 1024))
                .run()
        };
        let clean = scratch(&["status", "--porcelain"]).await;
        assert!(clean.ending.success(), "{}", clean.stderr.text);
        assert_eq!(clean.stdout.text, "", "the seed commit covers the tree");
        let base = scratch(&["log", "--format=%s"]).await;
        assert_eq!(base.stdout.text.lines().count(), 1, "{}", base.stdout.text);

        std::fs::write(checkout.path().join("b.txt"), "changed\n").unwrap();
        for args in [&["add", "-A"][..], &["commit", "--quiet", "-m", "the item"]] {
            let result = scratch(args).await;
            assert!(result.ending.success(), "{}", result.stderr.text);
        }
        let stat = scratch(&["show", "--stat", "--format=", "HEAD"]).await;
        assert!(stat.stdout.text.contains("b.txt"), "{}", stat.stdout.text);
        assert!(!stat.stdout.text.contains("a.txt"), "{}", stat.stdout.text);
        assert!(
            !stat.stdout.text.contains(".git"),
            "neither Git directory is staged: {}",
            stat.stdout.text
        );
        checkout.close().await.unwrap();
    }
