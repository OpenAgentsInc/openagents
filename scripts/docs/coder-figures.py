#!/usr/bin/env python3
"""The four figures on the "How Coder works" docs page, as static SVG.

  scripts/docs/coder-figures.py [OUT_DIR]

OUT_DIR defaults to crates/openagents-web/static/docs/. The site serves
each file at /static/docs/<name>.svg and the guide
(crates/openagents-web/content/docs/coder-how-it-works.md) draws them.

The look is the Coder v1 launch film's route map (scripts/film/
system1-routing.py): the noir canvas 0a0a0a, text ededed / 818181 /
656565, thin white edges at low alpha, the request light (236,240,255)
with a soft glow, and the payment gold (255,206,84). Each figure is its
own dark panel, so it reads the same on the light and the dark theme.
Text uses the site's own font stacks (system UI and system mono), which
an SVG drawn as an image can reach. Light moves with CSS alone; under
prefers-reduced-motion the moving light is hidden and the figure holds
still. No scripts, no outside files.
"""

import math
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
OUT = Path(sys.argv[1]) if len(sys.argv) > 1 else REPO / "crates/openagents-web/static/docs"

W, H = 640, 420
CANVAS = "#0a0a0a"
LIGHT = "rgb(236,240,255)"
GOLD = "rgb(255,206,84)"
SANS = 'ui-sans-serif,-apple-system,system-ui,"Segoe UI","Noto Sans",Helvetica,Arial,sans-serif'
MONO = 'ui-monospace,"SFMono-Regular","SF Mono",Menlo,Monaco,Consolas,"Liberation Mono",monospace'


def esc(text):
    return text.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")


def polar(cx, cy, r, deg):
    a = math.radians(deg)
    return cx + r * math.cos(a), cy + r * math.sin(a)


