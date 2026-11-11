"""Episode 294 cut: the Grid backdrop, the footage in a large window on the left (about 80% of
the height), the talk as a tall card on the right with its top and bottom 10% cut off, captions
under the footage, the title and links under the card. Adapted from Episode 293's assemble.py
(captions from whisper word times, timed PNG tracks, rounded masked cards, x264 crf 19, AAC 192k).

    python3 assemble.py cut1.json OUT.mp4

Encodes run one at a time (the Mac's file-table guard): each segment, then the final pass.
"""
import json, os, re, subprocess, sys
from PIL import Image, ImageDraw, ImageFont

# Footage, stills and the work folder live outside git, in EP_DIR (default ~/work/film-out/ep294).
HERE = os.path.expanduser(os.environ.get("EP_DIR", "~/work/film-out/ep294"))
W, H, FPS = 1920, 1080, 30
FONTS = os.path.expanduser("~/work/film-out/fonts")
WORK = os.path.join(HERE, "work")
os.makedirs(WORK, exist_ok=True)

# Layout. The footage window: a 1280x864 box on the left. The talk card: 480x683 on the right.
BX, BY, BW, BH = 56, 48, 1280, 864
PW, PH = 480, 683
PX, PY = W - 56 - PW, BY + (BH - PH) // 2
R, EDGE = 22, 3
EDGE_RGB = (210, 222, 232)
ORANGE = (247, 147, 26)


def run(args):
    r = subprocess.run(args, capture_output=True, text=True)
    if r.returncode:
        sys.exit("failed: " + " ".join(args) + "\n" + r.stderr[-3000:])


def dur(path):
    return float(subprocess.check_output(["ffprobe", "-v", "error", "-show_entries", "format=duration",
                                          "-of", "csv=p=0", path], text=True).strip())


def size(path):
    s = subprocess.check_output(["ffprobe", "-v", "error", "-select_streams", "v:0", "-show_entries",
                                 "stream=width,height", "-of", "csv=p=0", path], text=True).strip().split(",")
    return int(s[0]), int(s[1])


def font(name, px):
    return ImageFont.truetype(os.path.join(FONTS, name), px)


def fit(w, h):
    k = min(BW / w, BH / h)
    fw, fh = int(w * k) // 2 * 2, int(h * k) // 2 * 2
    return fw, fh, BX + (BW - fw) // 2, BY + (BH - fh) // 2


def masks(fw, fh, tag=None):
    """The window's rounded mask and its edge-and-shadow plate (full frame, with the tag)."""
    mk = os.path.join(WORK, f"mask-{fw}x{fh}.png")
    m = Image.new("L", (fw, fh), 0); ImageDraw.Draw(m).rounded_rectangle([0, 0, fw - 1, fh - 1], R, fill=255); m.save(mk)
    return mk


def plate(path, x, y, fw, fh, tag=None, placeholder=False):
    """Transparent full frame: a soft shadow and a lit edge round the window, and its tag."""
    im = Image.new("RGBA", (W, H), (0, 0, 0, 0))
    d = ImageDraw.Draw(im)
    for k in range(14, 0, -2):
        d.rounded_rectangle([x - k, y - k + 6, x + fw + k, y + fh + k + 6], R + k, fill=(0, 0, 0, 10))
    d.rounded_rectangle([x - EDGE, y - EDGE, x + fw + EDGE - 1, y + fh + EDGE - 1], R + EDGE, outline=EDGE_RGB + (255,), width=EDGE)
    if tag:
        f = font("GeistMono-Medium.ttf", 22)
        tw = d.textlength(tag, font=f)
        d.rounded_rectangle([x + 18, y + fh - 62, x + 18 + tw + 36, y + fh - 18], 10, fill=(8, 10, 14, 242), outline=(70, 78, 90, 255), width=1)
        d.text((x + 36, y + fh - 54), tag, font=f, fill=(235, 238, 242, 255))
    if placeholder:
        f = font("GeistMono-Medium.ttf", 24)
        t = "PLACEHOLDER"
        tw = d.textlength(t, font=f)
        d.rounded_rectangle([x + fw - tw - 52, y + 18, x + fw - 18, y + 60], 10, fill=(200, 40, 40, 230))
        d.text((x + fw - tw - 36, y + 24), t, font=f, fill=(255, 255, 255, 255))
    im.save(path)


def card(path, lines):
    """A question card in the window: dark glass, Geist, one line per spoken question."""
    im = Image.new("RGBA", (BW, BH), (0, 0, 0, 0))
    d = ImageDraw.Draw(im)
    d.rounded_rectangle([0, 0, BW - 1, BH - 1], R, fill=(10, 13, 18, 228))
    f = font("Geist-Regular.ttf", 76)
    y = BH / 2 - len(lines) * 55
    for t, col in lines:
        if len(col) == 4 and col[3] == 0: y += 110; continue
        w = d.textlength(t, font=f)
        d.text(((BW - w) / 2, y), t, font=f, fill=col)
        y += 110
    im.save(path)


