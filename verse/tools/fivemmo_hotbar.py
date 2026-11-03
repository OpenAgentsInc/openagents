#!/usr/bin/env python3
"""5E MMO's hotbar art, drawn as Everglade's is.

    python3 verse/tools/fivemmo_hotbar.py

5E MMO (apps/openagents-web/fivemmo.coax) gives each class the hotbar 5E Lab's has
(fivemmo-battle.coax abilities): every SRD 5e spell and class feature the engine plays.
Its slots look like Everglade's (verse/tools/everglade_hotbar.py, whose drawing this
uses): a game-icons.net SVG rasterized white, tinted and laid over a dark slot, its key
on a dark tab in the top left, and a short card in Paper Mono on a dark notched panel.
A class's slots hold different abilities, so the icons are drawn bare and the key tabs
on their own (the page lays the tab over the icon):

  apps/openagents-web/realms/5e-mmo/hotbar/NAME-SIZE.jpg     an ability's icon, SIZE px a side
  apps/openagents-web/realms/5e-mmo/hotbar/key-K-SIZE.jpg    the tab of key K (1 to 9)
  apps/openagents-web/realms/5e-mmo/hotbar/NAME-card-Sx.jpg  its card at S device px a CSS px
  apps/openagents-web/fivemmo-hotbar.coax                    their names and sizes, for the page

A spell's card is its SRD 5.2.1 text (aethyr/rules/srd/spells.coax), cut to its first
sentences; a feature's is the SRD 5.2.1 feature's own sentence. The icons' credits are in
assets/verse/icons/game-icons/CREDITS.md.

This work includes material from the System Reference Document 5.2.1 ("SRD 5.2.1") by Wizards
of the Coast LLC, available at https://www.dndbeyond.com/srd. The SRD 5.2.1 is licensed under the
Creative Commons Attribution 4.0 International License, available at
https://creativecommons.org/licenses/by/4.0/legalcode.
"""
import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import everglade_hotbar as eg  # noqa: E402

ROOT = eg.ROOT
OUT = ROOT / "apps/openagents-web/realms/5e-mmo/hotbar"
TABLE = ROOT / "apps/openagents-web/fivemmo-hotbar.coax"
SPELLS = ROOT / "aethyr/rules/srd/spells.coax"

FIRE, RADIANT, NECROTIC, FORCE, PSYCHIC = (1.0, 0.55, 0.2), (1.0, 0.9, 0.5), (0.62, 0.9, 0.55), (0.8, 0.55, 1.0), (1.0, 0.6, 0.9)
HEAL, NATURE, STORM, STEEL, HOLY, FOCUS, SHADOW = (0.45, 1.0, 0.5), (0.45, 0.95, 0.4), (0.55, 0.75, 1.0), (0.86, 0.84, 0.8), (1.0, 0.85, 0.45), (0.6, 0.88, 1.0), (0.7, 0.66, 0.78)
RAGE, CHARM = (1.0, 0.35, 0.25), (0.9, 0.6, 1.0)
KEY, TIME, RULE, MANA = eg.KEY, eg.TIME, eg.RULE, eg.MANA
BACK = (f"Back {9} s after it is spent", MANA)   # fivemmo-battle.coax ACTION_RECHARGE