class Figure:
    """One SVG: shapes in layers, and a keyframe per moving light."""

    def __init__(self, name, title, desc, cycle):
        self.name, self.title, self.desc, self.cycle = name, title, desc, cycle
        self.back, self.mid, self.front, self.keyframes = [], [], [], []
        self.n = 0

    # --- shapes ----------------------------------------------------------
    def edge(self, d, alpha=0.15, width=1.0, dash=None, layer=None):
        extra = f' stroke-dasharray="{dash}"' if dash else ""
        (layer if layer is not None else self.back).append(
            f'<path d="{d}" fill="none" stroke="#fff" stroke-opacity="{alpha}" '
            f'stroke-width="{width}"{extra}/>'
        )

    def ring(self, cx, cy, r, alpha=0.18, width=1.0, dash=None):
        extra = f' stroke-dasharray="{dash}"' if dash else ""
        self.back.append(
            f'<circle cx="{cx:.1f}" cy="{cy:.1f}" r="{r}" fill="none" stroke="#fff" '
            f'stroke-opacity="{alpha}" stroke-width="{width}"{extra}/>'
        )

    def dot(self, x, y, r, gray=220, alpha=1.0, layer=None, ring=False):
        fill = f"rgb({gray},{gray},{gray})"
        out = layer if layer is not None else self.mid
        if ring:
            out.append(
                f'<circle cx="{x:.1f}" cy="{y:.1f}" r="{r + 3.5}" fill="none" stroke="#fff" '
                f'stroke-opacity="0.22" stroke-width="1"/>'
            )
        out.append(f'<circle cx="{x:.1f}" cy="{y:.1f}" r="{r}" fill="{fill}" fill-opacity="{alpha}"/>')

    def text(self, x, y, s, size=13, cls="t2", anchor="start", weight=400, mono=False, layer=None):
        fam = ' class="{} mono"'.format(cls) if mono else f' class="{cls}"'
        w = f' font-weight="{weight}"' if weight != 400 else ""
        (layer if layer is not None else self.front).append(
            f'<text x="{x:.1f}" y="{y:.1f}" font-size="{size}" text-anchor="{anchor}"{w}{fam}>{esc(s)}</text>'
        )

    def core(self, cx, cy, r, line1, line2=None, bright=1.0):
        """The bright node with dark text: the film's big model, or Coder."""
        self.mid.append(
            f'<circle cx="{cx}" cy="{cy}" r="{r * 2.1:.1f}" fill="url(#halo)" opacity="{0.55 * bright:.2f}" class="breathe"/>'
        )
        self.mid.append(
            f'<circle cx="{cx}" cy="{cy}" r="{r + 7}" fill="none" stroke="#fff" stroke-opacity="0.25"/>'
        )
        self.mid.append(f'<circle cx="{cx}" cy="{cy}" r="{r}" fill="url(#core)" opacity="{bright}"/>')
        if line2:
            self.text(cx, cy - 1, line1, size=max(12, r * 0.42), cls="ink", anchor="middle", weight=500)
            self.text(cx, cy + r * 0.38, line2, size=max(10, r * 0.27), cls="ink2", anchor="middle")
        else:
            self.text(cx, cy + r * 0.15, line1, size=max(12, r * 0.42), cls="ink", anchor="middle", weight=500)

    def light(self, d, length, start, dur, color=LIGHT, width=1.6, tail=16):
        """A short streak of light that runs along `d` from `start` for `dur`
        seconds of the figure's cycle, then waits for the next cycle."""
        self.n += 1
        k = f"k{self.n}"
        a = start / self.cycle * 100
        b = min(100.0, (start + dur) / self.cycle * 100)
        self.keyframes.append(
            f"@keyframes {k}{{0%,{a:.2f}%{{stroke-dashoffset:{tail}}}"
            f"{b:.2f}%,100%{{stroke-dashoffset:{-length - 1:.1f}}}}}"
        )
        anim = f"animation:{k} {self.cycle}s linear infinite"
        dash = f"stroke-dasharray:{tail} {length + tail + 40:.0f};stroke-dashoffset:{tail}"
        self.front.append(
            f'<path class="light" d="{d}" fill="none" stroke="{color}" stroke-width="{width * 4}" '
            f'stroke-linecap="round" stroke-opacity="0.35" filter="url(#soft)" style="{dash};{anim}"/>'
        )
        self.front.append(
            f'<path class="light" d="{d}" fill="none" stroke="{color}" stroke-width="{width}" '
            f'stroke-linecap="round" style="{dash};{anim}"/>'
        )

    def flash(self, cx, cy, r, start, dur, color=LIGHT):
        """A glow that comes up at `start` and fades over `dur` seconds."""
        self.n += 1
        k = f"k{self.n}"
        a = start / self.cycle * 100
        m = min(100.0, (start + dur * 0.25) / self.cycle * 100)
        b = min(100.0, (start + dur) / self.cycle * 100)
        self.keyframes.append(
            f"@keyframes {k}{{0%,{a:.2f}%{{opacity:0}}{m:.2f}%{{opacity:1}}{b:.2f}%,100%{{opacity:0}}}}"
        )
        self.front.append(
            f'<circle class="light" cx="{cx:.1f}" cy="{cy:.1f}" r="{r}" fill="none" stroke="{color}" '
            f'stroke-width="1.4" style="opacity:0;animation:{k} {self.cycle}s ease-out infinite"/>'
        )
        self.front.append(
            f'<circle class="light" cx="{cx:.1f}" cy="{cy:.1f}" r="{r * 2.2:.1f}" fill="url(#halo)" '
            f'style="opacity:0;animation:{k} {self.cycle}s ease-out infinite"/>'
        )

    # --- output ----------------------------------------------------------
    def svg(self):
        style = (
            f"text{{font-family:{SANS};letter-spacing:-.01em}}"
            f".mono{{font-family:{MONO};letter-spacing:0}}"
            ".t1{fill:#ededed}.t2{fill:#818181}.t3{fill:#656565}"
            ".ink{fill:#0a0a0a}.ink2{fill:#3a3a3a}.gold{fill:" + GOLD + "}"
            f"@keyframes breathe{{0%,100%{{opacity:.45}}50%{{opacity:.7}}}}"
            f".breathe{{animation:breathe {self.cycle}s ease-in-out infinite}}"
            + "".join(self.keyframes)
            + "@media (prefers-reduced-motion:reduce){.light{display:none}.breathe{animation:none}}"
        )
        defs = (
            '<radialGradient id="core"><stop offset="0" stop-color="#fff"/>'
            '<stop offset=".7" stop-color="rgb(240,242,250)"/>'
            '<stop offset="1" stop-color="rgb(210,214,228)"/></radialGradient>'
            '<radialGradient id="halo"><stop offset="0" stop-color="rgb(236,240,255)" stop-opacity=".55"/>'
            '<stop offset=".45" stop-color="rgb(236,240,255)" stop-opacity=".12"/>'
            '<stop offset="1" stop-color="rgb(236,240,255)" stop-opacity="0"/></radialGradient>'
            '<radialGradient id="goldhalo"><stop offset="0" stop-color="rgb(255,206,84)" stop-opacity=".5"/>'
            '<stop offset="1" stop-color="rgb(255,206,84)" stop-opacity="0"/></radialGradient>'
            '<radialGradient id="vignette" cx=".62" cy=".5" r=".75"><stop offset="0" stop-color="#141414"/>'
            f'<stop offset="1" stop-color="{CANVAS}"/></radialGradient>'
            '<filter id="soft" x="-50%" y="-50%" width="200%" height="200%">'
            '<feGaussianBlur stdDeviation="2.4"/></filter>'
        )
        body = "\n".join(self.back + self.mid + self.front)
        return (
            f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {W} {H}" width="{W}" height="{H}" '
            f'role="img" aria-labelledby="title desc">\n'
            f'<title id="title">{esc(self.title)}</title>\n<desc id="desc">{esc(self.desc)}</desc>\n'
            f"<defs>{defs}</defs>\n<style>{style}</style>\n"
            f'<rect width="{W}" height="{H}" fill="url(#vignette)"/>\n{body}\n'
            f'<rect x=".5" y=".5" width="{W - 1}" height="{H - 1}" fill="none" stroke="#fff" stroke-opacity=".08"/>\n</svg>\n'
        )


