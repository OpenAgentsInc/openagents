# Lab loadout build regressions

Partial #13038/#13046 work on e181d4daed, using cached compiler 0.52.0.
The complete Lab build is still unqualified: the Siege tactics `met` assertion
fails, independently reproduced on unmodified main 79a2ce92b3. The retained
baseline log is evidence of that failure, not a passing release receipt.

Spell eligibility and grant queries now project class, level, invocation and
grant words instead of copying the entire combat creature in loadout loops.
The public creature APIs retain their semantics. The sliders regression passes
at its unchanged limit (202323162 steps); no interpreter budget was raised.
The primitive policy keeps canonical eligibility, parent spell roots, explicit
world grants and Fiendish Vigor boundaries. No new grant or waiver is introduced.

The existing level-17 Player Warlock world kit now explicitly supplies Circle
of Death's reusable powder of a crushed black pearl worth 500+ GP (p.115 of
the pinned SRD 5.2.1). The bar check verifies component availability and that
casting retains the material. This preserves higher-level existing behavior;
it does not expand the epic's level-1–6 qualification scope. Finger of Death's
kill fixture now uses a killed NPC Humanoid. An independent legal level-six
player fixture verifies that merely reaching zero HP without instant death
does not cause resurrection as a Zombie. No executor change was needed.

## Evidence

Logs retain the initial missing-policy/component regressions and final results:
187 library checks, 226 component checks, sliders, Player Warlock and independent
dying-victim checks pass. Mutation gates kill 45 projection-policy, 215 selection
and 186 access mutants, zero survivors and no exemptions. Tools-check passes.
The unchanged mutation and selected-world receipts were reused rather than rerun.

Commands for the library composition:

```
coax/bin/coax test aethyr/rules/srd/spell-state-checks.coax --library
coax/bin/coax test aethyr/rules/srd/component-checks.coax --library
```

Mutation uses `--library --unit` with spell-state-checks as the entry for
spell-state-policy/selection and component-checks for spell-access. Selected
Lab checks use `coax test apps/openagents-web/fivelab.coax --only` with the
names printed in the retained logs. No full Wasm/browser pass is claimed.