def grid_loop():
    """The Grid plate (studio/grid-plate.mp4, 4K, 14 s), at 1080p, played forward then back: 28 s."""
    out = os.path.join(WORK, "grid-loop.mp4")
    if os.path.exists(out): return out
    src = os.path.expanduser("~/work/film-out/studio/grid-plate.mp4")
    run(["ffmpeg", "-v", "error", "-y", "-i", src, "-filter_complex",
         f"[0:v]scale={W}:{H},fps={FPS},split[a][b];[b]reverse[r];[a][r]concat=n=2:v=1,eq=brightness=-0.02,format=yuv420p[v]",
         "-map", "[v]", "-c:v", "libx264", "-preset", "veryfast", "-crf", "16", out])
    return out


def segment(i, s, total, grid):
    a, b = s["at"], s.get("to") or total
    length = b - a
    out = os.path.join(WORK, f"seg{i:02d}.mp4")
    src = os.path.join(HERE, os.path.expanduser(s.get("src", "") or "."))
    kind = s.get("kind", "video")
    gin = ["-ss", f"{a % 28:.3f}", "-stream_loop", "-1", "-i", grid]
    if kind == "card":
        png = os.path.join(WORK, f"card{i:02d}.png")
        layers = []
        # one PNG per reveal: lines appear at their spoken times
        for k in range(len(s["lines"])):
            p = os.path.join(WORK, f"card{i:02d}-{k}.png")
            card(p, [(t, (240, 242, 245) if j <= k else (0, 0, 0, 0)) for j, (t, _) in enumerate(s["lines"])])
            layers.append(p)
        fw, fh, x, y = BW, BH, BX, BY
        pl = os.path.join(WORK, f"plate{i:02d}.png"); plate(pl, x, y, fw, fh, s.get("tag"))
        mk = masks(fw, fh)
        ins = gin[:]
        for p in layers: ins += ["-loop", "1", "-i", p]
        ins += ["-loop", "1", "-i", mk, "-loop", "1", "-i", pl]
        n = len(layers)
        fc = []
        prev = "[0:v]"
        for k in range(n):
            t0 = s["lines"][k][1] - a
            t1 = s["lines"][k + 1][1] - a if k + 1 < n else length + 1
            fc.append(f"[{k + 1}:v]format=rgba[c{k}];{prev}[c{k}]overlay={x}:{y}:enable='between(t,{max(t0, 0):.3f},{t1:.3f})'[g{k}]")
            prev = f"[g{k}]"
        fc.append(f"{prev}[{n + 2}:v]overlay=0:0,format=yuv420p[v]")
        run(["ffmpeg", "-v", "error", "-y"] + ins + ["-filter_complex", ";".join(fc), "-map", "[v]", "-t", f"{length:.3f}",
             "-r", str(FPS), "-c:v", "libx264", "-preset", "veryfast", "-crf", "17", out])
        return out
    if kind == "scroll":   # a tall still, scrolled from y0 to y1 over the segment
        w, h = Image.open(src).size
        fw, fh = BW, BH
        x, y = BX, BY
        y0, y1 = s.get("y0", 0), s.get("y1", h - BH)
        content = (f"[1:v]crop={BW}:{BH}:0:'{y0}+({y1}-{y0})*min(t/{length:.3f},1)',format=rgba[c]")
        cin = ["-loop", "1", "-framerate", str(FPS), "-i", src]
    elif kind == "still":  # a still with a slow push in toward a rectangle (x, y, w, h in source pixels)
        w, h = Image.open(src).size
        fw, fh, x, y = fit(w, h)
        z = s.get("zoom", [0, 0, w, h])
        n = max(int(length * FPS), 1)
        p = f"(1-pow(1-min(on/{n},1),2))"   # eases out
        # zoompan on a 2x copy (smoother sub-pixel motion); zoom = full width over the window's
        content = (f"[1:v]scale={2 * w}:{2 * h}:flags=lanczos,"
                   f"zoompan=z='1+({w / z[2]:.5f}-1)*{p}':x='{2 * z[0]}*{p}':y='{2 * z[1]}*{p}':d=1:s={fw}x{fh}:fps={FPS},format=rgba[c]")
        cin = ["-framerate", str(FPS), "-loop", "1", "-t", f"{length + 0.5:.3f}", "-i", src]
    else:
        w, h = size(src)
        fw, fh, x, y = fit(w, h)
        vf = f"fps={FPS},scale={fw}:{fh}:flags=lanczos"
        if s.get("speed"): vf = f"setpts=PTS/{s['speed']}," + vf
        if s.get("hold_last"): vf += f",tpad=stop_mode=clone:stop_duration={length + 1:.3f}"
        content = f"[1:v]{vf},format=rgba[c]"
        cin = (["-stream_loop", "-1"] if s.get("loop") else []) + ["-ss", str(s.get("in", 0)), "-i", src]
    mk = masks(fw, fh)
    pl = os.path.join(WORK, f"plate{i:02d}.png"); plate(pl, x, y, fw, fh, s.get("tag"), s.get("placeholder", False))
    fc = (f"{content};[2:v]format=gray[m];[c][m]alphamerge[cr];"
          f"[0:v][cr]overlay={x}:{y}:shortest=0[g];[g][3:v]overlay=0:0,format=yuv420p[v]")
    run(["ffmpeg", "-v", "error", "-y"] + gin + cin + ["-loop", "1", "-i", mk, "-loop", "1", "-i", pl,
         "-filter_complex", fc, "-map", "[v]", "-an", "-t", f"{length:.3f}", "-r", str(FPS),
         "-c:v", "libx264", "-preset", "veryfast", "-crf", "17", out])
    return out


