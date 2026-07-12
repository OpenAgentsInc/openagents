# EP250 live-proof rerun and exit-honesty receipt

- Issue: [#8712](https://github.com/OpenAgentsInc/openagents/issues/8712)
- Date: 2026-07-12
- Class: receipt
- Final disposition: retained counterexample; later owner-accepted receipt closed #8712
- Dispatch: no
- Owner: Sol Episode 250 proof
- Base: `b2339bfcb9`

## Outcome

The integrated real-provider proof was rerun against the actual default Pylon
registry. Shell, Fleet, new-chat, and harness-selection structure passed. The
named provider acceptance did not:

- all seven registered isolated Codex accounts failed the preflight probe;
  the freshly linked `codex-2` inventory row is `usage_limited`, while the
  other isolated accounts are revoked or otherwise reconnect-required;
- the selected Claude account reached the real provider but returned the
  provider policy failure that the organization has disabled Claude
  subscription access for Claude Code; and
- therefore `account-preflight`, `fable-turn`, `codex-chip`, and `codex-turn`
  failed honestly. No provider success or #8712 closure is claimed.

The first attempt exposed a proof-harness defect: Electron exited before the
summary and the package command returned zero despite persisted required-step
failures. The live-proof command now runs through a journal-verdict wrapper.
After Electron exits, the wrapper independently requires one successful
receipt for every required step. Failed, missing, malformed, or absent journal
evidence forces exit 1 regardless of the child exit code.

The post-fix rerun completed the full journal and exited 1 with the exact four
required failures above. Optional file-save/git-review failures were also
recorded without being promoted into provider claims.

## Verification

- focused live-proof suite: 16 passed, 0 failed;
- full Desktop verification: 1,025 passed, 1 documented skip, 0 failed,
  5,517 assertions;
- Desktop typecheck and build passed;
- built Electron smoke passed with lifecycle teardown `active: 0`; and
- real live-proof command exited 1 and printed the failed/missing journal
  verdict rather than returning a false zero.

## Remaining gate

#8712 needs at least one probe-verified Codex account with available usage and
one Claude account whose organization permits Claude Code subscription access
(or the separately accepted API-key account path), followed by a clean
integrated live-proof run. The provider-capacity/policy failure is external to
the already-green deterministic Desktop surface.