def curve(x1, y1, x2, y2, bend=0.0):
    """A gentle quadratic from (x1,y1) to (x2,y2), and its length."""
    mx, my = (x1 + x2) / 2, (y1 + y2) / 2
    dx, dy = x2 - x1, y2 - y1
    cx, cy = mx - dy * bend, my + dx * bend
    d = f"M{x1:.1f},{y1:.1f} Q{cx:.1f},{cy:.1f} {x2:.1f},{y2:.1f}"
    # length by sampling
    n, length, px, py = 40, 0.0, x1, y1
    for i in range(1, n + 1):
        t = i / n
        x = (1 - t) ** 2 * x1 + 2 * (1 - t) * t * cx + t * t * x2
        y = (1 - t) ** 2 * y1 + 2 * (1 - t) * t * cy + t * t * y2
        length += math.hypot(x - px, y - py)
        px, py = x, y
    return d, length


def line(x1, y1, x2, y2):
    return f"M{x1:.1f},{y1:.1f} L{x2:.1f},{y2:.1f}", math.hypot(x2 - x1, y2 - y1)


def joined(*parts):
    """Paths end to end as one path (each part starts where the last ended)."""
    d = parts[0][0]
    for p in parts[1:]:
        d += " " + p[0].split(" ", 1)[1]
    return d, sum(p[1] for p in parts)


def headline(f, lines):
    for i, s in enumerate(lines):
        f.text(28, 46 + i * 26, s, size=20, cls="t1")


