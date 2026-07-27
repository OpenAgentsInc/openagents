# Exo as a selectable Omega executor — what works, what does not, and what a person must do — 2026-07-26

Why Exo was missing from Omega's executor menu, what was changed, and exactly
where the line between working and not working falls. Everything below was
measured on this machine on 2026-07-26, not inferred from source.

This document is evidence and a runbook. It is not dispatch, deploy, release,
spend, or public-claim authority. The strategic companion is
[the integration analysis](./2026-07-26-exo-openagents-integration-analysis.md).
The two teardowns it cites remain the reference for what Exo is.

Sources and pins, all read live:

| Thing | State when measured |
| --- | --- |
| Omega | measured at `origin/main` `2db23c3e26`, and the change landed as `2dee4b136f` |
| Exo checkout | `~/work/exo`, `OpenAgentsInc/exo`, detached at `cd7c0d2` |
| Exo binary | `~/work/exo/target/release/exo`, built 2026-07-26 03:33, version `0.1.0` |
| Live `exo serve` | `127.0.0.1:4766`, root `/tmp/exo-serve-copy/.exo`, agent `zerobase`, model binding `or-free` |

There are two unrelated projects called exo. This is `exoharness/exo` — the
agent harness — via the `OpenAgentsInc/exo` fork, never exo labs'
`exo-explore/exo` cluster-inference appliance. Omega's lane refuses the wrong
one by remote URL, which is the omega#86 lesson made executable.

## 1. Why Exo was not an available executor

`crates/agent_ui/src/omega_executor_selector.rs` offers one of four names —
Omega, Exo, Codex, Claude — only when that name can actually run a turn.
`ready()` gates Exo on `ExoLaneConfig::resolve`, which is the owner's lane file
if one exists and otherwise
`omega_agent_detect::exo::derive_lane_from_env()`.

On this machine that derivation returns `Err(NoStateRoot)`. Verified directly:

| Candidate the derivation tries, in order | Present |
| --- | --- |
| `$OMEGA_EXO_ROOT` | unset |
| `<working directory>/.exo` | not applicable — a Dock or Finder launch gets `/`, which `chosen_working_directory` refuses |
| `~/work/exo/.exo` (the checkout) | **absent** |
| the root named by `$OMEGA_EXO_LANE_FILE` | unset |
| the lane file `~/Library/Application Support/Omega*/openagents/omega-exo-lane.json` | **absent** in every Omega profile on this machine |

The checkout is there and the binary is built — the two upstream steps
succeed — so the failure is precisely and only that **Exo has been installed
here and never run**. There is no state root, therefore no agent, therefore
no conversation, therefore nothing for `exo acp` to attach to.

The name was withheld correctly. What was wrong is that it was withheld
*silently*: the menu read `Omega  Codex  Claude` and said nothing about the
fourth name, so "not configured" and "does not exist" looked identical.

## 2. The judgement, and the option that was rejected

Two ways to make the name appear were considered. Both were tested before
either was chosen.

**Rejected: offer Exo whenever the binary is present, and derive the lane
lazily at connect.** This fails twice, and the second way is worse than the
first.

1. `exo acp` takes an *existing* agent and an *existing* conversation as
   positional arguments. On a machine with a binary and an empty root there is
   nothing to attach to at all.
2. The failure does not land at connect. Measured against a real root with a
   file-backed secret store and no secret environment: `initialize` returns
   agent capabilities and `session/new` returns a session id — both succeed —
   and then `session/prompt` returns
   `{"code":-32603,"message":"Internal error","data":"failed to decrypt secret payload"}`.
   A name offered on binary-presence would therefore fail **after** the person
   had typed and sent their first message.

That is the exact defect `OMEGA-DELTA-0115` built the ready filter to prevent,
arrived at from a new direction. A name that appears and then errors is worse
than a name that is absent with a reason.

**Rejected: Omega creates the state root.** `OMEGA-DELTA-0107` forbids Omega
from starting `exo serve`. Creating a root is a different act, and it was
weighed separately rather than assumed to fall under that rule. It is still
refused, for three reasons of its own:

- `.exo` is single-writer storage. Omega cannot prove no `exo serve` already
  holds the root it would write into, and a second writer to a live root
  corrupts a history rather than failing loudly.
