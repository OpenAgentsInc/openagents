# 49. Adoption is policy

> **Status: unimplemented — design draft.** This chapter is a plan for the
> owner to edit. It ships no code yet, and paths marked planned do not exist.

## Premise

A validated claim is still only evidence. This chapter makes the decision that
turns it into a default: a written policy reads the records, and an operator
signs an adoption that anyone can audit.

## Draft

Reports are evidence. Claims summarize effects. Adoption is policy. The three
are kept apart so that no one can confuse "it tested well" with "everyone now
runs it".

An adoption is a signed record. It cites the reports, the confirming checks
and at least one validation. It may also cite a marginal report, a regression
check, a reliability record, the change in authority the member needs, and
what is at stake if it fails.

The marginal report is the one people forget. A plugin is measured against the
agent as it is today, with all current defaults in place, and not against the
agent it was first tested with. A plugin that helped a bare agent may add
nothing to one that already has three similar plugins, and it may get in their
way. A member that makes the default set worse when combined with it is
rejected, however well it did alone.

Safety is the other exit. A member that the evidence shows to be unsafe is
rejected, however useful it is. Usefulness can't buy back safety.

Adoption is an operator's decision. We sign adoptions for the defaults we
ship. Someone who runs their own instance can read the same public evidence
and decide differently. A run in progress keeps the lock it started with.

The first adoption was made on 2026-09-29, into the defaults every Coder used.

## What we build

A plan for what this chapter ships once the draft is approved:

- A written adoption policy: what evidence a default needs.
- An adoption record that cites its evidence, and a defaults release that
  cites the adoption.
- A marginal run: the subject against today's defaults, with a regression
  check.

It builds on what exists today:

- [`coax/lib/nostr/eval`](../../coax/lib/nostr/eval)
- [`coax/lib/nostr/pol`](../../coax/lib/nostr/pol)
- [`coax/lib/nostr/ext`](../../coax/lib/nostr/ext)

On the protocol ([chapter 65](ch065-one-schema-three-carriages.md)):

| Record | Schema | Carriage | Today |
| --- | --- | --- | --- |
| Adoption | `openagents.policy.v1`, which holds adoption authority ([spec 19.2](../../docs/protocol/openagents-protocol-spec.md#192-openagentspolicyv1)) | Nostr (NIP-POL) | Spec sketch; no `.proto` file |
| The defaults set | `openagents.extension.v1` `ComponentSet`, `SetSlot` ([spec 20.2](../../docs/protocol/openagents-protocol-spec.md#202-openagentsextensionv1)) | Inside a release event (chapter 54) | Spec sketch; no `.proto` file |

### The chapter's check

- **Source:** the adoption policy and record in `openagents/eval/adopt.coax`,
  and a defaults release in `openagents/registry/defaults/` that cites it.
- **Trace:** fixture reports, three confirming checks and one validation go
  in; the marginal run against today's defaults uses recorded answers. The
  record prints its citations and the verdict `admit`, and the defaults
  release names the adoption's id.
- **Refusal:** an adoption that cites no validation is refused, as the
  existing reader refuses one today with `EA_VALIDATION`. A subject whose
  marginal run makes the default set worse is rejected, however well it did
  alone.
- **Independent check:** the admission reader in
  [`coax/lib/nostr/eval/report.coax`](../../coax/lib/nostr/eval/report.coax),
  written before this chapter, reads the same record and agrees.

Planned paths: `openagents/eval/adopt.coax`, `openagents/registry/defaults/`.

## Sources

- **Episodes:**
  - [Episode 289, *OpenAgents*](../../docs/transcripts/289.md): 2:00, "If it's good, it gets added"
  - [Episode 234, *Product Promises*](../../docs/transcripts/234.md): a registry of what is live
  - [Episode 248, *Predictable Software*](../../docs/transcripts/248.md)
- **Old code at `coder-v1.0.0`, crates:** `ext-eval`, `gym`, `eval-runner`
- **NIPs:** [NIP-EVAL](../../nostr/nips/openagents/NIP-EVAL.md), [NIP-POL](../../nostr/nips/openagents/NIP-POL.md), [NIP-EXT](../../nostr/nips/openagents/NIP-EXT.md)
- **Also:**
  - *Test-Time Capabilities* (`git show coder-v1.0.0:docs/essays/2026-09-29-test-time-capabilities.md`), "Capability adoption"
  - `packages/coder-defaults` at `coder-v1.0.0`
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P10, P15

## Open questions for the owner

1. What does the policy require before something becomes a default for
   everyone: how many confirmations, how many validations, and from whom?
2. Is there one default set for the whole agent, or one per member, as Coder
   had?
