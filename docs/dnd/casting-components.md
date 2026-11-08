# Casting component state and admission

Implemented component-policy foundation for #13038, 2026-11-08. Source is the
pinned SRD 5.2.1 pp.105–106; this does not qualify a complete equipment system
or every spell. [Verification](../verification/srd-components-2026-11-08/README.md).

The canonical importer already preserves component text, V/S/M, material cost
text and consumption. The compact runtime index now carries the corresponding
flags: V=1, S=2, M=4, priced=8, consumed=16. All 339 projected records are
compared independently with the canonical catalog. Legacy executable strings
are not the admission authority. Source IDs, class tables and PDF pin stay fixed.

## Authoritative facts

`RsCreature` carries speech blockage, a free gesture hand, a hand available to
access materials/pouch, and an authorized focus held or available under its
description's non-held exception. `focus_class` records the feature authorizing
the focus. `focus_worn` means a lawful description-specific exception, not a
generic ability to wear any focus. Equipment/effect owners must maintain these
facts; they are not client-supplied cast options. Default hand facts describe
available hands, not a proof that a rendered weapon is currently held.

A V spell refuses when speech is blocked. An S spell needs a gesture hand;
an M+S spell may share the hand actually accessing its materials or holding
its eligible focus. A held focus does not provide a free hand for an S-only
spell. A non-held focus exception provides M availability, not a gesture hand.
Pouches need a hand to access them. Priced or consumed materials cannot be
replaced by a pouch or focus.

Eight bounded material-stock entries hold canonical requirement keys and
quantities. One unit means a complete exact material set satisfying that
spell's canonical requirement, including required value; it is not a pool of
gold, an arbitrary stone, or a general equipment item. The material owner must
establish that receipt from actual inventory. Admission checks positive matching
stock; payment removes one set only when the requirement is consumed. Duplicate
stock entries pay from the first positive matching entry; empty and unrelated
entries and other actors remain unchanged. Reusable priced sets are retained.
No insertion/eviction API or automatic currency purchase is added here.

This receipt boundary does not yet resolve target-dependent sets (for example
materials required per corpse), actual item IDs, sharing one physical item
across multiple requirement recipes, item acquisition/valuation, wield/drop
transitions or hands changed by animations. Those need equipment/feature/world
integration before their spells or worlds can be qualified. It must not be
used to mint a complete set merely because a player owns enough gold.

## Cast lifecycle and grants

Ordinary casts check components in admission and pay after target/option and
effect/zone capacity preflight, before costs and effects. Reactions share
component preflight and payment with their slot-history transaction. Slot
spending changes no component fact between preflight and payment. Empty-target
casts use the same component payment. Operational helpers such as hurling an
already held Produce Flame do not recast the parent or repay its components.
The four existing house records have empty component declarations; their
identity remains explicitly non-SRD.

`rs_grant_class_spell` records class-feature provenance in addition to ordinary
spell access. The source class list or an owned class-feature grant can authorize
that class's focus. A generic `rs_grant_spell` world grant supplies access but
does not by itself expand class-focus eligibility. Existing subclass/discovery
presets use class grants; Fiendish Vigor remains a selected Warlock feature.
This is not full validation of every possible feature selection or multiclassing.

Fresh class presets supply their focus. Cleric/Paladin presets include the
reusable Holy Symbol satisfying Bless's priced requirement. Current Mage,
Priest and Fanatic constructors name their source Wand/Holy Symbol focus facts.
Everglade's existing broad destruction caster explicitly carries a pouch for
its extra-list spells and reusable Holy Symbol, Circle of Death pearl and
Shapechange circlet receipts. This is that world's casting kit, not baseline
wealth granted to every class or proof of those high-level spell mechanics.

## Remaining work

Actual grip/inventory/effect maintenance and player/wire controls remain
#13044/#13046 work. Component waivers, Subtle Spell/other feature exceptions,
innate casting, casting workflows and target-dependent recipe quantities need
their source-specific execution. Silence's spell/zone implementation is not
created by the speech-blockage fact. The old room's unrestricted resource
reset is not a component waiver or a completed shared profile. No broader
casting, character or world compliance is claimed.