# ---- captions, from whisper's word times
def words(path):
    out = []
    for block in open(path).read().strip().split("\n\n"):
        L = block.strip().split("\n")
        if len(L) < 3: continue
        m = re.match(r"(\d+):(\d+):(\d+),(\d+) --> (\d+):(\d+):(\d+),(\d+)", L[1])
        g = [int(v) for v in m.groups()]
        t = " ".join(L[2:]).strip()
        if t: out.append((g[0] * 3600 + g[1] * 60 + g[2] + g[3] / 1000, g[4] * 3600 + g[5] * 60 + g[6] + g[7] / 1000, t))
    return out


def caption_lines(ws, maxc=44, hard=58):
    """Whisper's words into caption lines: break at sentence ends, after commas in a long line,
    and before a word that would run past maxc (a sentence's last word may run to hard)."""
    lines, cur = [], []
    text = lambda l: " ".join(x[2] for x in l)
    for w in ws:
        cand = text(cur + [w])
        if cur and len(cand) > maxc and not (w[2][-1] in ".?!" and len(cand) <= hard):
            lines.append(cur); cur = []
        cur.append(w)
        t = text(cur)
        if w[2][-1] in ".?!" or (w[2][-1] == "," and len(t) > 24):
            lines.append(cur); cur = []
    if cur: lines.append(cur)
    return [(l[0][0], l[-1][1], text(l)) for l in lines]


def caption_png(path, text):
    im = Image.new("RGBA", (W, H), (0, 0, 0, 0))
    d = ImageDraw.Draw(im)
    if text:
        f = font("Geist-Regular.ttf", 40)
        w = d.textlength(text, font=f)
        cx = BX + BW / 2
        d.text((cx - w / 2, BY + BH + 44), text, font=f, fill=(255, 255, 255, 255), stroke_width=3, stroke_fill=(0, 0, 0, 255))
    im.save(path)


def side_png(path, kind, text):
    """Under the talk card: the episode title, or the links at the end."""
    im = Image.new("RGBA", (W, H), (0, 0, 0, 0))
    d = ImageDraw.Draw(im)
    x, y = PX, PY + PH + 28
    if kind == "title":
        d.text((x, y), "EPISODE 294", font=font("GeistMono-Medium.ttf", 24), fill=ORANGE + (255,))
        d.text((x, y + 36), text, font=font("Geist-Regular.ttf", 44), fill=(240, 242, 245, 255))
    if kind in ("links", "end"):
        for k, t in enumerate(text.split("|")):
            d.text((x, y + k * 40), t, font=font("GeistMono-Medium.ttf", 23), fill=(240, 242, 245, 255) if k else ORANGE + (255,))
    if kind == "end":   # and a large link across the footage window's foot
        text = text.split("|")[1]
        f = font("GeistMono-Medium.ttf", 44)
        w = d.textlength(text, font=f)
        cx = BX + BW / 2
        d.rounded_rectangle([cx - w / 2 - 30, BY + BH - 150, cx + w / 2 + 30, BY + BH - 70], 16, fill=(8, 10, 14, 215))
        d.text((cx - w / 2, BY + BH - 138), text, font=f, fill=ORANGE + (255,))
    im.save(path)