def counters(f, cost, time, was=None):
    f.text(28, 318, "cost per accepted fix", size=11, cls="t3")
    f.text(188, 318, "median time", size=11, cls="t3")
    f.text(28, 350, cost, size=28, cls="t1", mono=True)
    f.text(188, 350, time, size=28, cls="t1", mono=True)
    if was:
        for x, s in ((28, was[0]), (188, was[1])):
            f.text(x, 376, s, size=14, cls="t3", mono=True)
            w = len(s) * 8.4
            f.front.append(
                f'<line x1="{x - 1}" y1="371" x2="{x + w + 1:.1f}" y2="371" stroke="#656565" stroke-width="1"/>'
            )


def messages(f, items, x_dot=206, y0=150, step=32):
    spots = []
    for i, (s, bright) in enumerate(items):
        y = y0 + i * step
        f.text(28, y + 4.5, s, size=14, cls="t1" if bright else "t2")
        f.dot(x_dot, y, 2.6, gray=235 if bright else 150)
        spots.append((x_dot, y))
    return spots


# --- 1. Another harness ------------------------------------------------------
def harness():
    f = Figure(
        "coder-harness",
        "Another harness: every step goes through the big model",
        "Three messages on the left each run straight into one large glowing "
        "circle labeled the big model. A thin ring around it holds only the "
        "model's own commands: Bash, Read, Edit, Write, Grep, and Web. Every "
        "command's result goes back into the model. Below: $4.00 per accepted "
        "fix and a 284 second median.",
        cycle=7.0,
    )
    cx, cy = 440, 210
    headline(f, ["Another harness: every step", "goes through the big model."])
    spots = messages(
        f,
        [("Fix the flaky parser test", True), ("Find where the config loads", True), ("What does this crate do?", False)],
    )
    counters(f, "$4.00", "284 s")
    ring_r = 92
    f.ring(cx, cy, ring_r, alpha=0.22)
    f.ring(cx, cy, ring_r + 6, alpha=0.08)
    commands = ["Read", "Edit", "Write", "Grep", "Web", "Bash"]
    nodes = []
    for i, name in enumerate(commands):
        deg = -30 + i * 60
        x, y = polar(cx, cy, ring_r, deg)
        nodes.append((x, y, deg))
        ix, iy = polar(cx, cy, 60, deg)
        f.edge(line(ix, iy, x, y)[0], alpha=0.25)
        f.dot(x, y, 5, gray=225)
        lx, ly = polar(cx, cy, ring_r + 22, deg)
        anchor = "start" if math.cos(math.radians(deg)) > 0.3 else ("end" if math.cos(math.radians(deg)) < -0.3 else "middle")
        f.text(lx, ly + 4.5, name, size=13, cls="t2", anchor=anchor)
    f.core(cx, cy, 56, "System 2", "the big model")
    # each message runs into the model; the model runs out to a command and back
    t = 0.2
    for i, (mx, my) in enumerate(spots):
        ex, ey = polar(cx, cy, 60, 180 + (i - 1) * 9)
        d = curve(mx + 4, my, ex, ey, bend=0.04 * (i - 1))
        f.edge(d[0], alpha=0.13 if i < 2 else 0.08)
        f.light(d[0], d[1], start=t, dur=1.1)
        f.flash(cx, cy, 58, t + 1.0, 0.9)
        for hop in (i * 2, i * 2 + 1):
            x, y, deg = nodes[(hop * 2 + i) % 6]
            ix, iy = polar(cx, cy, 58, deg)
            out = line(ix, iy, x, y)
            back = line(x, y, ix, iy)
            f.light(joined(out, back)[0], out[1] * 2, start=t + 1.2 + (hop - i * 2) * 0.55, dur=0.55, width=1.3, tail=10)
        t += 2.2
    return f


