#!/usr/bin/env python3
"""Stand-in screen footage for the studio shots, until the real recordings land.

The real footage comes from `scripts/film/footage/` into
`~/work/film-out/footage/{web-chat,terminal-coder,web-sync,phone-approve}.mp4`.
Until then this writes clips with the same names, sizes and frame rate into
`~/work/film-out/studio/standin/` from the committed screens in
`docs/verse/film/shots/` (and a live openagents.com capture when headless
Chrome is available), so `studio.py` can be framed and lit against real UI.

Usage: python3 scripts/film/studio/standin.py [OUT_DIR]
       python3 scripts/film/studio/standin.py --sync [DIR]   # shared-clock sync clips + beats.json
Needs Pillow and ffmpeg.
"""

import os
import subprocess
import sys

from PIL import Image, ImageDraw, ImageFont

REPO = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "..", ".."))
SHOTS = os.path.join(REPO, "docs", "verse", "film", "shots")
OUT = os.path.expanduser(sys.argv[1] if len(sys.argv) > 1 else "~/work/film-out/studio/standin")
FPS = 30
DESK = (3024, 1964)  # 1512x982 CSS px at 2x
PHONE = (1180, 2556)  # 393x852 CSS px at 3x, padded to an even width for yuv420p
BG = (10, 10, 10)  # the app's dark background
CHROME = os.environ.get("CHROME", "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome")


def fit(path, size):
    """Scale an image to the clip's width and pad the bottom with the app background."""
    im = Image.open(path).convert("RGB")
    w, h = size
    s = w / im.width
    im = im.resize((w, round(im.height * s)), Image.LANCZOS)
    out = Image.new("RGB", size, BG)
    out.paste(im.crop((0, 0, w, min(h, im.height))), (0, 0))
    return out


def live_desktop():
    """openagents.com at 1512x982@2x, or None without Chrome or network."""
    png = os.path.join(OUT, "oa-desktop.png")
    if not os.path.exists(png) and os.path.exists(CHROME):
        subprocess.run([CHROME, "--headless=new", "--hide-scrollbars", "--force-device-scale-factor=2",
                        "--window-size=1512,982", "--virtual-time-budget=8000", f"--screenshot={png}",
                        "https://openagents.com"], capture_output=True, timeout=60)
    return png if os.path.exists(png) else None


class Writer:
    def __init__(self, name, size):
        self.path = os.path.join(OUT, name + ".mp4")
        self.p = subprocess.Popen(
            ["ffmpeg", "-hide_banner", "-loglevel", "error", "-y", "-f", "rawvideo", "-pix_fmt", "rgb24",
             "-s", f"{size[0]}x{size[1]}", "-r", str(FPS), "-i", "-", "-c:v", "libx264", "-crf", "12",
             "-preset", "fast", "-pix_fmt", "yuv420p", self.path], stdin=subprocess.PIPE)

    def put(self, im, n=1):
        b = im.tobytes()
        for _ in range(n):
            self.p.stdin.write(b)

    def close(self):
        self.p.stdin.close()
        self.p.wait()
        print(self.path)


def slideshow(name, size, images, hold, fade=0.6):
    """Hold each image `hold` seconds with a crossfade between them."""
    w = Writer(name, size)
    nf = round(fade * FPS)
    for i, im in enumerate(images):
        w.put(im, round(hold * FPS) - (nf if i + 1 < len(images) else 0))
        if i + 1 < len(images):
            for k in range(nf):
                w.put(Image.blend(im, images[i + 1], (k + 1) / (nf + 1)))
    w.close()


