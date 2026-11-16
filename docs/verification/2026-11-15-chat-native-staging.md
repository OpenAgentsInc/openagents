# Native task staging qualification

Live qualification for #13620. This record distinguishes setup, actual native
SDK observations and the coding task result. Acceptance remains open until the
task and installed phone/CLI controls have complete receipts.

## Setup and SDK observations

- API source: `92825459623454ff2a587efc171c42380b90e462`, including the landed
  capture/Stop recovery in `fbf70360d9db0fddabe4ab0044dc901d9e959cbc`.
- Staging revision: `oa-api-staging-00234-fbg`; the immutable image is
  `sha256:b929b300b283be526d56099c33d297cd30cbeea8430fca2c9808a0b291a481fe`.
  Staging migrations, deployment verification and the HTTP smoke passed.
  Production was not deployed by this qualification.
- Staging previously had no HOST broker secret. A new, independent staging
  signer was provisioned in Secret Manager and mounted by the existing
  deployment path. It was paired through the existing `browser.request` and
  local `allow` flow; no key bytes appear in this record.
- The isolated resident uses `CODER_HOME=~/.openagents/q13620` and a separate
  host key. Its workspace is `~/.openagents/qualifications/13620-native/workspace`.
  The existing resident and its main-checkout binding were left untouched.
- The catalog probes used the existing `durable-native-staging` CLI account.
  The selected simulator has a different existing phone key. Its account was
  enrolled in the staging team beta and the scratch enrollment moved to that
  account, preserving exclusive ownership. The phone approved the separate
  `native-phone-staging` CLI login through the existing device approval route.
  The fresh resident was explicitly enabled for this scratch workspace and
  provider-account access.
- Both real catalog probes through the installed CLI returned `ready`:
  eight Codex models and thirteen Claude models, with the SDK's actual effort
  identifiers. These are live catalogs, not the controlled SDK fixture.

The initial isolated socket path was 104 bytes, which the macOS Unix socket
contract refuses. A shorter explicit `CODER_HOME` resolved it without changing
the product. The first CLI offer request correctly returned 503 while staging
still lacked the broker mount; the post-deployment probes succeeded.

The scratch `sum.coax` has one check with three arithmetic assertions. Its
unmodified subtraction implementation fails the first assertion. The real
coding task must change that implementation and pass the same checks; setup
does not count as the result.

Receipts are under `/tmp/oa-chat-task-artifacts/`: `staging-api-prepare.log`,
`staging-api-deploy.log`, `staging-pair-request.log`, `staging-resident.log`,
`staging-codex-offer2.log`, `staging-claude-offer.log` and
`staging-patch-baseline.log`. Signer material stays outside the repository.

## Coding task and installed client result

The phone sent the coding prompt through its actual generated program, native
signer, device proof and HTTP adapter. The router selected the scratch Mac,
Codex, `gpt-6.1-sol` and medium effort. The saved identities are:

- Conversation: `cnv_941c26ea2caaf4fa0238b4f4b0efd31d`.
- Turn/task: `e469f5395d4b1ada295e6c043bd2cd8d` (the turn has its `trn_` prefix).
- Native session: `01a1e2d8-d2b4-7e41-b874-87819be06a1e`.

The installed CLI read the same waiting task and whole captured transcript,
then approved its first exact held interaction. A fresh phone program reopened
that task and approved the remaining four permission requests. No second
Start was sent. The native agent observed the original failed check, changed
only `sum.coax` from `return a-b;` to `return a+b;`, reran the same checks and
returned `STAGING-NATIVE-PATCH-13620` with `1 check, 1 passed, 0 failed`.
Independent inspection confirms that exact one-line patch and test result.
The CLI's durable follow received the completed event and saved answer.

The final native capture is complete at revision 28, 5,725 bytes and SHA-256
`0ebab762c09a97473e4f2d5a4ff04f2de86ddd9478efd021a08f5e2b139c302f`.
It preserves all five native tool IDs and their results, including the initial
failed check. The result was consumed by subsequent agent steps and the final
saved answer. Export: `~/.openagents/qualifications/13620-native/capture-final.jsonl`.

The terminal-task reopen case passes in
`/tmp/oa-native-task-staging-r7.xcresult` (4.7 seconds). Its XCTest attachment
retains the exact task association, saved completed turn and visible computer
and model. The computer label comes from the original station report,
`Christophers-MacBook-Pro-2`; the account enrollment alias is not that report's
display field.

Earlier runs are not passes. The first refused a different phone account;
later fixture diagnosis replaced a removed sidebar control and made the
driver yield at the installed wait boundary. A frame observer now allows a
real permission while the wait leaves the control enabled. Run r4 created
the one actual task but exhausted the old driver's effect-chain bound. Run r6
resumed that task, allowed four requests and observed completion; its final
display assertion expected the wrong label. Run r7 verifies the corrected
reopen expectation without creating another task. These receipts establish a
real phone-started coding task and reconnect; they do not establish Deny,
Stop, all provider-account choices or the full remaining issue scope.