# --- 2. System 1 in front -----------------------------------------------------
def system_one():
    f = Figure(
        "coder-system-1",
        "Coder: System 1 in front, only the hard parts go to the big model",
        "The same three messages now stop at a ring of quick decisions around "
        "a smaller big model: route, brief, trim, gate, notes, and check. Two "
        "messages hop out to nearby answers on an outer ring, such as your "
        "code and the commands, without reaching the model. Only the fix goes "
        "on to the center. Below: $1.84 per accepted fix instead of $4.00, and "
        "a 121 second median instead of 284.",
        cycle=7.0,
    )
    cx, cy = 432, 210
    headline(f, ["Put System 1 in front.", "Send only the hard parts on."])
    spots = messages(
        f,
        [("Fix the flaky parser test", True), ("Run the tests", True), ("What does this crate do?", True)],
    )
    counters(f, "$1.84", "121 s", was=("$4.00", "284 s"))
    # the outer dotted ring of the route map
    for i in range(120):
        if math.cos(math.radians(i * 3)) < -0.45:
            continue  # leave the left open for the messages coming in
        x, y = polar(cx, cy, 192, i * 3)
        f.back.append(f'<circle cx="{x:.1f}" cy="{y:.1f}" r="0.9" fill="#fff" fill-opacity="0.22"/>')
    outer_r = 150
    outer = [
        ("Commands", 152), ("Your code", 208), ("Ask back", 246), ("Answers", 282), ("Knowledge", 318),
        ("Wallet", 352), ("Checks", 26), ("Tasks", 62), ("Plugins", 96), ("Decks", 124),
    ]
    places = {}
    for name, deg in outer:
        x, y = polar(cx, cy, outer_r, deg)
        places[name] = (x, y)
        f.edge(line(*polar(cx, cy, 100, deg), x, y)[0], alpha=0.12)
        # a few leaves past each place, as on the map
        for j in (-1, 1) if math.cos(math.radians(deg)) > -0.35 else ():
            lx, ly = polar(cx, cy, 178, deg + j * 5)
            f.edge(line(x, y, lx, ly)[0], alpha=0.07)
            f.dot(lx, ly, 1.5, gray=110, layer=f.back)
        f.dot(x, y, 4, gray=190)
        c, s_ = math.cos(math.radians(deg)), math.sin(math.radians(deg))
        if abs(c) > 0.35:
            tx, ty = x + (10 if c > 0 else -10), y + 4
            anchor = "start" if c > 0 else "end"
        else:
            tx, ty = x, y + (17 if s_ > 0 else -9)
            anchor = "middle"
        f.text(tx, ty, name, size=11.5, cls="t3", anchor=anchor)
    ring_r = 92
    f.ring(cx, cy, ring_r, alpha=0.55, width=1.4)
    decisions = [("route", 180), ("brief", 240), ("trim", 300), ("gate", 0), ("notes", 60), ("check", 120)]
    nodes = {}
    for name, deg in decisions:
        x, y = polar(cx, cy, ring_r, deg)
        nodes[name] = (x, y)
        f.dot(x, y, 5.5, gray=240, ring=True)
        lx, ly = polar(cx, cy, ring_r - 20, deg)
        c = math.cos(math.radians(deg))
        anchor = "end" if c > 0.3 else ("start" if c < -0.3 else "middle")
        f.text(lx, ly + 4, name, size=11.5, cls="t1", anchor=anchor)
    f.text(cx, cy - ring_r - 12, "System 1", size=14, cls="t1", anchor="middle", weight=500)
    f.core(cx, cy, 36, "System 2", "the big model", bright=0.9)
    # every message lands on the ring, then takes a short hop
    plan = [(1, "Commands"), (2, "Your code"), (0, None)]
    nx, ny = nodes["route"]
    t = 0.2
    for src, dest in plan:
        mx, my = spots[src]
        inbound = curve(mx + 4, my, nx - 8, ny, bend=0.03 * (src - 1))
        f.edge(inbound[0], alpha=0.14)
        f.light(inbound[0], inbound[1], start=t, dur=0.8)
        f.flash(nx, ny, 7, t + 0.75, 0.7)
        if dest:
            dx, dy = places[dest]
            hop = curve(nx, ny, dx, dy, bend=0.25 if dy > ny else -0.25)
            f.edge(hop[0], alpha=0.22)
            f.light(hop[0], hop[1], start=t + 0.85, dur=0.45, tail=12)
            f.flash(dx, dy, 6, t + 1.25, 0.9)
        else:
            ix, iy = polar(cx, cy, 38, 180)
            hard = line(nx + 6, ny, ix, iy)
            f.light(hard[0], hard[1], start=t + 0.85, dur=0.35, tail=10)
            f.flash(cx, cy, 38, t + 1.15, 1.4)
        t += 2.1
    return f