# The spells: their slug (the engine's id), icon and tint. Their card is the engine's SRD text.
SPELL_ART = {
    "starry-wisp": ("lorc/star-swirl.svg", RADIANT),
    "produce-flame": ("carl-olsen/flame.svg", FIRE),
    "produce-flame-hurl": ("lorc/burning-embers.svg", FIRE),
    "thunderwave": ("lorc/sonic-boom.svg", STORM),
    "entangle": ("delapouite/vines.svg", NATURE),
    "healing-word": ("zeromancer/heart-plus.svg", HEAL),
    "call-lightning": ("lorc/lightning-storm.svg", STORM),
    "fire-bolt": ("lorc/burning-meteor.svg", FIRE),
    "chill-touch": ("lorc/spectre.svg", NECROTIC),
    "ray-of-sickness": ("lorc/vomiting.svg", NECROTIC),
    "false-life": ("lorc/glass-heart.svg", NECROTIC),
    "scorching-ray": ("lorc/fire-ray.svg", FIRE),
    "blindness-deafness": ("delapouite/blindfold.svg", SHADOW),
    "fireball": ("lorc/fireball.svg", FIRE),
    "eldritch-blast": ("lorc/beam-wake.svg", FORCE),
    "hex": ("lorc/pentagram-rose.svg", FORCE),
    "hold-person": ("delapouite/frozen-body.svg", FORCE),
    "vicious-mockery": ("lorc/screaming.svg", PSYCHIC),
    "hypnotic-pattern": ("lorc/vortex.svg", CHARM),
    "sacred-flame": ("lorc/candle-flame.svg", RADIANT),
    "guiding-bolt": ("lorc/spark-spirit.svg", RADIANT),
    "spirit-guardians": ("lorc/angel-outfit.svg", RADIANT),
    "divine-smite": ("lorc/lightning-saber.svg", RADIANT),
    "hunters-mark": ("lorc/on-target.svg", NATURE),
}
# The class features on the bars: slug, icon, tint, card title, the SRD 5.2.1 sentence, details.
FEATURES = [
    ("attack", "lorc/crossed-swords.svg", STEEL, "Attack",
     "Make an attack with a weapon or an Unarmed Strike. Your turn takes it when no other slot is waiting: your target if it is in reach, else a shot, else your class's attack cantrip.",
     [("Action", RULE)]),
    ("dash", "lorc/run.svg", STEEL, "Dash",
     "When you take the Dash action, you gain extra movement for the current turn. The increase equals your Speed after applying any modifiers.",
     [("Action", RULE)]),
    ("lands-aid", "lorc/flowers.svg", NATURE, "Land's Aid",
     "Expend a use of your Wild Shape: flowers and thorns fill a 10-foot-radius Sphere within 60 feet. Each creature of your choice in it makes a Constitution save, 2d6 Necrotic damage, half on a success, and one creature of your choice there regains 2d6 Hit Points.",
     [("Magic action", RULE), ("Wild Shape", RULE), BACK]),
    ("innate-sorcery", "lorc/magic-swirl.svg", FIRE, "Innate Sorcery",
     "For 1 minute the spell save DC of your Sorcerer spells increases by 1, and you have Advantage on the attack rolls of Sorcerer spells you cast.",
     [("Bonus Action", RULE), ("1 minute", TIME), BACK]),
    ("bardic-inspiration", "lorc/lyre.svg", PSYCHIC, "Bardic Inspiration",
     "Inspire another creature within 60 feet who can see or hear you: it gains one of your Bardic Inspiration dice, to add to a D20 Test it fails within the next hour.",
     [("Bonus Action", RULE), ("60 ft range", RULE), BACK]),
    ("turn-undead", "lorc/holy-symbol.svg", HOLY, "Turn Undead",
     "Present your Holy Symbol and censure Undead: each Undead of your choice within 30 feet makes a Wisdom save or has the Frightened and Incapacitated conditions for 1 minute, and tries to move away from you.",
     [("Magic action", RULE), ("Channel Divinity", RULE), BACK]),
    ("preserve-life", "lorc/angel-wings.svg", HEAL, "Preserve Life",
     "Present your Holy Symbol and restore Hit Points equal to five times your Cleric level, divided among Bloodied creatures within 30 feet, you among them; none goes past half its Hit Point maximum.",
     [("Magic action", RULE), ("Channel Divinity", RULE), BACK]),
    ("second-wind", "lorc/heart-bottle.svg", HEAL, "Second Wind",
     "Draw on a limited well of physical and mental stamina: regain Hit Points equal to 1d10 plus your Fighter level.",
     [("Bonus Action", RULE), BACK]),
    ("action-surge", "lorc/lightning-helix.svg", STEEL, "Action Surge",
     "Push yourself beyond your normal limits for a moment: on your turn, take one additional action, except the Magic action.",
     [("No action", RULE), BACK]),
    ("lay-on-hands", "lorc/glowing-hands.svg", HEAL, "Lay On Hands",
     "Touch a creature, which could be yourself, and draw power from your pool of healing to restore Hit Points to it.",
     [("Bonus Action", RULE), ("Touch", RULE), BACK]),
    ("sacred-weapon", "lorc/winged-sword.svg", HOLY, "Sacred Weapon",
     "Imbue a Melee weapon you hold with positive energy for 10 minutes: add your Charisma modifier to its attack rolls, its hits can deal Radiant damage, and it sheds Bright Light in a 20-foot radius.",
     [("With the Attack action", RULE), ("Channel Divinity", RULE), BACK]),
    ("reckless-attack", "lorc/axe-swing.svg", RAGE, "Reckless Attack",
     "Attack recklessly: Advantage on attack rolls using Strength until the start of your next turn, but attack rolls against you have Advantage during that time.",
     [("With your first attack", RULE)]),
    ("rage", "delapouite/angry-eyes.svg", RAGE, "Rage",
     "Imbue yourself with a primal power: Resistance to Bludgeoning, Piercing and Slashing damage, extra damage on Strength attacks, and Advantage on Strength checks and saving throws.",
     [("Bonus Action", RULE), ("Up to 10 minutes", TIME), BACK]),
    ("flurry-of-blows", "lorc/punch-blast.svg", FOCUS, "Flurry of Blows",
     "Expend 1 Focus Point to make two Unarmed Strikes.",
     [("Bonus Action", RULE), ("1 Focus Point", MANA), BACK]),
    ("patient-defense", "lorc/meditation.svg", FOCUS, "Patient Defense",
     "Expend 1 Focus Point to take both the Disengage and the Dodge actions.",
     [("Bonus Action", RULE), ("1 Focus Point", MANA), BACK]),
    ("step-of-the-wind", "lorc/footprint.svg", FOCUS, "Step of the Wind",
     "Expend 1 Focus Point to take both the Disengage and the Dash actions, and your jump distance is doubled for the turn.",
     [("Bonus Action", RULE), ("1 Focus Point", MANA), BACK]),
    ("wholeness-of-body", "lorc/beams-aura.svg", HEAL, "Wholeness of Body",
     "Roll your Martial Arts die: regain Hit Points equal to the number rolled plus your Wisdom modifier (at least 1).",
     [("Bonus Action", RULE), BACK]),
    ("steady-aim", "lorc/archery-target.svg", STEEL, "Steady Aim",
     "Give yourself Advantage on your next attack roll on the current turn. Only if you haven't moved this turn, and your Speed is 0 until its end.",
     [("Bonus Action", RULE)]),
    ("cunning-dash", "lorc/cloak-dagger.svg", SHADOW, "Cunning Action: Dash",
     "Your quick thinking and agility let you take the Dash action as a Bonus Action.",
     [("Bonus Action", RULE)]),
]

