    // Added by the issue-flow evaluation: the test the fix for #9448 added,
    // renamed so it can't collide with a candidate's own tests.
    #[test]
    fn issue_eval_a_rewritten_remote_still_names_the_checkout() {
        let dir = tempfile::tempdir().unwrap();
        let repository = dir.path().join("checkout");
        std::fs::create_dir(&repository).unwrap();
        git(&repository, &["init"]).unwrap();
        git(
            &repository,
            &[
                "remote",
                "add",
                "origin",
                "https://github.com/OpenAgentsInc/openagents.git",
            ],
        )
        .unwrap();
        let global = dir.path().join("gitconfig");
        std::fs::write(
            &global,
            "[url \"https://git-manager.devin.ai/proxy/github.com/\"]\n\tinsteadOf = https://github.com/\n",
        )
        .unwrap();
        let previous = std::env::var_os("GIT_CONFIG_GLOBAL");
        unsafe { std::env::set_var("GIT_CONFIG_GLOBAL", &global) };
        let rewritten = git(&repository, &["remote", "get-url", "origin"]).unwrap();
        let same = same_repository("https://github.com/OpenAgentsInc/openagents", &repository);
        let different = same_repository("https://github.com/OpenAgentsInc/coder", &repository);
        match previous {
            Some(value) => unsafe { std::env::set_var("GIT_CONFIG_GLOBAL", value) },
            None => unsafe { std::env::remove_var("GIT_CONFIG_GLOBAL") },
        }
        assert!(
            rewritten.starts_with("https://git-manager.devin.ai/proxy/"),
            "the rewrite has to be in force for the check to mean anything: {rewritten}"
        );
        assert!(same.met, "{}", same.found);
        assert!(!different.met, "{}", different.found);
    }
