# Build 24: checks pay either way (2026-09-29)

The essay's revision of 2026-09-29 (commit `65f53d6d15`) separated the
claim key, the evidence records, and the decision policy, paid checks
whether they confirm or dispute, required a second test set before
adoption, and set the admission's lifetime by the subject's identity.
This build carries the app's side of it.

## What changed in the app

| Where | Before | Now |
| --- | --- | --- |
| Check card (menu, **Check a result**) | "Run the same tests to check it. A check doesn't use a daily run." | The same, plus "and it earns XP whether it confirms the result or not" when the check would earn XP |
| Result card, XP line | "+50 XP once our referee confirms your check." | "+50 XP once our referee signs your check, whichever way it went." |
| Result card, why | "When other trainers confirm it, Coder can use this tool for everyone." | "When other trainers confirm it and it holds up on a test set someone else wrote, Coder can use this tool for everyone." |
| Add to the Gym sheet | "If your check confirms the result, you and the trainer who added it earn XP." | "You and the trainer who added the result earn XP whether your check confirms it or not." |
| Add to the Gym, published | "Added to the Gym. If your check confirms the result, XP comes once our referee signs it." | "Added to the Gym. XP comes once our referee signs your check, whichever way it went." |
| Next line | "Next: a check confirmed your result. XP is on its way." | "Next: someone checked your result. XP is on its way." |
| Profile, Your results | "Your check was confirmed" / "Your check holds up. XP is on its way" | "Your check earned XP" / "Your check followed the rules. XP is on its way" |
| What's new, build 22 items | "...earn XP when you confirm them" / "When a check confirms a result, both trainers earn XP, and Coder can adopt a tool that holds up." | "...earn XP when you check them" / "A check earns XP whether it confirms the result or not, and so does the trainer who added it. Coder adopts a tool only after checks confirm it and it holds up on a test set someone else wrote." |
| What's new, build 24 | none | "Checks pay either way", three items, with its What to test line |

The copy follows the rule the ledger already enforces: `eval-check` pays a
protocol-following check whether it confirms or disputes, a dispute
confirms nothing, and adoption needs three confirming checks and one
externally validating result on a suite someone other than the tool's
author released after the tool. The check card's numbers ("passed 5 of 6
tests instead of 2") are the effect estimate the essay asks readers to
compare beside the verdict; nothing new was needed for them.

## Tests

- `cargo test --manifest-path crates/openagents-mobile/Cargo.toml`: 190
  passed, 20 ignored. The three tests that pin the sheet and XP-line
  copy were updated with it.
- `cargo test` for `nostr` (eval), `xp-ledger`, `ext-eval` (expected
  reports re-blessed), `gym`, `eval-runner`, and `microcoder` (xpnet);
  `openagents-cli`, `verse`, and `coder` build; clippy clean on the
  edited workspace crates.

## Build 24

Archived from `65f53d6d15` (clean) with `bins/openagents-ios/build.sh
archive`; the archive's Info.plist says `com.openagents.app` 1.0.0 (24).
Uploaded with `build.sh upload` between 16:57 and 16:59 UTC ("Upload
succeeded", "EXPORT SUCCEEDED"). App Store Connect (filtered by pre-release version 1.0.0 and build 24)
shows it uploaded at 2026-09-29T17:01:04Z, processing state `VALID`,
checked at 17:02 UTC.