CAST = {"RS_T_ACTION": "Action", "RS_T_BONUS": "Bonus Action", "RS_T_REACTION": "Reaction"}


def srd_spells():
    """The engine's spells: slug -> name, level, cast time, range, duration, concentration, text."""
    src = SPELLS.read_text()
    out = {}
    for m in re.finditer(r'id = "([a-z-]+)", name = "([^"]+)", level = (\d+).*?cast_time = (RS_T_\w+), range_ft = (\d+)'
                         r'.*?duration = "([^"]*)".*?concentration = (\w+).*?text = "((?:[^"\\]|\\.)*)"', src, re.S):
        out[m.group(1)] = dict(name=m.group(2), level=int(m.group(3)), cast=CAST.get(m.group(4), "Action"),
                               range=int(m.group(5)), duration=m.group(6), text=m.group(8))
    return out


def first_sentences(text, most=360):
    """The spell's text up to its last whole sentence within `most` characters (at least one)."""
    parts = re.split(r"(?<=[.!?])\s+", text)
    out = parts[0]
    for p in parts[1:]:
        if len(out) + 1 + len(p) > most:
            break
        out += " " + p
    return out


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    spells = srd_spells()
    slots = []
    for slug, (svg, tint) in SPELL_ART.items():
        sp = spells[slug]
        title = sp["name"] + (": Hurl" if slug == "produce-flame-hurl" else "")
        details = [("Cantrip" if sp["level"] == 0 else f"Level {sp['level']}", RULE), (sp["cast"], RULE)]
        details.append(("Self" if sp["range"] == 0 else f"{sp['range']} ft range", RULE))
        if sp["duration"] != "Instantaneous":
            details.append((sp["duration"], TIME))
        if sp["level"] > 0:
            details.append(("A spell slot, " + BACK[0].lower(), MANA))
        slots.append((slug, svg, tint, title, first_sentences(sp["text"]), details))
    for slug, svg, tint, title, body, details in FEATURES:
        slots.append((slug, svg, tint, title, body, details))
    names, cards = [], []
    for slug, svg, tint, title, body, details in slots:
        names.append(slug)
        for size in eg.SIZES:
            eg.icon(slug, svg, tint, "", size, bare=True).save(OUT / f"{slug}-{size}.jpg", quality=90, subsampling=0)
        for scale in eg.SCALES:
            image = eg.card(title, body, details, scale)
            image.save(OUT / f"{slug}-card-{scale}x.jpg", quality=90, subsampling=0)
            cards.append(image.size)
    for k in range(1, 10):
        for size in eg.SIZES:
            eg.key_tab(str(k), size).save(OUT / f"key-{k}-{size}.jpg", quality=90, subsampling=0)
    tabs = [eg.tab_size("1", size) for size in eg.SIZES]
    n = len(slots)
    table = [
        "// 5E MMO's hotbar pictures (realms/5e-mmo/hotbar/), drawn as Everglade's bar is: written by",
        "// verse/tools/fivemmo_hotbar.py, do not edit. Each ability's name (a spell's engine id, else its",
        "// feature's), the icon sizes (px a side), each card at 1x, 2x and 3x (device px, ability * 3 +",
        "// scale - 1), and a key's tab (key-K-SIZE.jpg) at each icon size.",
        f"MH_ABILITIES :: {n};",
        f"MH_NAME :: [{n}] string.[{', '.join(f'\"{x}\"' for x in names)}];",
        f"MH_SIZES :: {len(eg.SIZES)};",
        f"MH_SIZE :: [{len(eg.SIZES)}] int.[{', '.join(str(s) for s in eg.SIZES)}];",
        f"MH_CARD_W :: [{len(cards)}] f32.[{', '.join(str(w) for w, _ in cards)}];",
        f"MH_CARD_H :: [{len(cards)}] f32.[{', '.join(str(h) for _, h in cards)}];",
        f"MH_TAB_W :: [{len(tabs)}] f32.[{', '.join(str(w) for w, _ in tabs)}];",
        f"MH_TAB_H :: [{len(tabs)}] f32.[{', '.join(str(h) for _, h in tabs)}];",
        "",
    ]
    TABLE.write_text("\n".join(table))
    print(f"{n} abilities: {len(eg.SIZES)} icon sizes and {len(eg.SCALES)} cards each, 9 key tabs, into {OUT.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
