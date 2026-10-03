# Historical reserve task: Linux chat catalog notifications

This is a sanitized display copy of the frozen private task input. Owner
checkout paths, the historical test hostname, and the commit attribution
are replaced with labeled placeholders. The executor receives the complete
private task, whose digest is recorded separately. Operational instructions
inside the historical assignment are overridden by the experiment contract.

Implement the historical bug fix below in this source export. Preserve existing behavior outside the requested correction and add meaningful regression coverage. The experiment operational contract overrides the historical instructions to use SSH, run commands, claim issues, commit, push, or remove checkouts. A separate verifier handles isolated Linux formatting, crate tests, and behavioral checks. Do not access owner computers or external services.

BEGIN ORIGINAL HISTORICAL ASSIGNMENT
Fix OpenAgents issue #9989 (`gh issue view 9989`): `a_direct_connection_nudges_the_catalog_when_any_harness_starts_a_chat` fails on Linux ([LINUX_TEST_HOST]) on origin/main. Repo OpenAgentsInc/openagents. Worktree: `git -C [REPOSITORY] fetch origin && git -C [REPOSITORY] worktree add --detach [WORKTREE] origin/main`. Read AGENTS.md, INVARIANTS.md, the test (git grep it; likely crates/coder-connect/tests/direct.rs or coder-host) and crates/coder-connect/src/direct/watch.rs (file-watcher nudges from 69a3a2d7c1; Linux uses inotify via `notify`; note the host now serves only Coder chats since 951da41b10 — the test may still assume all harnesses, i.e. it might be outdated rather than flaky). Reproduce on [LINUX_TEST_HOST] over ssh in a scratch clone with its own CARGO_TARGET_DIR (don't disturb running services; build with `cargo +1.97.1`; the NixOS box may need `nix develop ~/openagents/os#compositor -c` for some deps — only if needed). Find the root cause; fix code or the test (if the behavior it asserts was intentionally removed, update the test to the current contract and cite the commit). Prove with 50 consecutive passes on Linux and macOS. clippy -D warnings, fmt. Commit "Closes #9989", push HEAD:main (message ends with "[HISTORICAL_MODEL_ATTRIBUTION]"). Remove worktree and scratch clone. Report root cause and hash.
END ORIGINAL HISTORICAL ASSIGNMENT

BEGIN ISSUE BODY RECORDED AT THE ORIGINAL START
Found while verifying #9980. On [LINUX_TEST_HOST] (Linux) the coder-connect/coder-host test `a_direct_connection_nudges_the_catalog_when_any_harness_starts_a_chat` fails, and it fails the same way on origin/main without #9980. Find the cause, then fix the code or the test's synchronization properly (no sleeps that mask a bug), and prove it with repeated runs on Linux and macOS. Refs #9965.
END ISSUE BODY RECORDED AT THE ORIGINAL START