# --- 3. One Coder in front of every agent -------------------------------------
def agents():
    f = Figure(
        "coder-agents",
        "One Coder in front of every agent",
        "Coder sits at the center as a bright node inside a ring marked Auto. "
        "Around it are the agents it can hand a chat to: Claude Code, Codex, "
        "OpenCode, Grok Build, Devin, and Cursor. A light steps from one agent to "
        "the next, as Shift+Tab does: Auto, then Claude Code, Codex, "
        "OpenCode, and the others found on your computer.",
        cycle=7.0,
    )
    cx, cy = 408, 206
    headline(f, ["One Coder in front", "of every agent."])
    f.text(28, 128, "Your message", size=11, cls="t3")
    f.text(28, 150, "Add tests for the login page", size=13, cls="t1")
    f.dot(222, 146, 2.6, gray=235)
    # the Shift+Tab keycap and the order it steps through
    f.front.append(
        '<rect x="28.5" y="322.5" width="86" height="28" rx="5" fill="#141414" stroke="#fff" stroke-opacity="0.28"/>'
    )
    f.text(71.5, 341, "Shift+Tab", size=12.5, cls="t1", anchor="middle", mono=True)
    f.text(126, 341, "chooses who answers", size=12, cls="t2")
    f.text(28, 374, "Auto → Claude Code → Codex →", size=12, cls="t3")
    f.text(28, 392, "OpenCode → the others here", size=12, cls="t3")
    ring_r = 62
    f.ring(cx, cy, ring_r, alpha=0.5, width=1.3)
    f.ring(cx, cy, 140, alpha=0.07, dash="1 5")
    ax, ay = polar(cx, cy, ring_r, -90)
    f.front.append(
        f'<rect x="{ax - 22:.1f}" y="{ay - 10:.1f}" width="44" height="20" rx="10" fill="{CANVAS}" '
        'stroke="#fff" stroke-opacity="0.45"/>'
    )
    f.text(ax, ay + 4.2, "Auto", size=11.5, cls="t1", anchor="middle")
    f.core(cx, cy, 34, "Coder")
    names = ["Claude Code", "Codex", "OpenCode", "Grok Build", "Devin", "Cursor"]
    degs = [-62, -24, 14, 52, 92, 132]
    agent_r = 140
    step = f.cycle / len(names)
    inbound = curve(226, 146, *polar(cx, cy, ring_r + 2, 180), bend=-0.06)
    f.edge(inbound[0], alpha=0.15)
    for i, (name, deg) in enumerate(zip(names, degs)):
        x, y = polar(cx, cy, agent_r, deg)
        ix, iy = polar(cx, cy, ring_r + 4, deg)
        spoke = line(ix, iy, x, y)
        f.edge(spoke[0], alpha=0.2)
        f.dot(x, y, 6.5, gray=215, ring=True)
        c = math.cos(math.radians(deg))
        lx, ly = polar(cx, cy, agent_r + 18, deg)
        sn = math.sin(math.radians(deg))
        if sn > 0.6:
            f.text(x, y + 26, name, size=13, cls="t1", anchor="middle")
        else:
            anchor = "start" if c > 0.2 else ("end" if c < -0.2 else "middle")
            f.text(lx, ly + 4.5, name, size=13, cls="t1", anchor=anchor)
        t = i * step
        f.light(inbound[0], inbound[1], start=t, dur=0.45, tail=12)
        f.light(spoke[0], spoke[1], start=t + 0.45, dur=0.4, tail=12)
        f.flash(x, y, 9, t + 0.8, step * 0.9)
    return f


