# Owned natural-sleep condition

Base: `e4c0f5687f`. Source: pinned SRD 5.2.1 p185 (Long Rest), p191
(Unconscious); PDF identity is in `docs/dnd/long-rest-implementation.md`.

`rs_sleep_begin` retains an event-owned `EF_REST_SLEEP` record before applying
any lifecycle consequences. Full storage, immunity to Unconscious, invalid
actors, zero HP/death and borrowed Reaction frames refuse without evicting
records, dropping the represented held focus or ending concentration. Repeated
sleep is idempotent. Actual admission uses the existing incapacitation lifecycle
and blocks actions, effective movement and Reactions through shared conditions.

The record does not expire on a clock tick or receive a spell save. It records
only a preexisting *unrecorded* Unconscious source in its otherwise private
`cond` payload. Retirement preserves that raw source, other recorded sources
and zero-HP ownership. An independently expiring spell source does not become
an immortal raw source. Waking leaves Prone; admission respects immunity to
Prone. The represented held focus is dropped; a worn focus remains worn.

## Verification

- Composed `long-rest-resources-checks.coax --library`: 268 checks pass.
- Scoped mutation through that entry: sleep 56/56 killed; effect storage
  127/127 killed; clock ownership 40/40 killed. Zero survivors/exemptions in all
  three changed authority units. The storage/clock gates passed before the
  final sleep-only sharpening; their product files were unchanged afterward.
- `coax/web/tool.sh aethyr/rules/srd/rest-sleep-native.coax`:
  `PASS REST-SLEEP (72 class/level scenarios)`. All twelve classes at levels1–6
  sleep through a 30,000-second actual tick, lose concentration/held focus,
  retain Unconscious through healing from zero while still sleeping, and wake
  Prone without retaining the retired sleep condition.
- `coax/web/tool.sh tools-check`: 1897 tracked tools, 35 vendored, no missing
  reasons. `git diff --check` passes.

Source SHA256:

- sleep: `7fde9f332d77fc50adde1cabc60007f8320d5965573cce02a2d5a70855d7741d`
- storage: `bff02b52bf8471e7dfa52d3bf313db864e8611dc47bd4d15f80362e4e2823841`
- clock ownership: `b0a57296f080175c232c5e779eb834ae70f45d4ddffb8dd03fd438946558764a`

The existing compiler source-version advisory remains; no bootstrap was needed.

## Integration still required

These primitives are composed into the battle engine but are not yet called by
world rest controls or by the retained Long Rest journal. The world must own
sleep transitions and synchronize its prior activity before waking or applying
an interruption. No automatic damage wake was added ahead of that causal
ordering work. Clock conversion/scheduling, full recovery benefits, controls and
rejoin persistence remain necessary. General held-weapon/inventory dropping is
not represented by this helper; only the existing held-focus state is updated.
This is not a claim that all Unconscious/world inventory behavior is complete.
#13040 remains open.

The full checked Lab Wasm composition also passes:

```
coax/web/wasm.sh apps/openagents-web/fivelab.coax apps/openagents-web/fivelab-face.c /tmp/srd-rest-sleep.wasm apps/openagents-web/everglade-step.coax aethyr/sticks.coax
```

38.6 seconds, 2,539,493 bytes. SHA256:
`f299b4b84b56774bbdcb1e1df5cf8d32f42627b6844f4faddb3ebe59f13ec21d`.
Existing near-limit traversal/value-slot advisories were reported; no work
budget was raised. The temporary artifact name has no shipping size budget;
no product budget was added for it. Native entry SHA256:
`277992c97d32883c451a4a91b07a0d6edc48316896f291fa6b3a8c4b34a2bb00`.
