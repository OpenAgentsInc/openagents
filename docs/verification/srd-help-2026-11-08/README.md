# Help: checks, attacks and actual turn expiry

2026-11-08, baseline `79a2ce92b3`, final integration base `75ce46daf0`.
Partial #13039/#13040 work; neither issue is
complete. The containing commit and [hashes](source-hashes.txt) identify the
verified source/compiler. No owner action is required.

## Source and behavior

Pinned [SRD 5.2.1](https://media.dndbeyond.com/compendium-images/srd/5.2/SRD_CC_v5.2.1.pdf),
SHA-256 `8974902d109d6e63672d7c490bde9ccf052410503d9cfa768237154fbc5e3d87`,
pp.182–183 Help; pp.6–9 checks/proficiency/Advantage. The public Basic Rules
glossary was also consulted; the pinned SRD remains the acceptance source.

`help.coax` owns admission, retention, consumption and turn retirement:

- Ability assistance selects exactly one of the helper's actual eighteen skill
  or thirty-seven tool proficiencies, and one different ally. Jack or a display
  count cannot substitute for selected proficiency. Tools retain their variants.
  Assistance feasibility is an authoritative caller fact. The recipient gains
  Advantage, not proficiency or a second proficiency bonus.
- Attack assistance distracts a living enemy within one five-foot grid square,
  including diagonals. It names no particular attacker: another ally of the
  helper can consume it on its next attack against that enemy. That ally need
  not be adjacent; the helper can move away after distracting. The helper's own
  attack and an attack by someone outside its team cannot consume that record.
- Both Actions reject invalid logical/physical actors, self, wrong team/turn,
  incapacity, impossible assistance and full recipient storage before spending.
  Insertion precedes Action payment; accepted Help spends no Bonus Action, slot
  or die. Records retain the helper and selected skill/tool independently.
- Check assistance enters the existing Advantage fact before ordinary
  cancellation. The check resolver queries first and consumes only after request
  admission. Matching independent owners are all consumed on that check without
  stacking Advantage. Other skills, malformed requests and Passive Perception
  preserve a pending record. **Source interpretation:** an admitted sensory
  automatic failure is still the next ability check made, so it consumes Help
  while rolling/spending no die or rescue. Passive Perception is a score without
  making that check and neither benefits from nor consumes this next-check grant.
- A helper must still be capable of assisting the actual ability check.
  `RsAbilityRequest.help_blocked` lists helpers whose current task, position or
  senses make assistance impossible. Remaining capable helpers can contribute;
  all matching next-check benefits are then consumed. `RsFirstAidContext`
  forwards the same list to the actual Medicine check. The list grants nothing
  and is not a second Action. It is authoritative input, not a geometry oracle.
- Attack Help enters the common attack-edge resolver, including spell/ranged
  attacks and misses. Canceled edges still consume matching distractions.
  Refused casts and saving throws do not consume an attack benefit. The
  momentary distraction is already performed, unlike ongoing check assistance.
- `EF_HELP_CHECK` and `EF_HELP_ATTACK` use existing bounded effect storage but
  impose no condition bits. They are excluded from clock expiry. The helper's
  actual next logical turn retires only its unused Help records across all
  recipients. Other helpers and unrelated conditions/effects survive. The action
  timing profile therefore uses its declared logical turn, not a second timer.
- Saved-seat ability/attack Help uses the same policy and restores the other
  actor's full turn. This supplies APIs, not a new hotbar button, key, voice or
  wire command. Effect kinds and request spans change internal state/layout only;
  no protocol or frozen Character Create save key changes.

## Evidence

- [Red](red.log): an allied attack failed to receive/consume a retained
  distraction before implementation.
- [Core](core.log): 297 composed checks pass, including ability, HP, death-save,
  condition and Exhaustion suites. New cases cover all 55 proficiency choices,
  malformed/unknown choices, separate logical/physical bounds, recipient zero,
  full storage, every adjacent grid direction, multiple owners, current helper
  capability/exclusions, cancellation, first aid and actual turn retirement.
  Actual ranged misses and an actual Fire Bolt verify the shared attack path.
- [Help mutation](help-mutation.log): 165 killed, zero survivors.
  [Resolver](resolver-mutation.log): 87 killed.
  [Clock classification](clock-mutation.log): 37 killed.
  [First aid](firstaid-mutation.log): 112 killed. All have zero survivors and no
  exemptions. Initial Help survivors prompted physical-bound and recipient-zero
  retirement fixtures and a direct grid-square reach expression. The latter
  removes a 5-to-6-foot predicate mutation that whole-square coordinates cannot
  distinguish. No equivalent-mutant waiver was used. Resolver/clock/first-aid
  receipts precede the last Help reach refactor and additional fixtures; those
  three gated procedures/units and compiler are unchanged and their passing
  results are reused. Final core/native checks include the expanded fixtures.
- [Native](native.json), [summary](native.log): all 21 root scenarios pass in
  ordinary emitted checked C at O0/O2. The native runner does not count imported
  properties or establish sanitizer/browser qualification.
- [Hotbar](hotbar.log): 219 checks pass, including saved-seat spending, refusal
  and other-actor isolation. [Experimental](experimental.log): all 51 selected
  Experimental checks pass out of 753 on the final merged tree, including the
  incoming physical contact/health migration and real frame entry.
- No compiler or tooling source changed. The passing [tools-check receipt](../srd-stabilization-2026-11-08/tools.log)
  at `79a2ce92b3` is reused rather than repeating unchanged tool verification.

The first rebase retained the incoming contact/health migration and both
invariant/ledger records; all eleven original source/compiler hashes matched.
The subsequent Siege material/tactics change was also retained, including its
updated Wasm-build qualification statement. Its NPC constructor/world dependency
changes prompted fresh combined core/hotbar/native/Experimental checks. None of
the four gated policy units or compiler changed; their passing mutation receipts
are reused. The current hash list also records incoming world/dependency files
used by that final integration.

Tests have 180-second limits; mutation/native commands have 600-second limits:

```sh
coax/bin/coax test aethyr/rules/srd/help-checks.coax --library
coax/bin/coax mutate aethyr/rules/srd/help-checks.coax --library --unit aethyr/rules/srd/help.coax --jobs 4
coax/bin/coax mutate aethyr/rules/srd/help-checks.coax --library --unit aethyr/rules/srd/ability-checks.coax --jobs 4
coax/bin/coax mutate aethyr/rules/srd/help-checks.coax --library --unit aethyr/rules/srd/command-halt.coax --jobs 4
coax/bin/coax mutate aethyr/rules/srd/help-checks.coax --library --unit aethyr/rules/srd/stabilization.coax --jobs 4
python3 coax/tools/native_checks.py aethyr/rules/srd/help-checks.coax --out /tmp/oa-help-native-final-v2.json
coax/bin/coax test apps/openagents-web/fivebar-test.coax --library
coax/bin/coax test apps/openagents-web/fivelab.coax --library --only 'experimental'
```

## Remaining scope and caller duties

Task assistance feasibility, required senses, tools and per-helper exclusions
must come from the world/adjudicator when the check is made. Empty exclusions
mean no additional obstruction is reported; legacy open-grid callers do not
establish continuous-world occlusion, hearing or physical assistance. Attack
distraction admission also accepts the world's feasibility fact. Team/creature
indices must retain their encounter identities, backing storage/spans must stay
valid and logical turn events must be delivered by the host. Bare edge queries
are destructive roll boundaries, as for existing Vex/Guiding Bolt; they are not
UI previews. Full world controls and source geometry remain #13046 work.

Healer's Kit inventory/charges, knockout/Short Rest recovery, remaining condition
provenance/alternate senses, action/reaction windows and rests are not completed
by this change. The parent issues remain open.

This work includes material from the System Reference Document 5.2.1 ("SRD 5.2.1") by Wizards of the Coast LLC, available at https://www.dndbeyond.com/srd. The SRD 5.2.1 is licensed under the Creative Commons Attribution 4.0 International License, available at https://creativecommons.org/licenses/by/4.0/legalcode.
