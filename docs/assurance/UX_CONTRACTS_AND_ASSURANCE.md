# UX Rules, Behavior Contracts, and AssuranceSpec

Status: adopted clarification (owner-directive, 2026-07-13).

This note answers one recurring question: **where do owner-stated UX,
micro-interaction, and visual do-and-do not rules live, and how do they relate
to the assurance pieces?**

Owner statements that motivated it (verbatim):

> "I want the ones that we have for that for this app to include micro
> interactions and things that I do and do not want to see. There are some
> things I do not want to see, such as long streams of text where icons should
> be."

> "I want all that enforced in the assurance pieces. I want to be able to
> specify rules there. For example, I do not want to see certain things like
> strings where icons should be, certain fonts, and that must be specified"

## 1. The rule of homes

1. **The rule text lives in the owning app's behavior-contract registry.**
   For OpenAgents Desktop that is
   `apps/openagents-desktop/src/contracts/ux-contracts.ts` (schema:
   `packages/behavior-contracts`). The owner's statement is recorded
   **verbatim**, with source channel and date, in the same change that lands
   the rule — never conversation-only (house law, 2026-07-03).
2. **Every registered rule ships with a real oracle in the normal test
   sweep.** An `enforced` contract names at least one `bun-test` oracle that
   actually fails on violation. If the oracle cannot land yet, the contract
   enters `pending` with explicit `blockerRefs` — never a vacuous green test.
3. **AssuranceSpec references, it never duplicates.** Per
   `ASSURANCE_SPEC.md` §5, an assurance obligation cites the rule by
   `contract_refs: ["<contractId>"]` and adds what the registry cannot:
   environment-bound evidence with `technique: visual`, `accessibility`,
   `native`, or `device` — real pixels, real devices, real screen readers.
   The registry stays the single source of the rule text. AssuranceSpec adds
   proof design on top (the Law 14 spirit: it does not duplicate another
   system's ledger).

So "enforced in the assurance pieces" decomposes into two layers that never
collapse:

```text
owner statement (verbatim)
  └─ behavior-contract registry entry          ← single source of rule text
       ├─ bun-test oracle in the test sweep    ← immediate, every push
       └─ AssuranceSpec obligation
            contract_refs: [<contractId>]
            technique: visual | accessibility | native
            environment_refs: [...]            ← pixel/device evidence, later
```

## 2. Worked example (2026-07-13)

Registry version `2026-07-13.5` of the Desktop registry added three contracts:

| Contract | What it binds |
| --- | --- |
| `openagents_desktop.microinteraction.owner_review_register.v1` | The register: every future owner micro-interaction do/do not rule lands as its own versioned contract here. Its oracle fails if a registered rule is removed, downgraded, or undocumented. |
| `openagents_desktop.microinteraction.icon_slot_no_raw_text.v1` | Icon slots (dock items, icon-only action controls, status glyphs) render closed-catalog glyphs, never long raw text. Dock labels are bounded single-line micro-copy. |
| `openagents_desktop.typography.approved_fonts_only.v1` | Only the owner-selected preset stacks (Oxanium body/UI, Geist headings, system body fallbacks, and generic `monospace` for code) may be declared anywhere under `apps/openagents-desktop/src`. |

All three are enforced by
`apps/openagents-desktop/tests/owner-ux-rules.test.ts`, which runs in the
normal desktop sweep and — per assurance design law 4 (oracle sensitivity) —
includes falsifier fixtures proving each validator rejects a known-bad input
(a rogue `Comic Sans MS` declaration, an unknown glyph, a long-stream dock
label).

**Honest enforcement boundary.** The icon-slot oracle proves the strongest
mechanically expressible subset on the real typed view trees: closed-catalog
glyph resolution, glyph-plus-accessible-label IconButtons, and bounded dock
labels. The fully general "no long text ever appears where an icon was
designed" claim over arbitrary rendered pixels is not expressible on typed
trees. That residual is exactly what an AssuranceSpec obligation with
`technique: visual` citing
`openagents_desktop.microinteraction.icon_slot_no_raw_text.v1` will carry when
the MVP AssuranceSpec obligations are designed. The contract's
`authorityBoundary` states this residual explicitly rather than rounding up.

## 3. How an agent adds the next rule

When the owner states the next "I do not want to see X" (or "I want to see Y")
for Desktop:

1. **Registry entry, same change.** Append a new contract to
   `apps/openagents-desktop/src/contracts/ux-contracts.ts` — statement
   verbatim, `source` recorded, `contractId` as
   `openagents_desktop.<area>.<slug>.v1`. Never renumber or remove existing
   contracts. Supersede with `.v2` and retire the old one. **Bump the
   registry `version`** (`YYYY-MM-DD.N`).
2. **Real oracle, same change.** Add the enforcing test (extend
   `tests/owner-ux-rules.test.ts` or a closer suite). It must fail on
   violation and should include a falsifier fixture. `state: "enforced"` only
   when the oracle really runs in the sweep (`enforcementTier: "test-sweep"`).
   otherwise `state: "pending"` with `blockerRefs`.
3. **Cross-link the register.** Add a `contract:<id>` evidence ref to
   `openagents_desktop.microinteraction.owner_review_register.v1` if the rule
   is a micro-interaction/visual do-or-do not, and list the new id in this
   doc's worked-example table when it changes the picture.
4. **Defer pixels honestly.** If part of the rule needs rendered-pixel or
   device evidence, state the enforced subset and the residual in the
   contract's `authorityBoundary`/`verification`, and leave the residual to an
   AssuranceSpec obligation citing the contractId — do not weaken the oracle
   or claim visual proof the sweep does not produce.

Registry-schema truth (states, tiers, oracle kinds) lives in
`packages/behavior-contracts/src/contract.ts`. Mechanical registry rules in
`registry.ts`. The oracle-file linkage discipline in `coverage.ts`.