- A root alone resolves nothing. `agent_slug` refuses an empty root, so
  creating a root means creating an **agent**, and an agent that can answer
  anything carries a model binding and a provider secret. That is the owner's
  money and the owner's credentials. `agent_slug`'s refusal already says
  "Omega does not create one" in as many words.
- `OMEGA-DELTA-0107` settled the neighbouring question — Omega reads a durable
  log from a server the owner runs and starts none. Writing into that server's
  storage is the same claim of authority through a different door.

**Chosen: keep the gate exactly as it is, and make the absence legible.**
`ready()` is untouched and nothing new became clickable. `unavailable()` was
added as its exact complement, so every one of the four names is either offered or
explained. `OMEGA-DELTA-0123` is the record.

## 3. What changed in Omega

`OMEGA-DELTA-0123`, in `omega` at `origin/main` `2dee4b136f`.

- **The menu now names what it cannot run.** Under a separator, below the
  offered names, each unavailable executor appears as a disabled entry carrying
  a short reason: `Exo — Exo has never been run here`, `Codex — not installed`.
- **The reason is the type's, not the menu's.**
  `ExoLaneUnderivable::summary()` gives each of the eight typed refusals its own
  sentence, one per variant:

  | Refusal | What the menu says |
  | --- | --- |
  | `NoCheckout` | `Exo is not installed` |
  | `NotTheExoOmegaDrives` | `a different Exo is installed` |
  | `NotBuilt` | `Exo is installed and not built` |
  | `NoStateRoot` | `Exo has never been run here` |
  | `NoAgent` | `your Exo has no agent` |
  | `SeveralAgents` | `your Exo has several agents; name one` |
  | `NoConversation` | `that Exo agent has no conversation` |
  | `SeveralConversations` | `several conversations; name one` |

  That enum's own documentation had argued for exactly this and its only caller
  discarded the value. It was built to be read and had no reader. The full
  refusal, with every path it looked at, still goes to the log.
- **The reason lives in the label, not in a hover aside.** `ContextMenu`
  registers a documentation aside only for a *selectable* item, and an entry is
  selectable exactly when it is not disabled — so an aside on a disabled entry
  never appears, and the `Info` icon the component draws beside one has nothing
  behind it. A delta check asserts that upstream fact so a rebase that changes
  it is noticed.
- **One answer to "is there a lane".** `exo_lane_resolves()` is now the absence
  of an absence rather than a second cached read of the same files.

Enforced by five checks in `crates/omega_deltas` and four in `crates/agent_ui`.
Each was watched failing against a mutation of the thing it guards — sixteen
mutations, all caught.

**Three of the nine checks were vacuous when first written, and only mutation
found it.** This is worth recording because all three looked correct:

- One looked for `choice.name()` in the menu builder. The *offered* loop three
  lines above renders `choice.name()` too, so the check stayed green with the
  reason removed from the disabled label entirely.
- One looked for each refusal's variant name inside `summary`. A name survives
  being folded into another arm — `Self::NoStateRoot { .. } | Self::NoAgent
  { .. } => "…"` leaves both names in the file — so the check passed on exactly
  the edit it existed to refuse. It now requires an arm per variant *and*
  counts the arms.
- One claimed to hold "not installed" and "installed and undrivable" apart. That
  second case cannot occur: `ready` offers a name only when it is detected
  **and** drivable, and `DRIVABLE_AGENT_IDS` names both Codex and Claude, so
  for those two the arm is unreachable. Collapsing the two arms passed. The arm
  is kept — it is the truthful answer if `DRIVABLE_AGENT_IDS` ever stops naming
  one of the four — and the check now asserts that reachability rather than
  pretending to exercise it.

## 4. What works — measured, not asserted

`exo acp` drives a complete turn from Omega's side of the wire. Driven exactly
as `AcpConnection::stdio` drives it — `initialize`, `session/new`,
`session/prompt` — against agent `zerobase`:

| Configuration | Result |
| --- | --- |
| Through the live `exo serve` (`EXO_EXOHARNESS_URL=http://127.0.0.1:4766`), with secret env | `stopReason: end_turn`, streamed `agent_message_chunk` deltas, answer returned verbatim |
| Through the live `exo serve`, **without** `EXO_SECRET_BACKEND` or `EXO_MASTER_KEY_PATH` | works — the server holds the secrets, so the ACP client needs neither |
| Against a root on disk, with `EXO_SECRET_BACKEND=file` and `EXO_MASTER_KEY_PATH` | `stopReason: end_turn`, answer returned |
| Against a root on disk, **without** that environment | `initialize` ok, `session/new` ok, `session/prompt` → `failed to decrypt secret payload` |