def terminal(name, seconds=10):
    """A Coder session typing into a dark terminal, at 1512x982@2x."""
    W, H = DESK
    font = ImageFont.truetype("/System/Library/Fonts/Menlo.ttc", 40)
    bold = ImageFont.truetype("/System/Library/Fonts/Menlo.ttc", 40, index=1)
    green, dim, white, blue, amber = (110, 220, 140), (130, 130, 140), (225, 228, 232), (120, 170, 255), (240, 190, 90)
    script = [
        (0.0, "$ ", white, "coder \"Fix the login bug in the sample app and open a pull request\"", 1.6),
        (2.0, "", blue, "◆ Coder  ·  sample-app  ·  main", 0),
        (2.4, "", dim, "  Reading src/auth/session.ts, src/routes/login.tsx", 0),
        (3.0, "", dim, "  Found it: the session cookie is set before the redirect resolves.", 0),
        (3.8, "", amber, "  edit  src/auth/session.ts  +12 -4", 0),
        (4.4, "", amber, "  edit  src/routes/login.tsx  +3 -1", 0),
        (5.0, "", dim, "  Running tests: npm test", 0),
        (6.0, "", green, "  ✓ 48 passed  (2.1 s)", 0),
        (6.8, "", dim, "  git switch -c fix/login-redirect && git push", 0),
        (7.6, "", green, "  ✓ Opened pull request #42: Fix login redirect race", 0),
        (8.4, "", white, "  Done. The chat is on openagents.com too.", 0),
    ]
    out = Writer(name, DESK)
    for f in range(seconds * FPS):
        t = f / FPS
        im = Image.new("RGB", DESK, (14, 14, 16))
        d = ImageDraw.Draw(im)
        d.rectangle((0, 0, W, 72), fill=(28, 28, 32))
        for i, c in enumerate([(255, 95, 87), (254, 188, 46), (40, 200, 64)]):
            d.ellipse((36 + i * 44, 22, 64 + i * 44, 50), fill=c)
        d.text((W // 2, 36), "coder — sample-app — 120×38", font=font, fill=dim, anchor="mm")
        y = 120
        for start, prompt, color, text, typing in script:
            if t < start:
                break
            shown = text if typing == 0 else text[: int(len(text) * min(1, (t - start) / typing))]
            x = 48
            if prompt:
                d.text((x, y), prompt, font=bold, fill=green)
                x += d.textlength(prompt, font=bold)
            d.text((x, y), shown, font=bold if prompt else font, fill=color)
            y += 70
        if int(t * 2) % 2 == 0:
            d.rectangle((48, y + 4, 70, y + 48), fill=white)
        out.put(im)
    out.close()


# ---------------------------------------------------------------- shared-clock sync stand-ins

SYNC_LEN = 16.0
# (sender, text, typing starts, sent): every other screen shows it LAND s later.
SYNC_MESSAGES = [
    ("terminal", "Fix the flaky login test and open a PR", 1.2, 3.0),
    ("phone", "Looks good. Ship it.", 6.4, 8.0),
    ("web", "Merged. Thanks, Coder!", 10.4, 12.0),
]
LAND = 0.4
SYNC_SIZES = {"terminal": DESK, "desktop": DESK, "web": (3840, 2160), "phone": (1206, 2622)}
SANS = "/System/Library/Fonts/HelveticaNeue.ttc"


def sync_frame(screen, t, size):
    """One stand-in frame of `screen` at shared time t: the chat so far,
    with the screen's own message typing in before it is sent."""
    W, H = size
    im = Image.new("RGB", size, BG)
    d = ImageDraw.Draw(im)
    scale = W / 1512 if screen != "phone" else W / 402
    mono = screen == "terminal"
    font = ImageFont.truetype("/System/Library/Fonts/Menlo.ttc" if mono else SANS, int(15 * scale))
    head = ImageFont.truetype(SANS, int(17 * scale), index=1) if not mono else font
    label = {"terminal": "coder - ~/code/slugkit", "desktop": "OpenAgents", "web": "openagents.com",
             "phone": "OpenAgents"}[screen]
    d.text((int(24 * scale), int(18 * scale)), label, font=head, fill=(235, 235, 235))
    if screen in ("web", "desktop"):
        d.line((int(240 * scale), 0, int(240 * scale), H), fill=(40, 40, 40), width=max(1, int(scale)))
    x0 = int((260 if screen in ("web", "desktop") else 24) * scale)
    y = int(70 * scale)
    pad = int(12 * scale)
    for sender, text, typing, sent in SYNC_MESSAGES:
        shown = None
        if sender == screen and t >= typing:
            shown = text if t >= sent else text[: int(len(text) * (t - typing) / (sent - typing))]
        elif sender != screen and t >= sent + LAND:
            shown = text
        if shown is None:
            continue
        mine = sender == screen
        who = {"terminal": "Terminal", "phone": "iPhone", "web": "Web"}[sender]
        if mono:
            d.text((x0, y), ("> " if mine else f"[{who}] ") + shown, font=font,
                   fill=(225, 228, 232) if mine else (110, 220, 140))
            y += int(28 * scale)
            continue
        tw = d.textlength(shown or " ", font=font)
        bx1 = W - int(24 * scale) if mine else x0 + tw + 2 * pad
        bx0 = bx1 - tw - 2 * pad if mine else x0
        d.rounded_rectangle((bx0, y, bx1, y + int(36 * scale)), radius=int(12 * scale),
                            fill=(48, 48, 52) if mine else (26, 26, 28))
        d.text((bx0 + pad, y + int(9 * scale)), shown, font=font, fill=(240, 240, 240))
        d.text((bx0, y + int(40 * scale)), who, font=ImageFont.truetype(SANS, int(11 * scale)),
               fill=(120, 120, 120))
        y += int(70 * scale)
    # composer at the bottom, with the caret blinking while this screen types
    cy = H - int(80 * scale)
    d.rounded_rectangle((x0, cy, W - int(24 * scale), cy + int(48 * scale)), radius=int(14 * scale),
                        outline=(60, 60, 60), width=max(1, int(scale)))
    if int(t * 2) % 2 == 0:
        d.rectangle((x0 + pad, cy + int(12 * scale), x0 + pad + max(2, int(2 * scale)), cy + int(36 * scale)),
                    fill=(230, 230, 230))
    return im


def boot_frame(screen, t, size):
    """Dark, then the app's name fades in, then the home screen."""
    W, H = size
    if t < 1.0:
        return Image.new("RGB", size, (0, 0, 0))
    if t < 2.6:
        im = Image.new("RGB", size, BG)
        a = min(1.0, (t - 1.0) / 0.6)
        d = ImageDraw.Draw(im)
        f = ImageFont.truetype(SANS, int(W / (8 if screen == "phone" else 14)), index=1)
        v = int(235 * a)
        d.text((W // 2, H // 2), "OpenAgents", font=f, fill=(v, v, v), anchor="mm")
        return im
    return sync_frame(screen, -1.0, size)


def sync_standins(out):
    """Stand-ins for ~/work/film-out/sync-v1 on one clock, plus beats.json."""
    import json

    os.makedirs(out, exist_ok=True)
    for screen, size in SYNC_SIZES.items():
        for name, fn, dur in ((screen, sync_frame, SYNC_LEN), (f"boot-{screen}", boot_frame, 5.0)):
            w = Writer.__new__(Writer)
            w.path = os.path.join(out, name + ".mp4")
            w.p = subprocess.Popen(
                ["ffmpeg", "-hide_banner", "-loglevel", "error", "-y", "-f", "rawvideo", "-pix_fmt", "rgb24",
                 "-s", f"{size[0]}x{size[1]}", "-r", str(FPS), "-i", "-", "-c:v", "libx264", "-crf", "16",
                 "-preset", "fast", "-pix_fmt", "yuv420p", w.path], stdin=subprocess.PIPE)
            last, cached = None, None
            for f in range(int(dur * FPS)):
                t = f / FPS
                key = (int(t * 2), int(t * 15))  # redraw at most 15 times a second
                if key != last:
                    cached, last = fn(screen, t, size), key
                w.put(cached)
            w.close()
    beats = {"stand_in": True, "clock": "seconds from the start of every clip",
             "beats": [{"from": s, "text": text, "typing": ty, "sent": se, "lands": se + LAND}
                       for s, text, ty, se in SYNC_MESSAGES],
             "boot": {"dark_until": 1.0, "home_at": 2.6}}
    with open(os.path.join(out, "beats.json"), "w") as f:
        json.dump(beats, f, indent=2)
    print(os.path.join(out, "beats.json"))


def main():
    if len(sys.argv) > 1 and sys.argv[1] == "--sync":
        sync_standins(os.path.expanduser(sys.argv[2] if len(sys.argv) > 2 else "~/work/film-out/studio/standin/sync"))
        return
    os.makedirs(OUT, exist_ok=True)
    s = lambda n: os.path.join(SHOTS, n)
    live = live_desktop()
    first = fit(live, DESK) if live else fit(s("home-desktop.png"), DESK)
    slideshow("web-chat", DESK, [first, fit(s("chat-desktop.png"), DESK)], hold=5)
    terminal("terminal-coder")
    slideshow("web-sync", DESK, [fit(s("work-desktop.png"), DESK), fit(s("chat-desktop.png"), DESK)], hold=5)
    slideshow("phone-approve", PHONE, [fit(s("device-phone.png"), PHONE), fit(s("signin-phone.png"), PHONE)], hold=5)


if __name__ == "__main__":
    main()
