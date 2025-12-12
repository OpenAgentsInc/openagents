# 2131 Work Log (cleanup-temp-dirs)

- Redirected runLog and tasks CLI integration tests to os.tmpdir to avoid polluting repo root with runlog-test-* and tasks-cli-* folders.
- Removed existing temp directories from repo root.
- bun test