Turn latency was 1.3–5.0 seconds against the `or-free` binding. The reply
arrives as streamed text chunks and the completion carries
`exo.session_id`, `exo.turn_id` and `exo.latest_event_id` in `_meta`, which is
what Omega's disclosure and durable-log surfaces read.

So the transport, the protocol, the streaming and the turn are all working.
**Given a lane, Omega can run a turn through Exo and get the answer back.**

## 5. What does not work

**Exo is still not selectable on this machine.** No lane resolves here, and
this change did not create one. The menu now says `Exo — Exo has never been run
here` where there was blank space. That is the whole of what changed for a
person looking at it.

**The lane file carries no secret-store fields.** `connect_configured_lane`
builds `AgentServerCommand { env: None }`, so the `exo acp` child inherits
whatever environment Omega was launched with. A shell launch after
`export EXO_SECRET_BACKEND=file EXO_MASTER_KEY_PATH=…` works. A Dock or Finder
launch has neither variable and, on the on-disk path with a file-backed store,
the first message fails with `failed to decrypt secret payload` — an error that
reads like a corrupt state root and is not.

This gap is narrower than it looks. Exo's default secret backend is
`apple-keychain`, and a root whose secrets were written that way needs no
environment at all. The file backend is what headless setup produces, which is
what every fixture on this machine uses. Repairing it properly means adding
secret-store fields to the `openagents.omega.exo_lane.v1` schema, which both
`ExoLaneConfig` and `omega_agent_detect::exo` read. That is a format change and
it was deliberately not made here.

**No window was opened.** Nothing in this work started Omega, so the rendered
menu is unproved: that the separator and the disabled entries draw, that the
label fits the popover width, and that the owner now sees a reason where there
was nothing. Those are acceptance items and they stay open.

**Untouched by all of this:** tool calls, mounts, sandbox providers, the Tier C
self-modification grant path, and the durable-log reader. They were working
before and are not on this change's path.

## 6. What a person must do to make Exo selectable

Omega will not do any of this, by the decision in section 2. All of it is one
time.

**If you have never run Exo.** Create a state root, an agent, and a
conversation. From a directory you choose to keep the root in:

```
exo agent create --slug <agent> "<name>"
exo model register …            # a binding and a provider secret
exo conversation create --slug <conversation> <agent> "<name>"
```

Omega finds `<that directory>/.exo` when Omega is started from it, or wherever
`OMEGA_EXO_ROOT` names.

**If you already have a root elsewhere.** Set `OMEGA_EXO_ROOT` to it, or write
the lane file directly at
`~/Library/Application Support/Omega/openagents/omega-exo-lane.json`:

```json
{
  "schema": "openagents.omega.exo_lane.v1",
  "binary": "/Users/<you>/work/exo/target/release/exo",
  "checkout": "/Users/<you>/work/exo",
  "root": "/Users/<you>/<somewhere>/.exo",
  "agent": "<agent-slug>",
  "conversation": "<conversation-slug>"
}
```

A lane file that exists is the answer even when it is broken — Omega does not
fall through to derivation there, on purpose, because replacing somebody's
explicit configuration with a guess about a different `.exo` is the
`OMEGA-DELTA-0042` failure. The menu says `Exo — its lane file cannot be read`
in that case.

**If your secrets are file-backed**, launch Omega from a shell that exports
`EXO_SECRET_BACKEND=file` and `EXO_MASTER_KEY_PATH`, until the schema carries
them. `EXO_SECRET_BACKEND=file` must be set *alongside* the key path or Exo
dies with the decryption error above.

**Do not point a lane at a root another process holds.** `.exo` is
single-writer. If an `exo serve` is running on that root, either point Omega at
the same server with `EXO_EXOHARNESS_URL` — which also removes the secret-env
requirement — or use a different root.

## 7. Open, in order

1. Open a window and look at the menu. Everything in section 3 is unproved
   against pixels.
2. Decide whether `openagents.omega.exo_lane.v1` should carry the secret
   backend and master-key path. Until it does, a Dock-launched Omega cannot
   drive a file-backed root.
3. Decide whether the menu should offer the person the act it is describing —
   a "set up Exo" affordance — rather than only naming what is missing. That is
   a product question, not a correctness one, and nothing here forecloses it.
