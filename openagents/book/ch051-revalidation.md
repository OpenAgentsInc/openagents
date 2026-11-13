# 51. Revalidation

> **Status: unimplemented — design draft.** This chapter is a plan for the
> owner to edit. It ships no code yet, and paths marked planned do not exist.

## Premise

An adopted member rests on a claim, and a claim rests on its key. This chapter
watches the key: when any part of it changes, the claim is checked again, and
a claim that no longer holds is pulled.

## Draft

Adoption is not the end of a member's life. The agent it was measured with
changes every week. Models are replaced. Defaults grow. Environments are
updated.

The essay's rule is short. When the subject, the baseline, the environment,
the grant or the measurement changes, revalidate: run the claim again on the
new key. When the policy changes, reinterpret: read the old records under the
new rule. And when a claim stops holding, quarantine the member or revoke its
adoption. A member is not kept on the strength of a claim that no longer
describes it.

A model change is the common case and the hardest to see. A hosted model can
be swapped behind its name. That is why chapter 46's key names what could not
be pinned, and why this chapter runs a small canary set on a schedule to
notice a change nobody announced.

Revalidation costs compute, so it needs an order. Members with the most use
and the most at stake go first. Each default shows when its evidence was last
run, so a reader can tell fresh evidence from old.

Pulling a default is as public as adopting it. The revocation is a signed
record that cites what changed and the run that showed it, and any credit that
rested on withdrawn evidence can be revoked too.

This closes the loop the essay draws. A new default agent meets harder tasks,
they expose new failures, and those suggest new candidates.

## What we build

A plan for what this chapter ships once the draft is approved:

- A watcher that notices when part of an adopted claim's key has changed.
- A scheduled canary run for parts that can't be pinned, such as hosted
  models.
- Quarantine and revocation records, and the date of last evidence on every
  default.

It builds on what exists today:

- [`coax/lib/nostr/eval`](../../coax/lib/nostr/eval)
- [`coax/lib/nostr/pol`](../../coax/lib/nostr/pol)
- [`coax/lib/gateway/usage-observation-checks.coax`](../../coax/lib/gateway/usage-observation-checks.coax)
- [`coax/lib/clock.coax`](../../coax/lib/clock.coax)

On the protocol ([chapter 65](ch065-one-schema-three-carriages.md)):

| Record | Schema | Carriage | Today |
| --- | --- | --- | --- |
| Quarantine and revocation | `openagents.extension.v1` `Revocation`, `Checkpoint` ([spec 20.2](../../docs/protocol/openagents-protocol-spec.md#202-openagentsextensionv1)) | Nostr kinds 3185 and 30185, public (NIP-EXT) | Spec sketch; no `.proto` file |

### The chapter's check

- **Source:** the watcher and canary schedule in
  `openagents/eval/revalidate.coax`.
- **Trace:** an adopted fixture claim; the fake model service changes its
  recorded answers behind the same name. After `clock advance` past the
  canary's interval, the canary runs, the watcher prints which key part
  changed, and the default's last-evidence date updates.
- **Refusal:** when the canary's verdict no longer holds, the member is
  quarantined and a signed revocation cites the change and the run. A
  default whose evidence is older than the policy allows is shown as stale.
- **Independent check:** the expected verdicts for both sets of recorded
  answers are written by hand, and the revocation reads through the existing
  EXT reader,
  [`coax/lib/nostr/ext/install.coax`](../../coax/lib/nostr/ext/install.coax),
  which then refuses the release with `EXT_REVOKED`.

Planned paths: `openagents/eval/revalidate.coax`.

## Sources

- **Episodes:**
  - [Episode 204, *DO NOT BREAK USERSPACE*](../../docs/transcripts/204.md): a provider changes the ground under you
  - [Episode 257, *Cursor Fails to Open a File*](../../docs/transcripts/257.md): a tool that stopped working
  - [Episode 258, *ChatGPT Codex Keeps Crashing*](../../docs/transcripts/258.md): a product that kept crashing
  - [Episode 234, *Product Promises*](../../docs/transcripts/234.md): withdrawn claims stay visible
- **Old code at `coder-v1.0.0`, crates:** `eval-runner`, `ext-eval`, `gym`
- **NIPs:** [NIP-EVAL](../../nostr/nips/openagents/NIP-EVAL.md), [NIP-POL](../../nostr/nips/openagents/NIP-POL.md), [NIP-XP](../../nostr/nips/openagents/NIP-XP.md)
- **Also:**
  - *Test-Time Capabilities* (`git show coder-v1.0.0:docs/essays/2026-09-29-test-time-capabilities.md`), the lifecycle figure: monitoring and revalidation
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P15

## Open questions for the owner

1. How much compute do we set aside each month for re-running old claims,
   inside the cloud budget?
