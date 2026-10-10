# Bring your own Claude: Claude Code in Cloud computers

Written October 8, 2026. This page defines how a Cloud computer can run Claude
Code on the user's own Claude plan or API key, within Anthropic's published
terms. It is the policy source for the BYO-Claude issues listed at the end.

## What Anthropic's terms allow

Source: [Claude Code legal and compliance](https://code.claude.com/docs/en/legal-and-compliance)
and [authentication](https://code.claude.com/docs/en/authentication), read on
October 8, 2026. Recheck both before each availability decision.

- A platform may preinstall and run Claude Code in hosted sandboxes, under
  Anthropic's Commercial Terms of Service.
- The binary must be installed and run as published. We may not remove,
  disable, or restrict any of its sign-in methods.
- Each end user signs in with their own Claude plan, their own Anthropic API key,
  or their own Bedrock, Vertex, or Foundry credential. Usage is billed to them.
  We may not pay for, resell, or intermediate that usage.
- Third-party developers may not offer Claude.ai login in their own
  applications, route requests through Free, Pro, or Max credentials on behalf
  of their users, or **collect, store, or intermediate Claude.ai credentials or
  session tokens**. Sign-in must complete through Anthropic's own flow.
- Pro and Max limits assume ordinary, individual use.
- We may say in plain text that a computer "runs Claude Code". We may not use
  the Claude Code or Anthropic names or logos in our product names or logos, or
  suggest endorsement.

## Rules for OpenAgents

1. **Sign-in happens inside the user's computer.** The user runs `claude` (or
   `/login`) in their own computer's terminal and completes Anthropic's browser
   flow. In a hosted terminal, the browser shows a code that the user pastes
   back into that same terminal. Our web app never shows its own Claude login
   form and never asks for a token.
2. **We collect no Claude.ai credential except the user's own subscription
   token, saved by them.** Since 2026-10-09 (owner-directed, #11204) Settings,
   Claude accepts the user's own `claude setup-token` value (below). No other
   openagents.com field, API, Secret Manager entry, or Coder setting accepts a
   claude.ai OAuth token, and no path ever takes a login document or refresh
   token. A login made inside a computer lives only there
   (`~/.claude/.credentials.json` in its isolated home).
3. **No path reads it back out.** Evidence capture, terminal recording, ATIF,
   export, logs, crash reports, support tooling, and saved environment images
   exclude Claude login files and redact `CLAUDE_CODE_OAUTH_TOKEN` values.
   Terminal input during a sign-in is never retained.
4. **It persists only in that user's own computer.** A working-computer
   checkpoint may carry the login so the user signs in once per computer. It
   never enters a reusable or shared environment image, another user's
   computer, or an operator copy.
5. **The binary is unmodified.** Runtime images install the published Claude
   Code release and pin its version. We do not patch, wrap away, or disable its
   sign-in choices.
6. **Usage stays the user's.** We do not meter, price, or resell Claude usage
   that runs on the user's plan or key. Our charges cover our computer, not
   their model usage.
7. **Plans get individual-use concurrency.** Automated turns on a Claude plan
   run one at a time per computer by default, respect usage-limit errors by
   pausing until the reported reset, and never fan out a fleet on one plan.
   Parallel or fleet work uses an API key or cloud-provider credential.
8. **API keys are a different custody class.** A user's own Anthropic API key,
   or Bedrock/Vertex/Foundry credential, may be stored in our secret custody
   for that user's own computers, billed to the key owner, revocable by the
   user, and never shared or resold.

## Product features within these rules

| Feature | How it stays compliant |
| --- | --- |
| Choose Claude Code as a computer's engine | Unmodified pinned binary in the runtime image. |
| One-click "Sign in to Claude" | Opens the computer's terminal and runs `claude`; Anthropic's flow does the rest. |
| Signed-in status, plan, and expiry warnings | Read by running the binary inside the computer; only status text leaves it, never credentials. |
| Sign in once per computer | Login persists in that user's own checkpoint (CMP-01). |
| Background tasks on your Claude plan | Coder drives the unmodified `claude -p` inside the user's computer on their login, one at a time, with pause-until-reset on limits. |
| Usage-limit display | Shows the reset time Claude Code reports; we keep no usage ledger for their plan. |
| Parallel fleets | Require the user's own API key or cloud credential. |
| Phone and web observation | Observers see the task and its evidence, never the credential. |

## Implemented (BYO-01, #11008)

- `coder_cloud::claude` pins the engine (`claude`, `@anthropic-ai/claude-code`
  at `VERSION`, installed at `/usr/local/bin/claude` by
  `scripts/cloud/coder-host-setup.sh`) and refuses `CLAUDE_CODE_OAUTH_TOKEN`
  and any claude.ai login value in every Coder credential path.
- `secret-screen` recognizes and redacts claude.ai logins; Cloud traces,
  events, and results pass through it, and artifacts refuse
  `.credentials.json`.
- The web workbench's "Sign in to Claude" opens a terminal on the user's
  computer running the unmodified program; the terminal host keeps only
  digests of typed input. The account sign-in form refuses claude.ai logins.

## Implemented (BYO-02, #11009)

- Status comes from the pinned binary itself: the host in the user's
  computer runs `/usr/local/bin/claude auth status` (JSON; exit 0 signed in,
  1 signed out) with standard input closed and standard error discarded,
  and reads only `loggedIn`, `authMethod`, `subscriptionType`, and a login
  expiry when the release reports one. The account email, organization, and
  everything else stay in the computer. No credential file is read.
- Usage limits and logins that stopped working show up only when Claude
  Code runs. The Coder delegate keeps the last one as a typed notice (kind
  and times, never text) in `~/.openagents/engine/claude.json` in that
  computer; a normal run clears it. The usage-limit reset is the one Claude
  Code reported. There is no usage ledger for the user's plan.
- `coder_engine_status::Status` is the only thing that leaves the computer:
  closed enums and Unix seconds, no string field, so it cannot carry
  credential bytes (tested). States: signed out, signed in, expiring
  (three days, as Claude Code warns), expired, rate limited, API key or
  cloud credential, and unavailable.
- The host answers it as a NIP-TERM engine status read
  (`coder_pty::engine`, terminal right required; the device names only the
  engine, never a program). The web workbench shows it in plain text with
  Renew Claude sign-in, which reuses BYO-01's sign-in terminal; the native
  terminal client shows the same summary once when the person must act.

## Implemented (BYO-04, #11011)

- `coder_cloud::claude::OwnCredential` defines the user's own credential
  classes: an Anthropic API key (`ANTHROPIC_API_KEY`), and Bedrock, Vertex,
  and Foundry documents (`OA_CLAUDE_BEDROCK`, `OA_CLAUDE_VERTEX`,
  `OA_CLAUDE_FOUNDRY`). Each is validated to a canonical shape, refuses
  claude.ai logins, and expands at launch into the variables the unmodified
  binary reads (`CLAUDE_CODE_USE_BEDROCK`, `AWS_*`, `CLAUDE_CODE_USE_VERTEX`
  with a private per-job service-account file removed after the run,
  `CLAUDE_CODE_USE_FOUNDRY`, `ANTHROPIC_FOUNDRY_*`). Values are redacted from
  traces and refused in artifacts like every selected credential.
- `coder_cloud::claude::admit_turns` is the concurrency rule: a plan login
  admits one automated turn at a time and refuses a fan-out with a pointer to
  adding a key; an own credential is not limited. The Coder operator applies
  it to every Claude submit and continue on a profile without an own
  credential. BYO-03 builds its scheduling on this rule.
- `openagents_web::cloud::byo` keeps the credential in the shared WEB-09
  custody vault under subject `byo:computers`, scoped to the account,
  workspace, and membership epoch, with explicit consent, digest-only status,
  and one current credential. `Computers::credentials` releases it fresh for
  each boot or automated turn (never into a checkpoint, image, export, or
  evidence); removal erases the entry, so running computers lose it at their
  next start or turn and future computers never see it. The page is
  `/settings/claude` (enabled by `--cloud-byo PRIVATE_DIR` with a keyring,
  below) and states that usage bills to the user's own Anthropic or cloud
  account.

### How saved credentials are kept

Fixed in [#11041](https://github.com/OpenAgentsInc/openagents/issues/11041).
A saved credential is encrypted before it touches disk. A copy of the custody
directory (a disk snapshot, a backup, a stolen volume) holds only ciphertext.

- **The lock.** Each saved credential is encrypted with AES-256-GCM
  (`crates/oa-seal`) under a key from a keyring. Each encryption uses a fresh
  random nonce. The entry's owner, workspace, epoch, digest, dates, and
  accepted terms stay readable so status needs no decryption. They are bound
  to the ciphertext, so editing any of them makes the entry unreadable.
- **Where the key lives.** Never in the custody directory. The server reads
  the keyring from `--cloud-byo-keys PRIVATE_JSON` (a file owned by the server
  user, `chmod 600`, not a symbolic link, and refused if it sits inside the
  custody directory) or from the environment variable
  `OPENAGENTS_WEB_CLOUD_BYO_KEYS` (for a Secret Manager secret on Cloud
  Run). `--cloud-byo` refuses to start without one; there is no unencrypted
  mode. The keyring document is:

  ```json
  {"schema":"openagents.seal.keyring.v1","current":"2026-10","keys":{"2026-10":"<32 random bytes, base64>"}}
  ```

  Make a key with `openssl rand -base64 32`.
- **Rotation.** Each entry records the id of the key that sealed it. To
  rotate, add a new key to `keys`, point `current` at it, and restart. Older
  entries still open, and each is resealed under the new key the next time it
  is read. Drop the old key only once nothing uses it. An entry sealed under
  a key that is no longer in the keyring cannot be read: the person adds
  their credential again.
- **Older plain-text entries.** Entries saved before this change held the
  credential in plain text. The first read seals them, and the plain-text
  bytes are overwritten with zeros before the sealed file replaces them.
- **Failing closed.** A wrong key, a missing key, or a changed byte makes the
  entry unavailable. The server never falls back to plain text, never rewrites
  an entry it could not open, and the owner's automated turns are refused
  (not quietly moved to the plan login) until the person removes or re-adds
  the credential.
- **Kept from before.** Entry files are still owner-only (`0600`) in an
  owner-only directory, written atomically and synced. A replaced or removed
  entry's bytes are zeroed before it is unlinked. Keys in memory, including
  the keyring, are zeroed when dropped and never printed.
- **Local fixtures** (`cloud_session_fixture`, `github_auth_fixture`, and the
  tests) make a throwaway keyring next to the custody directory.

## Implemented (BYO-03, #11010)

- A Cloud job whose engine is `claude` runs the Coder runtime in the user's
  computer, which drives the pinned, unmodified binary as `claude -p
  --output-format stream-json` with the task on standard input
  (`coder_new::bundled_runtime::claude_print`). It runs on the sign-in that
  computer holds: the login made there, or an own credential the run
  admitted. No `CLAUDE_CODE_OAUTH_TOKEN` is passed and no login file is read.
- Concurrency is BYO-04's `claude::admit_turns`; a paused task still counts
  as the plan's one turn, so a plan never fans out while it waits.
- A usage limit does not fail the task. The bridge reports the reset Claude
  Code gave; `coder_cloud::claude_task` marks the job `paused` with that
  reset, records the limit through the capacity owner
  (`microcoder_loop::capacity`) in a book per computer under the operator
  state (`claude-capacity/PROFILE`, no account fingerprint is read), and the
  operator continues the same session with a resume turn after the reset.
  The pause lives in the job record, so `Operator::load` (`resume_paused`)
  picks it up after a restart; a follow picks it up too, and cancel works
  while paused. A reset that already passed resumes at once; no reported
  reset holds for the capacity owner's default.
- A missing or expired login stops the task with a prompt to use Sign in
  to Claude for that computer (BYO-01's terminal action) and a
  `sign_in_required` event; the job can continue after sign-in.
- Evidence: the job's first event and binding name the engine, the pinned
  version, and the credential type (`claude_plan_login`,
  `anthropic_api_key`, `bedrock`, `vertex`, `foundry`); the bridge's answer
  adds the version and credential type Claude Code reported. Never a
  credential. Tests use a fixture `claude` stand-in only.

## Implemented (BYO-05, #11012)

- The web server releases the stored credential itself. When a confirmed
  Cloud submit, continue, or follow starts a turn on a Claude Code profile
  that names no credential of its own, `cloud::byo::release_turn` reads the
  key for the binding's account, workspace, and membership epoch (which
  must also be the signed-in viewer's) and sends it to the resident just
  before the effect. It goes as a `cloud.release` operation on a fresh
  authenticated native channel. It is never part of the staged packet or
  the effect book, and the host retains neither the request nor its reply.
  With no credential stored, nothing is sent.
- The operator (`coder_cloud::release`) keeps it in memory only. An offer
  waits up to two minutes for the effect of the same job from the same
  device. The effect binds it to that one turn, and the backend adds it
  only to that turn's process environment (the private per-job file the
  launch script reads and removes) and redacts it from the turn's events,
  results, and artifacts. The turn's end drops it. The job keeps only the
  credential type in its evidence, plus a digest of the owner. A release
  from another device, owner, or membership epoch is refused.
- After removal, the next turn gets no release. It runs on the plan login
  made inside the computer, admitted one at a time beside any key-backed
  turns. A job paused on a released key stays paused until a follow
  brings a fresh release. Parallel turns are admitted only when each has
  the user's own released key. Tests use fake keys and synthetic owners
  and check that the effect journal, the resident's access book, and the
  operator's records, admissions, journals, and archives never hold it.

## Claude subscription tokens (#11204)

Owner-directed on 2026-10-09: "I need to support OAuth tokens, not just API
keys." `claude setup-token` prints a one-year token (`sk-ant-oat01-…`) that
Claude Code reads from `CLAUDE_CODE_OAUTH_TOKEN` and that bills the person's
Claude plan. This relaxes rule 2 for that one value. Anthropic's terms quoted
above say third parties may not collect or store Claude.ai credentials; the
owner accepted that risk for this class. Recheck the terms before opening it
beyond the invite-only site.

- **Settings, Claude** offers "Claude subscription token (from claude
  setup-token)" beside "Anthropic API key". A pasted value is told apart by
  its prefix (`sk-ant-oat` or `sk-ant-api`), whichever of the two was picked
  (`cloud::byo::detect`). Only the bare token is accepted
  (`OwnCredential::SubscriptionToken.canonical`): never a credentials
  document, a refresh token (`sk-ant-ort`), or a token with anything around
  it.
- **Checked before it is kept.** `Computers::check` sends one
  `GET https://api.anthropic.com/v1/models?limit=1` with
  `anthropic-version: 2023-06-01` and, for a token, `Authorization: Bearer`
  plus `anthropic-beta: oauth-2025-04-20` (the header Claude Code sends); for
  an API key, `x-api-key`. No model call is made. 2xx or 429 keeps it; 400,
  401, or 403 is "Anthropic didn't accept that…"; anything else is "couldn't
  be reached", and nothing is saved either way.
- **Kept like an API key.** Material `claude_subscription_token` in the same
  sealed custody (#11041), subject `byo:computers`, scoped to the account,
  workspace, and membership epoch; status and Settings show a digest and the
  label only. Saving one replaces any other class.
- **Runs.** The release names it `CLAUDE_CODE_OAUTH_TOKEN`
  (`OwnCredential::SubscriptionToken`), and the launch environment holds that
  variable alone, never `ANTHROPIC_API_KEY`. `coder_cloud::claude::admit`
  admits a token only under that exact name; any other name still refuses a
  claude.ai login. Inside the computer, `claude -p` gets the token only when
  the run admitted it (`OA_CODER_CLOUD_CREDENTIAL_NAMES`); otherwise it is
  removed and the computer's own login is used. Runs on a Boat image whose
  Coder runtime predates this change remove the variable, so those need a
  runtime template built from this commit or later. The job evidence names
  the type `claude_subscription_token`.
- **Plan concurrency.** A token bills a plan, so `admit_turns` treats it like
  a plan login: one automated turn at a time, no fan-out (rule 7).
- **Not in the API.** The gateway's own-key path (`pay: "mine"`) and the
  Settings API-keys form refuse a subscription token for every provider
  (`subscription_token`): Anthropic permits it only inside Claude Code, and
  the gateway calls the Messages API directly.

## Existing docs this supersedes

- `2026-10-02-boat-sdk-plan.md` describes connecting Claude Pro or Max on Boat's
  dashboard, with Boat writing `~/.claude/.credentials.json` into sandboxes, and
  lists `CLAUDE_CODE_OAUTH_TOKEN` as an injectable key. That path may only serve
  the owner's own internal work on the owner's own plan. It must not carry
  customer Claude plans.
- `2026-10-02-cloud-parallel-execution-audit.md` lists the long-lived
  `claude setup-token` value for burst hosts. Customer Cloud must not collect or
  inject that token. Burst and fleet hosts use API keys.

## Issues

See the BYO-Claude issues referenced from
[#10964](https://github.com/OpenAgentsInc/openagents/issues/10964). Owner action:
accept Anthropic's Commercial Terms for OpenAgents before any customer
availability (recorded in the workspace `NEEDS_OWNER.md`).