def timed_track(name, items, total):
    lst = os.path.join(WORK, name + ".txt")
    blank = os.path.join(WORK, name + "-blank.png")
    caption_png(blank, "")
    t, rows = 0.0, []
    for a, b, png in sorted(items):
        if a > t: rows.append((blank, a - t))
        rows.append((png, b - max(a, t)))
        t = max(t, b)
    if t < total: rows.append((blank, total - t))
    with open(lst, "w") as f:
        for p, d in rows:
            f.write(f"file '{p}'\nduration {max(d, 0.034):.3f}\n")
        f.write(f"file '{rows[-1][0]}'\n")
    out = os.path.join(WORK, name + ".mov")
    run(["ffmpeg", "-v", "error", "-y", "-f", "concat", "-safe", "0", "-i", lst, "-vf", f"fps={FPS},format=argb",
         "-c:v", "qtrle", "-t", f"{total:.3f}", out])
    return out


def main(cut_path, out_path):
    cut = json.load(open(cut_path))
    talk = os.path.expanduser(cut["talk"])
    total = dur(talk)
    grid = grid_loop()
    only = os.environ.get("ONLY")   # ONLY=3,4 re-renders just those segments
    segs = []
    for i, s in enumerate(cut["segments"]):
        p = os.path.join(WORK, f"seg{i:02d}.mp4")
        if only is None or str(i) in only.split(",") or not os.path.exists(p):
            p = segment(i, s, total, grid)
        segs.append(p)
    lst = os.path.join(WORK, "segs.txt")
    open(lst, "w").write("".join(f"file '{p}'\n" for p in segs))
    bg = os.path.join(WORK, "background.mp4")
    run(["ffmpeg", "-v", "error", "-y", "-f", "concat", "-safe", "0", "-i", lst, "-c", "copy", bg])
    # captions
    fixes = cut.get("fixes", {})
    caps = []
    for k, (a, b, t) in enumerate(caption_lines(words(os.path.join(HERE, cut["words"])))):
        for x, y in fixes.items(): t = t.replace(x, y)
        nxt = None
        p = os.path.join(WORK, f"cap{k:03d}.png"); caption_png(p, t); caps.append((a, b, p))
    # hold each caption until the next starts (no flicker in short gaps)
    caps = [(a, (caps[k + 1][0] if k + 1 < len(caps) and caps[k + 1][0] - b < 0.8 else b + 0.3), p) for k, (a, b, p) in enumerate(caps)]
    capmov = timed_track("captions", caps, total)
    sides = []
    for k, o in enumerate(cut.get("side", [])):
        p = os.path.join(WORK, f"side{k:02d}.png"); side_png(p, o["kind"], o["text"]); sides.append((o["at"], o.get("to") or total, p))
    sidemov = timed_track("side", sides, total)
    # the talk card: rounded, with a lit edge
    frame = os.path.join(WORK, "pip-frame.png"); inner = os.path.join(WORK, "pip-mask.png")
    im = Image.new("RGBA", (PW + 2 * EDGE, PH + 2 * EDGE), (0, 0, 0, 0))
    ImageDraw.Draw(im).rounded_rectangle([0, 0, PW + 2 * EDGE - 1, PH + 2 * EDGE - 1], R + EDGE, fill=EDGE_RGB + (255,)); im.save(frame)
    mk = Image.new("L", (PW, PH), 0); ImageDraw.Draw(mk).rounded_rectangle([0, 0, PW - 1, PH - 1], R, fill=255); mk.save(inner)
    fc = (f"[1:v]crop=iw:ih*0.8:0:ih*0.1,scale={PW}:{PH}:force_original_aspect_ratio=increase:flags=lanczos,crop={PW}:{PH},format=rgba[tk];"
          f"[4:v]format=gray[mk];[tk][mk]alphamerge[tkr];"
          f"[5:v][tkr]overlay={EDGE}:{EDGE}:format=auto[pip];"
          f"[0:v][pip]overlay={PX - EDGE}:{PY - EDGE}[v1];[v1][2:v]overlay=0:0[v2];[v2][3:v]overlay=0:0,format=yuv420p[v];"
          f"[1:a]aformat=channel_layouts=stereo,alimiter=limit=0.95[a]")
    run(["ffmpeg", "-v", "error", "-y", "-i", bg, "-i", talk, "-i", sidemov, "-i", capmov,
         "-loop", "1", "-i", inner, "-loop", "1", "-i", frame,
         "-filter_complex", fc, "-map", "[v]", "-map", "[a]", "-t", f"{total:.3f}", "-r", str(FPS),
         "-c:v", "libx264", "-preset", "medium", "-crf", "19", "-c:a", "aac", "-b:a", "192k",
         "-color_primaries", "bt709", "-color_trc", "bt709", "-colorspace", "bt709",
         "-movflags", "+faststart", os.path.expanduser(out_path)])
    print("wrote", out_path, f"{total:.2f} s")


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
