#!/usr/bin/env python3
"""Everglade's spell bar art, as the previous engine drew it.

    python3 verse/tools/everglade_hotbar.py

The old engine's Everglade bar (`git show 0b401561a0^:crates/verse-zone-everglade/
src/zones/everglade/hotbar.rs`) drew each slot as a game-icons.net SVG,
rasterized white, tinted and laid over a dark slot (`crates/verse-content/src/
compiler/icons.rs` rasterize), its key in Paper Mono at the top right, and a
slot's card (`crates/verse-core/src/tooltip.rs`) in Paper Mono on a dark
notched panel. The page draws pictures through the host's media
(coax/web/host.js coaxial_media), so this writes those pictures:

  apps/openagents-web/realms/everglade/hotbar/NAME-SIZE.jpg   a slot's icon and key, SIZE px a side
  apps/openagents-web/realms/everglade/hotbar/NAME-card-Sx.jpg a slot's card at S device px a CSS px
  apps/openagents-web/everglade-hotbar.coax                    their names and sizes, for the page

It needs rsvg-convert and Pillow. The icons' credits are in
assets/verse/icons/game-icons/CREDITS.md, and the realm's licences page lists them.
"""
import io
import math
import subprocess
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

ROOT = Path(__file__).resolve().parents[2]
ICONS = ROOT / "assets/verse/icons/game-icons"
FONT = ROOT / "assets/verse/fonts/paper-mono/PaperMono-Medium.ttf"
OUT = ROOT / "apps/openagents-web/realms/everglade/hotbar"
TABLE = ROOT / "apps/openagents-web/everglade-hotbar.coax"

# The old rasterizer's slot background and padding (icons.rs BACKGROUND, PADDING).
BACKGROUND = (18 / 255, 21 / 255, 29 / 255)
PADDING = 0.12
# Icon sizes written: the page draws the smallest at least as large as the slot (36 units).
SIZES = (48, 72, 96, 128)
# Card densities: 1x, 2x and 3x, the page picks its device pixel ratio's.
SCALES = (1, 2, 3)
KEY_INK = (0.75, 0.75, 0.75)   # hotbar.rs draw_of: the key's label

# tooltip.rs palette (linear, as the old HUD kept its colours) and metrics, in CSS px at 1x
SURFACE = (0.04, 0.03, 0.018)
BORDER = (0.55, 0.45, 0.28)
SHADOW = (0.05, 0.03, 0.012)
TITLE = (0.98, 0.82, 0.38)
BODY = (0.88, 0.82, 0.68)
KEY = (0.78, 0.68, 0.48)
MANA = (0.62, 0.78, 1.0)
TIME = (0.64, 0.8, 0.5)
RULE = (0.8, 0.7, 0.95)
SEPARATOR = (0.46, 0.39, 0.24)
MAX_WIDTH, PAD_X, PAD_Y, GAP = 360.0, 9.0, 7.0, 3.0
FONT_PX = 14.0   # the HUD atlas: Paper Mono at 14 CSS px (everglade-web web.rs)

# icons.rs tints
METEOR = (1.0, 0.42, 0.18)
SUNLIGHT = (1.0, 0.9, 0.5)
MEGA = (0.82, 0.48, 1.0)
WATER = (0.35, 0.72, 1.0)
OIL = (0.86, 0.7, 0.36)
TRANSMUTATION = (0.76, 0.62, 1.0)
WIND = (0.6, 0.95, 0.9)
STORM = (0.55, 0.75, 1.0)
STONE = (0.82, 0.76, 0.66)
ROT = (0.55, 0.9, 0.5)
ICE = (0.65, 0.92, 1.0)
MIST = (0.72, 0.86, 1.0)
MOON = (0.88, 0.9, 1.0)
FIRE = (1.0, 0.55, 0.2)
VENOM = (0.6, 1.0, 0.35)
GROWTH = (0.45, 0.95, 0.4)
FEY = (0.9, 0.6, 1.0)
REST = (0.45, 1.0, 0.5)
THORN = (0.75, 0.82, 0.45)

