# Component admission and grant provenance

Contract: [casting-components.md](../../dnd/casting-components.md). Source:
SRD 5.2.1 pp.105–106, PDF SHA-256
`8974902d109d6e63672d7c490bde9ccf052410503d9cfa768237154fbc5e3d87`.

Checks preceded implementation: the first gagged-caster fixture was refused
because the required speech state did not exist (`red.log`). The independently
imported component facts now reach the compact runtime index and casting policy.
No source record count, class-list interpretation or source pin changed.

## Evidence

- Component entry: 201 checks pass; canonical source entry: 200 pass, including
  V/S/M/price/consumption equality for every one of the 339 records.
- Component authority: 143 mutants killed, zero survivors; Reaction component
  transaction: 12 killed, zero survivors; class access/grant authority: 208
  killed, zero survivors. No equivalent-mutant exemptions. Missing worn-focus,
  zero/empty stock, source-zero and 64-bit word-boundary cases were added.
  Shared material lookup and actor-indexed payment remove duplicate search and
  redundant copied-state bookkeeping. Inline composition checks added after
  the component gate do not change its procedures; that passing gate is reused.
- Saved-seat/hotbar: 181 checks pass, including readiness, use and empty-target
  refusal without spending. Composed spells entry: 48 pass with authority
  execution coverage intact. Ordinary probes: 175 pass.
- Experimental: 21 selected scenarios pass. Two original checks each allocated
  three independent Lab states and exceeded the compiler's aggregate limit
  after the actor state grew. They were split by scenario, retaining every
  assertion, with no raised limit. The 21 scenarios replace the previous 17.
  Peak scalar usage is 26,369,685/33,554,432. The room Command fixture passes.
- Native CAST-COMPONENTS passes: gagged refusal preserves slots/events/dice;
  a focus/pouch does not replace Bless's priced Symbol; reusable stock remains;
  V-only Healing Word works with full hands; Shield needs its S hand and refuses
  before Reaction/slot spending. Strict suite: 17 PASS, one parked Polymorph
  FAIL, exit 1 retained. This is not full-SRD qualification.
- Regeneration matches the pinned canonical projection; tools-check is retained
  in its log. Source/compiler hashes accompany the evidence.

Rebased over independent condition/save commits `399da4abbc` and `127e8032b4`.
Combined checks retain 201 component and 200 source checks; hotbar now passes
183 checks including the incoming regressions. The dedicated incoming save and
condition entries pass 171 and 173 checks. The combined native witness still
has only parked Polymorph failing, and selected Experimental/room checks pass.
Both ledger entries and all incoming logic are retained. The three owned
authority files are unchanged by rebase, so their passing mutation gates are
reused. `*-pre-rebase.log` files remain as historical evidence; unprefixed logs
record the final integration. The unchanged composed-spell check is reused.

Revivify's canonical consumed requirement is tested against material payment,
not an implemented resurrection spell. No current consuming spell's complete
execution is thereby qualified. Missing stock/capacity refuses before casting
costs; exact receipt creation and item valuation are a separate remaining
equipment boundary, not proven by these counters.

The cached 0.52.0 compiler is hashed. All commands completed locally within
fifteen minutes. Failed initial authority runs are not counted as passing;
all survivors were addressed through code/fixtures. No browser or audio opened.

Commands:

```sh
coax/bin/coax test aethyr/rules/srd/component-checks.coax --library
coax/bin/coax mutate aethyr/rules/srd/component-checks.coax --library --unit aethyr/rules/srd/cast-components.coax --jobs 4
coax/bin/coax mutate aethyr/rules/srd/component-checks.coax --library --unit aethyr/rules/srd/component-reaction.coax --jobs 4
coax/bin/coax mutate aethyr/rules/srd/component-checks.coax --library --unit aethyr/rules/srd/spell-access.coax --jobs 4
coax/bin/coax test aethyr/rules/srd/spell-access-source-checks.coax --library
coax/bin/coax test apps/openagents-web/fivebar-test.coax --library
coax/bin/coax test aethyr/rules/srd/spells.coax --library
coax/bin/coax test aethyr/rules/srd/compliance-probes.coax --library
coax/bin/coax test apps/openagents-web/fivelab.coax --library --only 'experimental combat'
coax/bin/coax test apps/openagents-web/everglade5e-room-rules.coax --library --only 'Cult Priests'
coax/web/tool.sh aethyr/rules/srd/spell-index-build.coax --check aethyr/rules/srd/data/spell-index.coax
coax/web/tool.sh aethyr/rules/srd/compliance-check.coax
coax/web/tool.sh tools-check
```

#13038, #13044, #13045 and #13046 remain open for their full narrowed acceptance.
The casting contract lists the actual equipment/grip/receipt, feature-waiver,
target-dependent material, workflow and world integration gaps. No owner action
is required.

This work includes material from the System Reference Document 5.2.1 ("SRD 5.2.1") by Wizards of the Coast LLC, available at https://www.dndbeyond.com/srd. The SRD 5.2.1 is licensed under the Creative Commons Attribution 4.0 International License, available at https://creativecommons.org/licenses/by/4.0/legalcode.
