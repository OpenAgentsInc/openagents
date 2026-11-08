# Displayed level 1–6 kits: access and supplied components

Audit on 18a6e0b412, cached compiler 0.52.0. All 12 classes at each selector
level 1–6 have a nonempty displayed kit. Every displayed spell passes actual
shared `rs_spell_ready` and `rs_cast_components_ready` against the corresponding
fresh preset. Twelve independent class slices keep each check small; they
retain all six levels rather than checking only endpoints. All twelve pass,
and the complete library composition passes 200 checks. Tools-check passes.

```
coax/bin/coax test apps/openagents-web/class-kit-admission-checks.coax --library
```

This rules out missing access/preparation or preset component supply as a
current cause of refusal in these *displayed* bars. It does not prove the
bars contain every required SRD choice, the effects execute correctly, resources
were acquired in gameplay, an object is physically held, target requirements
are met, a class/subclass feature is complete or the world projects the result.
Nonspell features are only subject to the nonempty-kit check here. Those
remaining requirements still govern #13038/#13043/#13044/#13046 acceptance.

This adds regression checks and retained evidence without changing product
policy, source catalog or loadout choices. Existing zero-survivor admission
receipts remain applicable; no new authority or mutation exemption is added.