FREE = ("No mana, no cooldown", MANA)
RANGE = ("140 m range", RULE)   # everglade-spells.coax SP_RANGE
# An SVG name ending in "^" is drawn upside down (icons.rs flip_y: Reverse Gravity's arrows point up).
# Each slot in the bar's order: its name, SVG, tint, key, card title, sentence and details.
# The four spells' sentences and details are the old dev bar's (hotbar.rs SLOTS), with
# this realm's cast bars (verse/rules/srd5.coax) and reach.
SLOTS = [
    ("meteor-swarm", "lorc/meteor-impact.svg", METEOR, "1", "Meteor Swarm",
     "Calls down six blazing meteors on a circle of ground you choose, blasting apart the town's buildings they reach; R restores the town.",
     [("Key 1", KEY), FREE, ("2.5 s cast", TIME), RANGE]),
    ("thunderbolt", "lorc/lightning-frequency.svg", SUNLIGHT, "2", "Thunderbolt",
     "Calls one huge bolt of lightning down on the wall or ground you choose, blasting a hole through the building it strikes; R restores the town.",
     [("Key 2", KEY), FREE, ("1.2 s cast", TIME), RANGE]),
    ("mega-thunderbolt", "lorc/lightning-frequency.svg", MEGA, "3", "Mega Thunderbolt",
     "Calls down a bolt five times as destructive as the Thunderbolt on the wall or ground you choose, tearing through the building it strikes; R restores the town.",
     [("Key 3", KEY), FREE, ("0.5 s cast", TIME), RANGE]),
    ("water-orb", "lorc/frozen-orb.svg", WATER, "4", "Water Orb",
     "Gathers an orb of water over you and throws it at the spot you choose, where it bursts and sends loose things rolling without breaking them.",
     [("Key 4", KEY), FREE, ("0.6 s cast", TIME), RANGE]),
    ("reverse-gravity", "delapouite/gravitation.svg^", TRANSMUTATION, "5", "Reverse Gravity",
     "Gravity flips in a 50-foot cylinder around you: you, the loose things in it and the townsfolk who fail to grab hold fall upward and hover near its top until you press again.",
     [("Key 5", KEY), FREE, ("1 minute", TIME)]),
    ("wind-wall", "lorc/whirlwind.svg", WIND, "6", "Wind Wall",
     "Raises another 30-foot wall of wind 4 m ahead whose updraft throws you upward when you walk into it.",
     [("Key 6", KEY), FREE, ("1 minute", TIME)]),
    ("levitate", "delapouite/body-height.svg", TRANSMUTATION, "7", "Levitate",
     "Hold to rise up to 18 m over the ground and let go to hover there; tap while hovering to fall, or hold X to sink.",
     [("Key 7", KEY), FREE, ("10 minutes", TIME)]),
    ("feather-fall", "lorc/feather.svg", TRANSMUTATION, "8", "Feather Fall",
     "Cast while falling to slow your descent, and that of up to five falling townsfolk near you, to a gentle drift until you land; press again to end it.",
     [("Key 8", KEY), FREE, ("1 minute", TIME)]),
    ("gust-of-wind", "lorc/wind-slap.svg", WIND, "9", "Gust of Wind",
     "A line of strong wind 60 feet long blows from you the way you face: it pushes townsfolk 15 feet unless they stand firm, sends loose debris and floats flying, and clears smoke; press again to end it.",
     [("Key 9", KEY), FREE, ("1 minute", TIME)]),
    ("thunderwave", "lorc/sonic-boom.svg", STORM, "0", "Thunderwave",
     "A wave of thunder sweeps out from you through a 15-foot cube ahead: 2d8 thunder, and townsfolk and loose things in it are pushed 10 feet away, with a boom heard 300 feet off.",
     [("Key 0", KEY), FREE]),
    ("wall-of-stone", "delapouite/stone-wall.svg", STONE, "S1", "Wall of Stone",
     "Raises another pair of granite panels 4 m ahead that block your way for ten minutes.",
     [("Shift+1", KEY), FREE, ("10 minutes", TIME)]),
    ("telekinesis", "lorc/juggler.svg", TRANSMUTATION, "S2", "Telekinesis",
     "Take hold of a loose thing, a prop or a person within 60 feet and lift it 20 feet; click again to move it up to 30 feet, and let go to throw it. A person makes a Strength save or is held.",
     [("Shift+2", KEY), FREE, ("60 ft range", RULE), ("Concentration, up to 10 minutes", TIME)]),
    ("black-tentacles", "delapouite/tentacles-barrier.svg", ROT, "S3", "Black Tentacles",
     "Writhing tentacles fill a 20-foot square of ground: townsfolk in it make a Strength save or take 3d6 bludgeoning and are held, and the tentacles seize the loose things in reach.",
     [("Shift+3", KEY), FREE, ("90 ft range", RULE), ("Concentration, up to 1 minute", TIME)]),
    ("ice-knife", "lorc/ice-spear.svg", ICE, "S4", "Ice Knife",
     "Throw a shard of ice at someone or a spot within 60 feet: a ranged spell attack for 1d10 piercing, then it bursts, and each creature within 5 feet makes a Dexterity save or takes 2d6 cold. It leaves slippery frost.",
     [("Shift+4", KEY), FREE, ("60 ft range", RULE)]),
    ("ice-storm", "lorc/ice-bolt.svg", ICE, "S5", "Ice Storm",
     "Hail pounds a 20-foot-radius cylinder: 2d10 bludgeoning and 4d6 cold, half on a Dexterity save, and windows break. Its ground is rough, then icy; on water it freezes hard enough to walk on.",
     [("Shift+5", KEY), FREE, ("300 ft range", RULE)]),
    ("sleet-storm", "lorc/snowflake-2.svg", ICE, "S6", "Sleet Storm",
     "Freezing sleet fills a 20-foot-radius cylinder: it hides what is behind it, puts out fires and ices the ground. Whoever is in it makes a Dexterity save each round or falls Prone and loses Concentration.",
     [("Shift+6", KEY), FREE, ("150 ft range", RULE), ("Concentration, up to 1 minute", TIME)]),
    ("water-walk", "lorc/sprint.svg", WATER, "S7", "Water Walk",
     "You walk and run on the water's surface as if it were solid ground; under it, you rise to its top. Press again to end it.",
     [("Shift+7", KEY), FREE, ("1 hour", TIME)]),
    ("control-water", "lorc/tornado-discs.svg", WATER, "S8", "Control Water",
     "Click water: you flood it first, a pond over its rim or a wave down the river. Press again to part it, again to turn its current your way, again for a whirlpool, and once more to end it.",
     [("Shift+8", KEY), FREE, ("300 ft range", RULE), ("Concentration, up to 10 minutes", TIME)]),
    ("create-water", "delapouite/fog.svg", MIST, "S9", "Create or Destroy Water",
     "Rain falls on a 30-foot cube and puts out what burns in it; press again before the click to destroy water instead, lifting the fog and the slush there away.",
     [("Shift+9", KEY), FREE, ("30 ft range", RULE)]),
    ("water-breathing", "lorc/shield-reflect.svg", MIST, "S0", "Water Breathing",
     "You breathe under water, so you can dive and stay down. Press again to end it.",
     [("Shift+0", KEY), FREE, ("24 hours", TIME)]),
    ("moonbeam", "delapouite/moon-orbit.svg", MOON, "S-", "Moonbeam",
     "A beam of pale light shines down in a 5-foot-radius cylinder: whoever is in it makes a Constitution save each round or takes 2d10 radiant, half on a success. Click again to move it 60 feet.",
     [("Shift+-", KEY), FREE, ("120 ft range", RULE), ("Concentration, up to 1 minute", TIME)]),
    ("produce-flame", "carl-olsen/flame.svg", FIRE, "C1", "Produce Flame",
     "A flame in your hand lights the way 40 feet round you; click someone or something within 60 feet to hurl it: a spell attack for 2d8 fire that sets debris burning. Press again to put it out.",
     [("Ctrl+1", KEY), FREE, ("Cantrip", RULE), ("60 ft range", RULE), ("10 minutes", TIME)]),
    ("starry-wisp", "lorc/star-swirl.svg", SUNLIGHT, "C2", "Starry Wisp",
     "Flings a mote of starlight at someone or something within 60 feet: a spell attack for 2d8 radiant, and whom it hits sheds dim light for a round.",
     [("Ctrl+2", KEY), FREE, ("Cantrip", RULE), ("60 ft range", RULE)]),
    ("shillelagh", "lorc/wizard-staff.svg", GROWTH, "C3", "Shillelagh",
     "Your staff glows with nature's power: click someone in reach to strike, +7 to hit for 1d10 + 3 force; a loose thing it hits is knocked away. Press again to end it.",
     [("Ctrl+3", KEY), FREE, ("Cantrip", RULE), ("5 ft reach", RULE), ("1 minute", TIME)]),
    ("poison-spray", "lorc/poison-gas.svg", VENOM, "C4", "Poison Spray",
     "Sprays toxic mist at someone within 30 feet: a spell attack for 2d12 poison. The mist hangs where it lands.",
     [("Ctrl+4", KEY), FREE, ("Cantrip", RULE), ("30 ft range", RULE)]),
    ("elementalism", "lorc/tornado-discs.svg", WIND, "C5", "Elementalism",
     "A 5-foot cube within 30 feet answers you: a breeze, a shroud of dust, harmless embers that light or snuff a small fire, or a cool mist. Press again for the next element.",
     [("Ctrl+5", KEY), FREE, ("Cantrip", RULE), ("30 ft range", RULE)]),
    ("entangle", "delapouite/vines.svg", GROWTH, "C6", "Entangle",
     "Grasping plants fill a 20-foot square: its ground is difficult terrain, and each creature there but you makes a Strength save or is restrained until it tears free.",
     [("Ctrl+6", KEY), FREE, ("90 ft range", RULE), ("Concentration, up to 1 minute", TIME)]),
    ("faerie-fire", "delapouite/fairy.svg", FEY, "C7", "Faerie Fire",
     "Each creature in a 20-foot cube makes a Dexterity save or is outlined in blue, green or violet light: it sheds dim light, and attacks at it have advantage. Press again to end it.",
     [("Ctrl+7", KEY), FREE, ("60 ft range", RULE), ("Concentration, up to 1 minute", TIME)]),
    ("healing-word", "zeromancer/heart-plus.svg", REST, "C8", "Healing Word",
     "Click someone within 60 feet: they regain 2d4 + 3 hit points. Click anywhere else to heal yourself.",
     [("Ctrl+8", KEY), FREE, ("Bonus action", RULE), ("60 ft range", RULE)]),
    ("spike-growth", "lorc/thorny-vine.svg", THORN, "C9", "Spike Growth",
     "Hidden thorns cover a 20-foot radius: its ground is difficult terrain, and whoever moves through it, you too, takes 2d4 piercing for every 5 feet. Press again to end it.",
     [("Ctrl+9", KEY), FREE, ("150 ft range", RULE), ("Concentration, up to 10 minutes", TIME)]),
    ("call-lightning", "lorc/lightning-storm.svg", STORM, "C0", "Call Lightning",
     "A storm cloud gathers 100 feet over you. Click under it for a bolt: each creature within 5 feet makes a Dexterity save, 3d10 lightning, half on a success, and the grass catches. Each click calls another.",
     [("Ctrl+0", KEY), FREE, ("Under the cloud, 60 ft round", RULE), ("Concentration, up to 10 minutes", TIME)]),
    ("wall-of-fire", "lorc/fire-ring.svg", FIRE, "C=", "Wall of Fire",
     "A wall of fire 60 feet long and 20 feet high rises across your view: 5d8 fire to whoever is in it, half on a Dexterity save, and its far side burns whoever ends a round within 10 feet. The townsfolk walk round it; the grass catches.",
     [("Ctrl+=", KEY), FREE, ("120 ft range", RULE), ("Concentration, up to 1 minute", TIME)]),
    ("conjure-animals", "lorc/wolf-howl.svg", FEY, "C-", "Conjure Animals",
     "A pack of spectral wolves gathers where you click; click again and it runs up to 30 feet. Whoever comes within 10 feet of it, you too, makes a Dexterity save or takes 3d10 slashing, once a round. Press again to end it.",
     [("Ctrl+-", KEY), FREE, ("60 ft range", RULE), ("Concentration, up to 10 minutes", TIME)]),
    ("polymorph", "delapouite/frog-prince.svg", TRANSMUTATION, "A1", "Polymorph",
     "Click someone within 60 feet: a Wisdom save, or they are a frog until its 1 hit point is gone. Click anywhere else to be a brown bear: 22 temporary hit points, faster, a bite and a claw, no spells.",
     [("Alt+1", KEY), FREE, ("60 ft range", RULE), ("Concentration, up to 1 hour", TIME)]),
    ("mass-cure-wounds", "delapouite/healing.svg", REST, "A2", "Mass Cure Wounds",
     "Up to six creatures within 30 feet of where you click, you among them, regain 5d8 + 3 hit points.",
     [("Alt+2", KEY), FREE, ("60 ft range", RULE)]),
    ("sunbeam", "lorc/sunbeams.svg", SUNLIGHT, "A3", "Sunbeam",
     "A beam of sunlight 60 feet long flashes the way you point: 6d8 radiant to each creature in it, half on a Constitution save, and a failure is blinded until your next turn. Each click flashes another; you shine 30 feet round.",
     [("Alt+3", KEY), FREE, ("60 ft line", RULE), ("Concentration, up to 1 minute", TIME)]),
    ("wall-of-thorns", "lorc/thorn-helix.svg", THORN, "A4", "Wall of Thorns",
     "A wall of thorny brush 60 feet long and 10 feet high rises across your view: 7d8 to whoever is in it or walks into it, you too, half on a Dexterity save, and it is hard going. The townsfolk walk round it.",
     [("Alt+4", KEY), FREE, ("120 ft range", RULE), ("Concentration, up to 10 minutes", TIME)]),
    ("fire-storm", "lorc/burning-embers.svg", METEOR, "A5", "Fire Storm",
     "Ten 10-foot cubes of roaring flame, two rows of five across your view: 7d10 fire to each creature in them, you too, half on a Dexterity save. The grass in them catches.",
     [("Alt+5", KEY), FREE, ("150 ft range", RULE)]),
    ("sunburst", "lorc/sun-radiations.svg", SUNLIGHT, "A6", "Sunburst",
     "Sunlight flashes 60 feet round where you click: 12d6 radiant to each creature in it, you too, half on a Constitution save. A failure is blinded for a minute, saving again each round.",
     [("Alt+6", KEY), FREE, ("150 ft range", RULE)]),
    ("storm-of-vengeance", "lorc/lightning-tree.svg", STORM, "A7", "Storm of Vengeance",
     "A storm cloud 300 feet round gathers where you click, over you too: thunder that deafens, acid rain, six bolts of 10d6 lightning, hail, then freezing rain that hides and slows. Its rain puts out fires.",
     [("Alt+7", KEY), FREE, ("Sight", RULE), ("Concentration, up to 1 minute", TIME)]),
    ("shapechange", "lorc/dragon-spiral.svg", TRANSMUTATION, "A8", "Shapechange",
     "Click to become a mammoth: 126 temporary hit points, faster, two gores at whoever you click within 10 feet, no spells. Press again for a brown bear, a wolf, then yourself.",
     [("Alt+8", KEY), FREE, ("Self", RULE), ("Concentration, up to 1 hour", TIME)]),
    ("speak-with-animals", "lorc/parrot-head.svg", FEY, "A9", "Speak with Animals",
     "For 10 minutes you understand the town's beasts and they you: the cats, the ducks and any frog say their piece. Press again to end it.",
     [("Alt+9", KEY), FREE, ("Ritual", RULE), ("10 minutes", TIME)]),
    ("wild-resurgence", "lorc/cycle.svg", GROWTH, "A0", "Wild Resurgence",
     "A spell slot spent for a use of Wild Shape, taken at once: you are a wolf with 9 temporary hit points, a bite at whoever you click in reach. Press again to end it.",
     [("Alt+0", KEY), FREE, ("No action", RULE)]),
    ("rebuild", "delapouite/house.svg", OIL, "R", "Rebuild and Long Rest",
     "Stands every broken building in the town back up whole, and you take a long rest: all your hit points back, every condition gone, your own shape again. The watch still wants you if it did.",
     [("Key R", KEY)]),
    # a phone's row switcher (the old Grove's Compact layout, its rise-icon)
    ("next-row", "delapouite/plain-arrow.svg^", TRANSMUTATION, "", "Next row",
     "Shows the bar's next row of spells.",
     []),
]


