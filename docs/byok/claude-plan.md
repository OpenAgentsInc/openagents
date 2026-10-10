# Your Claude plan in OpenAgents

Written October 10, 2026. This is the consolidated plan for running OpenAgents
work on a person's own Claude Pro, Max, Team, or Enterprise plan. It maps what
Amp shipped in [Use Your Claude Plan in Amp](https://ampcode.com/news/use-your-claude-plan)
(October 10, 2026) onto our architecture, item by item. The policy rules stay in
[`docs/cloud/claude-code-byo.md`](../cloud/claude-code-byo.md); this page is the
product plan on top of them.

## What Amp shipped, and what Anthropic says

From the post and the pages it relies on (all read October 10, 2026):

- **One choice when starting work.** Pick Mode > Claude Code when starting a
  thread (or Ctrl+S in the CLI). If no plan is linked, Amp asks for one right
  there. It can also be linked ahead of time in Settings → Model Routing as
  "Claude Pro/Max" ([Amp docs](https://ampcode.com/docs/the-dial#use-your-claude-subscription)).
- **The Claude Agent SDK is the engine.** That mode runs the Claude Agent SDK
  instead of Amp's own agent, and keeps Amp's features around it: orbs (Amp's
  hosted computers), runners (the person's own machines), thread sharing,
  portals, multiplayer, and thread-to-thread messaging.
- **Two places it runs.** Orbs come with Claude Code installed and use the
  plan linked in Amp's settings. On a runner, Claude Code runs on that machine,
  as the runner's user, with that machine's own install, `/login`, and
  `~/.claude` settings.
- **The plan only.** Amp removes `ANTHROPIC_API_KEY`, `ANTHROPIC_AUTH_TOKEN`,
  and the Bedrock, Vertex, and Foundry settings from Claude Code's environment
  in this mode, so only the plan is used.
- **Setup details.** Install with `curl -fsSL https://claude.ai/install.sh | bash`;
  the runner looks on its starting `$PATH`, `~/.local/bin`, and `~/bin`; it
  must run as a regular user, because Claude Code refuses to skip permission
  prompts as root (`IS_SANDBOX=1` works in a throwaway container, undocumented).
  Failures show "Claude Code Not Found" or "Claude Code Could Not Start" with
  Retry.
- **What it costs.** Free. Runners are free; orbs are paid by the minute and
  not covered by the plan. Amp's own image tool still uses Amp credits; its
  extra helper agents are left out to avoid charges; Claude Code's plan mode,
  todo tool, and notebook editing are off.
- **Pricing context** ([Free Agent](https://ampcode.com/news/free-agent)):
  Amp charges for compute, not for bring-your-own tokens, and lets people bring
  Bedrock, Vertex, Foundry, OpenRouter, and other keys in Model Routing.
- **Anthropic's own word.** The help center article Amp links,
  [Use the Claude Agent SDK with your Claude plan](https://support.claude.com/en/articles/15036540-use-the-claude-agent-sdk-with-your-claude-plan),
  says (update of October 7, 2026): "You can still use the Claude Agent SDK,
  `claude -p`, and third-party apps with your subscription limits." Its June 15
  update paused the earlier announced change to Agent SDK usage.
- **Monthly API credits.** Max and Team plans now include monthly Claude
  Platform credits ($100 Max 5x, $200 Max 20x, $20 or $100 per Team seat pooled
  up to $500), claimed into one Console organization. They cover the API, the
  Agent SDK, and `claude -p` when run with that organization's API key, never
  plan sign-ins or interactive Claude Code
  ([details](https://support.claude.com/en/articles/17154008-monthly-api-credits-for-max-and-team-plans)).
- **Limits.** Pro and Max have a five-hour session limit and a weekly limit;
  Max has a separate weekly Fable limit (up to 50% of the week). On Pro and
  standard Team seats, Fable runs only on paid usage credits. `/status` shows
  what is left; at a limit the person waits, upgrades, or turns on usage
  credits ([Pro/Max](https://support.claude.com/en/articles/11145838-use-claude-code-with-your-pro-or-max-plan),
  [Fable](https://support.claude.com/en/articles/15424964-claude-fable-models-on-your-plan)).

The post does not say how Amp shows plan usage or what happens at a limit;
nothing below copies an Amp behavior there.

## Where our architecture differs

- **Claude Code runs where the person's work runs.** We run the unmodified
  `claude` binary (directly, as `claude -p`, or through
  `crates/claude_agent_sdk`) on the person's own computer through Coder, or
  inside their own Cloud Environment. We never send their plan credential
  through a server-side model API.
- **The gateway refuses plan tokens.** `crates/gateway/src/inference_byok.rs`
  (`admit_key`, `SUBSCRIPTION_REFUSAL`) refuses `sk-ant-oat`/`sk-ant-ort` for
  every provider, because the gateway calls model APIs directly.
- **Decisions are ours.** Routing, judges, file relevance, and Verse questions
  go through our own decision API to our Pylons (#11225), never through a
  person's Claude plan.
- **Google first for what we pay.** Our own model spend goes to Vertex (Gemini)
  first.
- **Bitcoin-only payments.** Payments go over Bitcoin rails (Bitcoin and
  Bitcoin-based stablecoins). Whatever we charge is for our compute; we never
  resell, mark up, or pay for a person's Claude usage, and no Claude plan or
  Anthropic credit purchase ever goes through us.

## The items

### 1. Connect a Claude plan (Settings and the apps)

**Today.** Settings → Claude (`crates/openagents-web/src/settings.rs`,
`claude_content`; storage in `crates/openagents-web/src/cloud/byo.rs`) saves one
credential per account and workspace: a `claude setup-token` value (#11204), an
Anthropic API key, or a Bedrock, Vertex, or Foundry credential. Each is checked
with one `GET /v1/models` and sealed (#11041). Cloud computers also have **Sign
in to Claude** in the Workbench, which opens a terminal running `claude` so the
person signs in through Anthropic's own flow (BYO-01, #11008), and a status
line read from `claude auth status` (BYO-02, #11009). The desktop app has a
Claude engine switch (`crates/openagents-desktop/src/settings_shell.rs`); on the
person's own computer, Coder uses whatever `claude` login is there.

**Change.** Do what Amp does at the moment it matters: choosing Claude Code with
nothing signed in shows one inline card with **Sign in to Claude** (in the
environment's terminal, Anthropic's flow) and **Use a key instead**, then runs
the held message ([#11234](https://github.com/OpenAgentsInc/openagents/issues/11234)).
Settings → Claude leads with sign-in inside the environment, limits saving a
subscription token to an allowlist, and tells Max and Team subscribers about
their monthly API credits, whose Console key works as an ordinary API key
([#11235](https://github.com/OpenAgentsInc/openagents/issues/11235)).

**Different on purpose.** Amp links the plan in its settings and uses it in
its hosted orbs. Anthropic's legal page says third-party developers may not
offer Claude.ai login in their apps or collect, store, or intermediate
Claude.ai credentials ("unless previously approved", in the SDK overview).
Amp may have that approval; we don't. So our default is the sign-in made inside
the unmodified binary, and server-held subscription tokens stay limited until
Anthropic approves them in writing (see item 9).

### 2. Your computer (Coder) or our Cloud Environments

**Today.** On the person's own computer, Coder runs Claude Code on the local
login: chats through `crates/coder-new`, delegated turns through
`crates/coder-delegate`, repository steps through `crates/microcoder-loop`
(`claude.rs`), and Studio seats through `crates/microcoder/src/repository/claude_sdk.rs`.
In Cloud, a job whose engine is `claude` runs `claude -p --output-format
stream-json` inside the person's computer (`crates/coder-new/src/claude_print.rs`)
on the login made there or a released credential; the image pins Claude Code
`2.1.295` (`crates/coder-cloud/src/claude.rs`, `VERSION`) and runs it as the
non-root user `coder` (`scripts/cloud/coder-host-setup.sh`). Environments runs
("Claude Code in REPO", #11162) use the credential saved in Settings.

**Change.** Add Amp's clear local failure cards: not found (look in `$PATH`,
`~/.local/bin`, `~/bin`; show the install one-liner), not signed in (`claude`,
then `/login`), running as root, each with Retry (#11234). Keep the Cloud pin
current with the SDK's tracked release (2.1.296).

**Different on purpose.** Amp strips API keys and cloud-provider settings so
only the plan is used. We let the person choose: an API key or their own
Vertex/Bedrock/Foundry credential is a first-class choice that also unlocks
parallel work. Only our briefed agent strips `ANTHROPIC_API_KEY` and
`ANTHROPIC_AUTH_TOKEN` (`crates/briefed-agent/src/main.rs`, `REMOVED_ENV`), so
it runs on the owner's own Claude Code login. Anthropic's terms
also forbid disabling the binary's sign-in methods, so we only pass or withhold
variables; we never patch the binary.

### 3. `claude_agent_sdk` as the engine for our briefed agents

**Today.** `crates/claude_agent_sdk` tracks SDK 0.3.296 (Claude Code 2.1.296),
with in-process custom tools (`SdkMcpServer`, #11213). It runs the `claude`
binary as a child, so the binary's own sign-in applies, plan or key. It is the
engine of Studio seats (#10571) and of the briefed agent (`crates/briefed-agent`,
#11211), and `crates/coder-new/src/issue_run` uses it.

**Change.** None to the engine. This is the same thing Amp did (the Agent SDK
in place of their own agent), and Anthropic's help center now says SDK use on a
plan draws from the plan's limits. Item 4 reads usage from it.

**Different on purpose.** We keep our own agent (Coder's provider loop) beside
it rather than replacing it. The SDK path is for work that names Claude Code;
our own agent runs our model doors and never touches the person's plan.

### 4. Plan usage you can see

**Today.** Two partial sources. `claude -p` and SDK sessions emit
`rate_limit_event` (`SdkRateLimitInfo`: status, window type, utilization,
reset, overage), which `microcoder-loop/src/claude.rs` keeps per call and
`claude_print` turns into a pause. The fuller reading is the owner-host usage
probe (`crates/coder/src/task/usage.rs`, `microcoder-loop/src/usage.rs`), which
reads the Claude OAuth token from the keychain and calls
`api.anthropic.com/api/oauth/usage`; it is opt-in and recorded in
`INVARIANTS.md`. Cloud shows status and "Usage limit reached" with the reset,
and keeps no usage ledger.

**Change.** Make Claude Code itself the source: record every
`rate_limit_event`, ask the SDK's `get_usage` (Claude Code's own `/usage`)
at session start, and show the five-hour, weekly, and Fable windows in Coder's
status line, the desktop, the web Workbench, and Cloud job cards
([#11233](https://github.com/OpenAgentsInc/openagents/issues/11233)). Pro users
see that Fable runs on paid usage credits on their plan.

**Different on purpose.** The keychain probe reads a Claude login outside the
binary. It stays the owner's opt-in on owner hosts and is never the path for
anyone else.

### 5. Usage limits: pause and resume (#11179)

**Today.** Local chats pause on a usage limit, show "Paused until", and resume
on their own while background agents continue
(`crates/coder-new/src/long_session.rs`, #11179). Cloud jobs pause with the
reset Claude Code reported (`Claude AI usage limit reached|SECONDS`), survive a
restart, and resume with a continue turn (BYO-03). The capacity book
(`microcoder_loop::capacity`) remembers each refusal until its reset.

**Change.** Small: the pause card shows which window ran out (from item 4), and
offers the choices in item 6.

**Different on purpose.** We never buy Anthropic usage credits ("extra usage")
for anyone, and never prompt for them; continuing past a limit with Anthropic
is the person's own choice in their own Claude settings.

### 6. When the plan runs out: failover, Google first

**Today.** Local Coder already orders providers Codex → Claude → our cloud
worker on Vertex (`capacity::Provider::Vertex`), which holds the Google
credential on the worker; the person's own Vertex credential for Claude Code is
supported in Cloud (`OA_CLAUDE_VERTEX`, BYO-04). A paused Cloud Claude task only
waits.

**Change.** A paused task offers **Keep waiting** (default), **Continue on your
key** (the same Claude Code session on the person's own API key or their own
Vertex/Bedrock/Foundry credential), or **Continue with OpenAgents** (Coder's own
agent on our Vertex Gemini door, billed as our normal usage)
([#11236](https://github.com/OpenAgentsInc/openagents/issues/11236)).

**Different on purpose.** We never continue a customer's Claude Code session on
Claude billed to our account, even on our Vertex: Anthropic's terms forbid
paying for or intermediating a user's Claude Code usage. For what we pay, Google
comes first.

### 7. Decisions through our own Pylons, never the person's plan

**Today.** Decisions use the jev door chain (`crates/jev`), the hosted
decision worker, or TypeSafe. None of them start `claude`.

**Change.** #11225 makes our `/v1/systemone` the one decision entry point:
connected Pylons running Clef first, then our hosted Clef, then Vertex Gemini.
We added an acceptance line there: the `claude` binary is never a decision
door, and no decision ever runs on a person's Claude login or token.

**Different on purpose.** Amp's helper agents use Amp's own models and Amp
credits; it turns some off in Claude Code mode to avoid surprise charges. Our
decisions are small and ours, run on our Pylons for free to the person, and
never count against their Claude plan.

### 8. One task at a time on a plan

**Today.** Cloud admits one automated turn at a time on a plan login or
subscription token and refuses a fan-out with a pointer to adding a key
(`coder_cloud::claude::admit_turns`; rule 7). A paused task still holds the
slot. API keys and cloud credentials are not limited. Local Coder has no such
rule; background agents can each start `claude` on the same plan.

**Change.** Local automated and background runs on a plan login take one plan
slot by default and queue the rest with "waiting for your Claude plan"; the
person can raise the number, interactive chats are never queued, and an
`allowed_warning` holds new automated starts
([#11237](https://github.com/OpenAgentsInc/openagents/issues/11237)).

**Different on purpose.** Amp lists no concurrency rule. Anthropic's terms say
plan limits "assume ordinary, individual usage of Claude Code and the Agent
SDK", and we drive automated runs, so we keep fleets on API keys or cloud
credentials. The local default is the person's to change; Cloud stays at one.

### 9. The terms question

**What changed.** Anthropic's help center now says, in plain words, that the
Agent SDK, `claude -p`, and third-party apps can use a plan's limits
([article](https://support.claude.com/en/articles/15036540-use-the-claude-agent-sdk-with-your-claude-plan)).
That settles that our Claude Code runs on a person's plan, on their computer or
inside their Cloud Environment through the unmodified binary, are allowed.

**What did not change.** The [Claude Code legal page](https://code.claude.com/docs/en/legal-and-compliance)
still says developers may not offer Claude.ai login in their own apps, route
requests through Free, Pro, or Max credentials on their users' behalf, or
collect, store, or intermediate Claude.ai credentials or session tokens, and
that sign-in must complete through Anthropic's own flow. It also says plainly
that a person signing in to the unmodified binary with their own plan, including
where a platform hosts Claude Code, is fine. The
[Agent SDK overview](https://code.claude.com/docs/en/agent-sdk/overview) keeps
"Unless previously approved, Anthropic does not allow third party developers
to offer claude.ai login or rate limits for their products." Amp's settings link
may be under such an approval; nothing public says.

**Our decision.**
- Sanctioned and on by default: sign-in inside the person's computer or
  environment, through `claude` itself.
- Not covered without approval: our server keeping a `claude setup-token`
  value (#11204). It stays limited to an allowlist (the owner's account first)
  until the owner gets Anthropic's written approval (#11235, and the workspace
  `NEEDS_OWNER.md` entry).
- Always allowed: the person's own API key, including one billing their Max or
  Team plan's monthly API credits, and their own Bedrock, Vertex, or Foundry
  credential.
- Never: plan tokens in the gateway, or a person's Claude usage paid for, resold,
  or marked up by us.

## Issues

| Issue | What | Board |
| --- | --- | --- |
| [#11233](https://github.com/OpenAgentsInc/openagents/issues/11233) | Plan usage from Claude Code itself, shown everywhere | V1 (project 22) |
| [#11234](https://github.com/OpenAgentsInc/openagents/issues/11234) | Pick Claude Code, then connect, inline; local failure cards | V1 (project 22) |
| [#11235](https://github.com/OpenAgentsInc/openagents/issues/11235) | Settings → Claude: sign-in first, token allowlist, API-credits path | V1 (project 22) |
| [#11236](https://github.com/OpenAgentsInc/openagents/issues/11236) | Continue on your key or with OpenAgents when the plan runs out | later |
| [#11237](https://github.com/OpenAgentsInc/openagents/issues/11237) | Local one-at-a-time on a plan login, person-set | later |
| [#11225](https://github.com/OpenAgentsInc/openagents/issues/11225) | Decisions on our Pylons (acceptance line added) | existing |