# --- 4. Safety holds -----------------------------------------------------------
def holds():
    f = Figure(
        "coder-holds",
        "Hard-to-undo steps wait for you",
        "From Coder on the left, everyday work runs straight through to its "
        "places: read, edit, run tests, and push. Five other paths stop at a "
        "hold before they arrive: a deploy, a merge, a secret, a payment "
        "(drawn in gold), and a DNS change. Each one waits for you to answer "
        "Y to allow it or N to deny it.",
        cycle=7.0,
    )
    headline(f, ["Hard-to-undo steps", "wait for you."])
    cx, cy = 128, 238
    f.core(cx, cy, 30, "Coder")
    f.text(368, 92, "goes ahead", size=11, cls="t3")
    f.text(368, 212, "waits for you", size=11, cls="t3")
    free = ["read", "edit", "run tests", "push"]
    t = 0.2
    for i, name in enumerate(free):
        x, y = 470, 112 + i * 24
        d = curve(cx + 32, cy - 6, x - 6, y, bend=-0.08)
        f.edge(d[0], alpha=0.16)
        f.dot(x, y, 4.5, gray=220)
        f.text(x + 14, y + 4.5, name, size=13, cls="t2")
        f.light(d[0], d[1], start=t + i * 0.35, dur=0.9, tail=14)
        f.flash(x, y, 6, t + i * 0.35 + 0.85, 0.6)
    held = [("deploy", False), ("merge", False), ("secrets", False), ("payment", True), ("DNS", False)]
    hold_x = 452
    t = 2.4
    for i, (name, gold) in enumerate(held):
        x, y = 520, 236 + i * 30
        color = GOLD if gold else LIGHT
        d = curve(cx + 32, cy + 6, hold_x - 4, y, bend=0.06)
        f.edge(d[0], alpha=0.16)
        f.edge(line(hold_x + 6, y, x - 8, y)[0], alpha=0.35, dash="2 4")
        # the hold: a short bar across the path
        bar = GOLD if gold else "#ededed"
        f.mid.append(
            f'<line x1="{hold_x}" y1="{y - 8}" x2="{hold_x}" y2="{y + 8}" stroke="{bar}" '
            f'stroke-width="1.6" stroke-linecap="round" stroke-opacity="0.85"/>'
        )
        f.dot(x, y, 4.5, gray=150)
        f.text(x + 14, y + 4.5, name, size=13, cls="gold" if gold else "t1")
        start = t + i * 0.45
        f.light(d[0], d[1], start=start, dur=0.95, color=color, tail=14)
        f.flash(hold_x, y, 7, start + 0.9, 3.2, color=color)
    # the answer
    for i, (key, word) in enumerate((("Y", "allow, this once"), ("N", "deny"))):
        y = 340 + i * 32
        f.front.append(
            f'<rect x="28.5" y="{y - 17.5}" width="24" height="24" rx="4" fill="#141414" '
            'stroke="#fff" stroke-opacity="0.28"/>'
        )
        f.text(40.5, y - 1, key, size=12.5, cls="t1", anchor="middle", mono=True)
        f.text(62, y - 1, word, size=12.5, cls="t2")
    return f


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    for make in (harness, system_one, agents, holds):
        fig = make()
        path = OUT / f"{fig.name}.svg"
        path.write_text(fig.svg())
        print(path, path.stat().st_size, "bytes")


if __name__ == "__main__":
    main()