def byte(v):
    return max(0, min(255, round(v * 255)))


def srgb(colour):
    """The old HUD's colours are linear: its batch wrote them to an sRGB target, which encoded them."""
    return tuple(12.92 * c if c <= 0.0031308 else 1.055 * c ** (1 / 2.4) - 0.055 for c in colour)


def svg_coverage(svg, size):
    """The white artwork's coverage at size x size, padded as icons.rs pads it."""
    inner = round(size * (1 - 2 * PADDING))
    png = subprocess.run(["rsvg-convert", "-w", str(inner), "-h", str(inner), "-a", str(svg)],
                         check=True, capture_output=True).stdout
    art = Image.open(io.BytesIO(png)).convert("RGBA")
    cover = Image.new("L", (size, size), 0)
    at = round(size * PADDING)
    cover.paste(art.getchannel("A"), (at + (inner - art.width) // 2, at + (inner - art.height) // 2))
    return cover


def metrics(px):
    """The atlas's metrics at px (ui.rs rasterize): the advance of M, the line and the ascent, whole pixels."""
    font = ImageFont.truetype(str(FONT), px)
    head = FONT.read_bytes()
    import struct
    n = struct.unpack(">H", head[4:6])[0]
    tables = {}
    for i in range(n):
        tag, _, off, _ = struct.unpack(">4sIII", head[12 + 16 * i:28 + 16 * i])
        tables[tag] = off
    upm = struct.unpack(">H", head[tables[b"head"] + 18:tables[b"head"] + 20])[0]
    asc, desc, gap = struct.unpack(">hhh", head[tables[b"hhea"] + 4:tables[b"hhea"] + 10])
    return font, math.ceil(font.getlength("M")), math.ceil(px * (asc - desc + gap) / upm), math.ceil(px * asc / upm)


def text(draw, font, ascent, x, y, line, colour):
    """ui.rs text: the pen at x, the baseline at y + ascent, both whole pixels."""
    draw.text((round(x), round(y + ascent)), line, font=font, fill=tuple(byte(c) for c in srgb(colour)), anchor="ls")


def icon(name, svg, tint, key, size):
    flip = svg.endswith("^")
    cover = svg_coverage(ICONS / svg.rstrip("^"), size)
    if flip:
        cover = cover.transpose(Image.FLIP_TOP_BOTTOM)
    image = Image.new("RGB", (size, size))
    px = image.load()
    cv = cover.load()
    for y in range(size):
        for x in range(size):
            c = cv[x, y] / 255
            px[x, y] = tuple(byte(b * (1 - c) + t * c) for t, b in zip(tint, BACKGROUND))
    # the key at the top right, 14 CSS px on a 36-unit slot (hotbar.rs draw_of)
    s = size / 36
    font, advance, _, ascent = metrics(round(FONT_PX * s))
    draw = ImageDraw.Draw(image)
    text(draw, font, ascent, size - 3 * s - advance * len(key), 1 * s, key, KEY_INK)
    return image


def wrap(words, width, advance):
    """ui.rs wrap, for a monospace face."""
    lines, line = [], ""
    for word in words.split(" "):
        while True:
            candidate = word if not line else f"{line} {word}"
            if len(candidate) * advance <= width:
                line = candidate
                break
            if line:
                lines.append(line)
                line = ""
                continue
            per = max(1, int(width // advance))
            lines.append(word[:per])
            word = word[per:]
            if not word:
                break
    if line or not lines:
        lines.append(line)
    return lines


def card(title, body, details, scale):
    """tooltip.rs measure and draw at scale device px a CSS px, as an opaque picture."""
    font, advance, line_h, ascent = metrics(round(FONT_PX * scale))
    pad_x, pad_y, gap = PAD_X * scale, PAD_Y * scale, GAP * scale
    inner = MAX_WIDTH * scale - 2 * pad_x
    titles = wrap(title, inner, advance)
    bodies = wrap(body, inner, advance) if body else []
    dot = " · "
    rows = []
    for d, colour in details:
        piece = len(d) * advance
        if rows and sum(len(t) for t, _ in rows[-1]) * advance + len(dot) * advance + piece <= inner:
            rows[-1] += [(dot, SEPARATOR), (d, colour)]
        else:
            rows.append([(d, colour)])
    width = max([len(t) * advance for t in titles + bodies] + [sum(len(t) for t, _ in r) * advance for r in rows])
    lines = len(titles) + len(bodies) + len(rows)
    height = lines * line_h + (lines - 1) * gap
    w, h = math.ceil(min(width, inner) + 2 * pad_x), math.ceil(height + 2 * pad_y)
    image = Image.new("RGB", (w, h), tuple(byte(c) for c in srgb(SHADOW)))
    draw = ImageDraw.Draw(image)

    def notched(x, y, ww, hh, r, colour):
        fill = tuple(byte(c) for c in srgb(colour))
        draw.rectangle([x + r, y, x + ww - r - 1, y + hh - 1], fill=fill)
        draw.rectangle([x, y + r, x + ww - 1, y + hh - r - 1], fill=fill)

    notched(0, 0, w, h, round(2 * scale), BORDER)
    notched(round(scale), round(scale), w - 2 * round(scale), h - 2 * round(scale), round(scale), SURFACE)
    y = pad_y
    for t in titles:
        text(draw, font, ascent, pad_x, y, t, TITLE)
        text(draw, font, ascent, pad_x + scale, y, t, TITLE)
        y += line_h + gap
    for t in bodies:
        text(draw, font, ascent, pad_x, y, t, BODY)
        y += line_h + gap
    for r in rows:
        pen = pad_x
        for t, colour in r:
            text(draw, font, ascent, pen, y, t, colour)
            pen += len(t) * advance
        y += line_h + gap
    return image


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    names, cards = [], []
    for name, svg, tint, key, title, body, details in SLOTS:
        names.append(name)
        for size in SIZES:
            icon(name, svg, tint, key, size).save(OUT / f"{name}-{size}.jpg", quality=90, subsampling=0)
        for scale in SCALES:
            image = card(title, body, details, scale)
            image.save(OUT / f"{name}-card-{scale}x.jpg", quality=90, subsampling=0)
            cards.append(image.size)
    n = len(SLOTS)
    table = [
        "// Everglade's spell bar pictures (realms/everglade/hotbar/), as the previous engine drew the bar:",
        "// written by verse/tools/everglade_hotbar.py, do not edit. Each slot's name, the icon sizes",
        "// (px a side), and each slot's card at 1x, 2x and 3x (device px, slot * 3 + scale - 1).",
        f"HB_SLOTS :: {n};",
        f"HB_NAME :: [{n}] string.[{', '.join(f'\"{x}\"' for x in names)}];",
        f"HB_SIZES :: {len(SIZES)};",
        f"HB_SIZE :: [{len(SIZES)}] int.[{', '.join(str(s) for s in SIZES)}];",
        f"HB_CARD_W :: [{len(cards)}] f32.[{', '.join(str(w) for w, _ in cards)}];",
        f"HB_CARD_H :: [{len(cards)}] f32.[{', '.join(str(h) for _, h in cards)}];",
        "",
    ]
    TABLE.write_text("\n".join(table))
    print(f"{n} slots: {len(SIZES)} icon sizes and {len(SCALES)} cards each into {OUT.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
