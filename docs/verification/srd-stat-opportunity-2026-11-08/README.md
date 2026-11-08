# Typed melee stat-block opportunity attack

SRD 5.2.1 p.15 allows one melee attack as the opportunity Reaction. The
pinned source SHA-256 is
`8974902d109d6e63672d7c490bde9ccf052410503d9cfa768237154fbc5e3d87`.

`rs_stat_opportunity` accepts the same trusted typed melee record as
`rs_stat_attack`: printed attack bonus, damage dice/modifier/type, ability and
reach. The record's reach overrides caller-supplied weapon facts. It requires
a monster actor and the shared departure trigger, checks owned Charmed
prohibition, borrows the checked Reaction frame, and resolves one strike through
`rs_strike_resolved`. A miss spends the Reaction; refusal is atomic. It does
not spend an Action, grant Multiattack or move either creature. The returned
strike state and actual mover's restored budgets follow the ordinary opportunity
API. This natural melee record has neither mastery nor Finesse, so it has no
retained Sap/Vex/Stunning Strike/Cunning Strike rider reservation. A redundant
weapon preflight was removed rather than exempting its equivalent mutations.

## Evidence

Missing API regressions preceded implementation. Full composed entry: 227
checks pass. Mutation: 33 killed, zero survivors/no exemptions. Checks cover
10-foot departure, printed numbers independent of score, miss spending,
malformed record/non-monster/logical-span/nested-frame refusal, Charmed owner
versus unrelated conditions, natural attack event identity and returned strike
state. A seeded explicit trait-composition fixture observes shared critical
follow-up credit separately from the mover's movement; it is not a new catalog
monster. Native execution passes four actual Shield spent-slot/Quickened cases
at extended reach, including retained Shield duration, printed damage and frame
restoration. Hotbar passes 229 checks; selected Experimental combat passes all
31. Tools-check/diff check pass. Existing compiler-source and near-limit
advisories are retained; no budget was increased.

```
coax/bin/coax test aethyr/rules/srd/stat-opportunity-checks.coax --library
coax/bin/coax mutate aethyr/rules/srd/stat-opportunity-checks.coax --library --unit aethyr/rules/srd/stat-opportunity.coax --jobs 4
coax/web/tool.sh aethyr/rules/srd/stat-opportunity-native.coax
coax/bin/coax test apps/openagents-web/fivebar-test.coax --library
coax/bin/coax test apps/openagents-web/fivelab.coax --library --only 'experimental combat'
coax/web/tool.sh tools-check
```

## Remaining integration and profile boundary

This is the typed executor prerequisite for Experimental's physical monster
attacks; the physical before-departure controller is not wired by this change.
Its existing custom monster records have 5/10-foot reach; their source catalog
reconciliation remains in #13034. Record provenance remains the trusted
canonical/world adapter's responsibility; this is not arbitrary client data.
Source on-hit programs, additional attack types, special senses and continuous
geometry require their actual implementations, not substitutions with ordinary
weapons or rounded grid movement. No issue completion is claimed.

Everglade II's existing `everglade2-rules.coax` explicitly declares a social
profile: shared self effects and paid empty offensive casts, with no chosen
harm target or bystander area damage. Automatic hostile attacks on its network
movement would change that profile. Its movement integration must preserve the
declaration rather than importing the combat world's default foe selection.
