#!/usr/bin/env python3
"""Stand-in screen footage for the studio shots, until the real recordings land.

The real footage comes from `scripts/film/footage/` into
`~/work/film-out/footage/{web-chat,terminal-coder,web-sync,phone-approve}.mp4`.
Until then this writes clips with the same names, sizes and frame rate into
`~/work/film-out/studio/standin/` from the committed screens in
`docs/verse/film/shots/` (and a live openagents.com capture when headless
Chrome is available), so `studio.py` can be framed and lit against real UI.

Usage: python3 scripts/film/studio/standin.py [OUT_DIR]
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


def main():
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
