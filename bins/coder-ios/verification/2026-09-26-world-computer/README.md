# World-computer verification artifacts

These fixtures contain synthetic data. See the [assessment](../../../../docs/coder/verification/2026-09-26-world-computer.md) for scope and results.

- `verification.json` binds the changed product and native test sources to SHA-256 hashes.
- `rust-*.log.gz` and `rust-gate.json` retain the targeted checks.
- `ios-initial-full-suite.log.gz` precedes the final geometry and interaction corrections.
- `ios-final-world-suite.log.gz` covers the corrected source; `ios-attachments.json` identifies its screenshots.
- `ios-monitor.png` shows the physical monitor immediately before the real surface tap. `ios-open-computer.png` shows the resulting native reader panel.

- `android-*-results.xml` and `android-*.log.gz` retain all three attempts: 12/13, 11/13, then 0/2. The last APK's monitor/reader case passed; the full suite did not.
- `android-test-experiments.patch` contains unsuccessful test-only changes from the focused rerun. They were restored afterward; they are not part of the shipped implementation.
- `android-motion-failure.png` shows the slow-emulator diagnostic state. [Issue #9714](https://github.com/OpenAgentsInc/openagents/issues/9714) tracks the remaining motion and Gym timing failures.

`testflight-build43.json` records the clean archive source, signature verification, executable digest, successful upload, and Apple's `VALID` / `IN_BETA_TESTING` state. Simulator evidence does not claim physical-device acceptance.
