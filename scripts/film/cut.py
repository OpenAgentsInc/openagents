#!/usr/bin/env python3
"""The launch film's edit, built from a cut list (#11432).

  scripts/film/cut.py inspect CUT.json [--allow-missing]
  scripts/film/cut.py render  CUT.json [--preview] [--master] [--allow-missing] [--force]

`inspect` writes contact sheets sized for a model to read, then checks the cut:
  - sources exist and are long enough
  - slots land on the music's beat grid
  - title text sits inside the title-safe area, is big enough to read, and
    lines don't overlap
  - no shot goes black or freezes
It exits 1 when a check fails. `render` runs `inspect` first and refuses to run
on a failing cut unless --force is given. It then cuts and crossfades the
shots, mixes the music (and the voice, if the cut has one, with the music
ducked under it), and writes H.264 (plus a ProRes master with --master).

In the cut list, paths starting with ~ expand. Each shot is one of:
  {"name", "src", "in", "dur", "xfade"?}  a clip, trimmed to [in, in+dur)
  {"name", "title": {...}, "dur", "xfade"?}  a title card drawn here
`xfade` is the length of the crossfade into this shot, in seconds. Slots keep
their place on the timeline: the outgoing shot runs past its slot to cover the
crossfade.
"""

import argparse
import io
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw, ImageFont

ROOT = Path(__file__).resolve().parents[2]
FONTS = ROOT / "crates/paper-mono/fonts"


def expand(p):
    return Path(os.path.expanduser(p))


def run(cmd, text=True):
    r = subprocess.run(cmd, capture_output=True, text=text)
    if r.returncode:
        err = r.stderr if text else r.stderr.decode(errors="replace")
        raise SystemExit(f"{cmd[0]} failed ({r.returncode}):\n{err[-2000:]}")
    return r


def probe(path):
    out = run(["ffprobe", "-v", "error", "-select_streams", "v:0", "-show_entries",
               "stream=width,height,r_frame_rate:format=duration", "-of", "json", str(path)]).stdout
    j = json.loads(out)
    s = j["streams"][0]
    num, den = s["r_frame_rate"].split("/")
    return {"w": s["width"], "h": s["height"], "fps": float(num) / float(den), "dur": float(j["format"]["duration"])}


def load(path):
    cut = json.loads(Path(path).read_text())
    t = 0.0
    for i, shot in enumerate(cut["shots"]):
        shot["start"] = t
        nxt = cut["shots"][i + 1] if i + 1 < len(cut["shots"]) else None
        shot["tail"] = nxt.get("xfade", 0.0) if nxt else 0.0
        t += shot["dur"]
    cut["total"] = t
    return cut


# ---- titles -------------------------------------------------------------------

def font(weight, size):
    name = {"regular": "PaperMono-Regular.ttf", "medium": "PaperMono-Medium.ttf",
            "semibold": "PaperMono-SemiBold.ttf", "bold": "PaperMono-Bold.ttf"}[weight]
    return ImageFont.truetype(str(FONTS / name), size)


def ease_out(x):
    x = min(max(x, 0.0), 1.0)
    return 1 - (1 - x) ** 3


def backdrop(w, h, seed):
    """A near-black, neutral backdrop with a soft pool of light."""
    y, x = np.mgrid[0:h, 0:w].astype(np.float32)
    cx, cy = w * 0.5, h * 0.46
    r = np.sqrt(((x - cx) / w) ** 2 + ((y - cy) / h * 0.8) ** 2)
    glow = np.exp(-(r / 0.42) ** 2)
    v = 6 + 14 * glow
    return np.stack([v, v, v], -1)


def title_layout(spec, w, h):
    """Each line's font, size and box, centered as a block. Sizes are fractions of frame height."""
    lines = []
    for ln in spec["lines"]:
        size = max(8, int(ln["size"] * h))
        f = font(ln.get("weight", "regular"), size)
        tracking = ln.get("tracking", 0.0) * size
        width = sum(f.getlength(c) for c in ln["text"]) + tracking * max(0, len(ln["text"]) - 1)
        asc, desc = f.getmetrics()
        lines.append({**ln, "font": f, "px": size, "tracking": tracking, "width": width, "height": asc + desc})
    gap = spec.get("gap", 0.025) * h
    block = sum(l["height"] for l in lines) + gap * (len(lines) - 1)
    y = h * spec.get("center", 0.5) - block / 2
    for l in lines:
        l["box"] = ((w - l["width"]) / 2, y, (w + l["width"]) / 2, y + l["height"])
        y += l["height"] + gap
    return lines


def draw_title(spec, w, h, t, dur, grain_rng, base):
    img = base.copy()
    if spec.get("grain", True):
        img = img + grain_rng.normal(0, 1.6, img.shape[:2])[..., None]
    frame = Image.fromarray(np.clip(img, 0, 255).astype(np.uint8))
    layer = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    d = ImageDraw.Draw(layer)
    for i, l in enumerate(title_layout(spec, w, h)):
        delay = l.get("delay", 0.25 + 0.35 * i)
        p = ease_out((t - delay) / l.get("rise", 1.1))
        out = spec.get("out", 0.0)
        if out:
            p *= 1 - ease_out((t - (dur - out)) / out)
        if p <= 0:
            continue
        # Tracking settles from wide to its set value; the line rises a little as it fades in.
        track = l["tracking"] + (1 - p) * 0.12 * l["px"]
        width = sum(l["font"].getlength(c) for c in l["text"]) + track * max(0, len(l["text"]) - 1)
        x = (w - width) / 2
        y = l["box"][1] + (1 - p) * 0.012 * h
        color = tuple(int(c) for c in l.get("color", [236, 238, 241])) + (int(255 * p),)
        for c in l["text"]:
            d.text((x, y), c, font=l["font"], fill=color)
            x += l["font"].getlength(c) + track
    frame.paste(layer, (0, 0), layer)
    return frame


def render_title(shot, w, h, fps, out):
    """Draws a title card. With `"under": {"src", "in", "dim"}` the text sits
    over that clip (dimmed by `dim`, 0-1) instead of the plain backdrop."""
    n = int(round((shot["dur"] + shot["tail"]) * fps))
    rng = np.random.default_rng(len(shot["name"]))
    under = shot["title"].get("under")
    dec = None
    if under:
        dec = subprocess.Popen(["ffmpeg", "-v", "error", "-ss", str(under.get("in", 0.0)), "-i", str(expand(under["src"])),
                                "-vf", f"scale={w}:{h}:flags=lanczos,fps={fps}", "-f", "rawvideo", "-pix_fmt", "rgb24", "-"],
                               stdout=subprocess.PIPE)
    base = backdrop(w, h, 0)
    spec = {**shot["title"], "grain": shot["title"].get("grain", not under)}
    enc = subprocess.Popen(["ffmpeg", "-v", "error", "-y", "-f", "rawvideo", "-pix_fmt", "rgb24", "-s", f"{w}x{h}",
                            "-r", str(fps), "-i", "-", "-c:v", "libx264", "-crf", "8", "-preset", "fast",
                            "-pix_fmt", "yuv420p", str(out)], stdin=subprocess.PIPE)
    for k in range(n):
        if dec:
            raw = dec.stdout.read(w * h * 3)
            if len(raw) == w * h * 3:
                base = np.frombuffer(raw, np.uint8).reshape(h, w, 3).astype(np.float32) * (1 - under.get("dim", 0.0))
        enc.stdin.write(draw_title(spec, w, h, k / fps, shot["dur"], rng, base).tobytes())
    enc.stdin.close()
    enc.wait()
    if dec:
        dec.kill()


def placeholder(shot, w, h, fps, out):
    spec = {"lines": [{"text": f"MISSING  {shot['name']}", "size": 0.04}], "grain": False}
    render_title({**shot, "title": spec}, w, h, fps, out)


# ---- segments -------------------------------------------------------------------

def render_segment(shot, cut, w, h, out, allow_missing):
    fps = cut["fps"]
    if "title" in shot:
        return render_title(shot, w, h, fps, out)
    src = expand(shot["src"])
    if not src.exists():
        if allow_missing:
            return placeholder(shot, w, h, fps, out)
        raise SystemExit(f"missing source {src}")
    length = shot["dur"] + shot["tail"]
    vf = f"scale={w}:{h}:force_original_aspect_ratio=decrease:flags=lanczos,pad={w}:{h}:-1:-1:black,fps={fps},setsar=1"
    if shot.get("speed"):
        vf = f"setpts=PTS/{shot['speed']}," + vf
    run(["ffmpeg", "-v", "error", "-y", "-ss", str(shot.get("in", 0.0)), "-i", str(src), "-t", str(length),
         "-vf", vf, "-an", "-c:v", "libx264", "-crf", "8", "-preset", "fast", "-pix_fmt", "yuv420p", str(out)])


# ---- inspect ------------------------------------------------------------------

def inspect(cut, cut_path, work, w, h, allow_missing):
    problems, notes = [], []
    beat = 60.0 / cut.get("bpm", 120)
    for shot in cut["shots"]:
        name = shot["name"]
        if abs(shot["start"] / beat - round(shot["start"] / beat)) > 1e-6:
            problems.append(f"{name}: starts at {shot['start']:.2f}s, off the {cut.get('bpm', 120)} BPM beat grid")
        if "title" in shot:
            under = shot["title"].get("under")
            if under and not expand(under["src"]).exists():
                (notes if allow_missing else problems).append(f"{name}: missing {under['src']}")
                shot["title"].pop("under")
            safe = (w * 0.1, h * 0.1, w * 0.9, h * 0.9)
            lines = title_layout(shot["title"], w, h)
            for l in lines:
                x0, y0, x1, y1 = l["box"]
                if x0 < safe[0] or x1 > safe[2] or y0 < safe[1] or y1 > safe[3]:
                    problems.append(f"{name}: '{l['text']}' leaves the title-safe area")
                if l["px"] < h * 0.025:
                    problems.append(f"{name}: '{l['text']}' is {l['px']}px, too small to read at {h}p")
            for a, b in zip(lines, lines[1:]):
                if a["box"][3] > b["box"][1]:
                    problems.append(f"{name}: '{a['text']}' overlaps '{b['text']}'")
            continue
        src = expand(shot["src"])
        if not src.exists():
            (notes if allow_missing else problems).append(f"{name}: missing {src}")
            continue
        p = probe(src)
        need = shot.get("in", 0.0) + (shot["dur"] + shot["tail"]) * shot.get("speed", 1.0)
        if p["dur"] + 1e-3 < need:
            problems.append(f"{name}: {src.name} is {p['dur']:.2f}s, the cut needs {need:.2f}s")
        if abs(p["w"] / p["h"] - w / h) > 0.01:
            problems.append(f"{name}: {src.name} is {p['w']}x{p['h']}, not {w}:{h}; it would be letterboxed")
        if p["h"] < h:
            notes.append(f"{name}: {src.name} is {p['h']}p, upscaled to {h}p")

    music = cut.get("music")
    if music and expand(music).exists():
        mdur = float(run(["ffprobe", "-v", "error", "-show_entries", "format=duration", "-of", "csv=p=0",
                          str(expand(music))]).stdout)
        if mdur + 1e-3 < cut["total"]:
            problems.append(f"music is {mdur:.2f}s, the cut is {cut['total']:.2f}s")
    elif music:
        problems.append(f"music missing: {music}")

    # Render the segments, then check each for black or frozen stretches.
    segs = []
    for i, shot in enumerate(cut["shots"]):
        seg = work / f"{i:02d}-{shot['name']}.mp4"
        render_segment(shot, cut, w, h, seg, allow_missing)
        segs.append(seg)
        if "title" in shot or not expand(shot["src"]).exists():
            continue
        log = subprocess.run(["ffmpeg", "-v", "info", "-i", str(seg), "-vf",
                              "blackdetect=d=0.4:pix_th=0.06,freezedetect=n=0.0008:d=1.5", "-f", "null", "-"],
                             capture_output=True, text=True).stderr
        for m in re.finditer(r"black_start:([\d.]+) black_end:([\d.]+)", log):
            problems.append(f"{shot['name']}: black from {float(m[1]):.1f}s to {float(m[2]):.1f}s of its slot")
        for m in re.finditer(r"freeze_duration: ([\d.]+)", log):
            if float(m[1]) >= shot.get("hold_ok", 1.5):
                problems.append(f"{shot['name']}: frame frozen for {float(m[1]):.1f}s")

    sheets = contact_sheets(cut, segs, cut_path)
    return problems, notes, segs, sheets


def contact_sheets(cut, segs, cut_path):
    """Three frames per shot (in, middle, out) on 1440 px sheets of four shots each."""
    tw, th = 480, 270
    out_dir = expand(cut.get("sheets", "~/work/film-out/cut"))
    out_dir.mkdir(parents=True, exist_ok=True)
    label = font("medium", 18)
    sheets = []
    for s0 in range(0, len(segs), 4):
        sheet = Image.new("RGB", (tw * 3, (th + 30) * min(4, len(segs) - s0)), (18, 18, 20))
        d = ImageDraw.Draw(sheet)
        for row, i in enumerate(range(s0, min(s0 + 4, len(segs)))):
            shot = cut["shots"][i]
            for col, frac in enumerate((0.08, 0.5, 0.92)):
                t = shot["dur"] * frac
                png = run(["ffmpeg", "-v", "error", "-ss", f"{t:.3f}", "-i", str(segs[i]), "-frames:v", "1",
                           "-vf", f"scale={tw}:{th}", "-f", "image2pipe", "-vcodec", "png", "-"], text=False).stdout
                tile = Image.open(io.BytesIO(png))
                sheet.paste(tile, (col * tw, row * (th + 30) + 30))
            d.text((8, row * (th + 30) + 6), f"{i + 1}. {shot['name']}  {shot['start']:.1f}-{shot['start'] + shot['dur']:.1f}s",
                   font=label, fill=(220, 220, 225))
        path = out_dir / f"contact-{s0 // 4 + 1}.png"
        sheet.save(path)
        sheets.append(path)
    return sheets


# ---- render -------------------------------------------------------------------

def assemble(cut, segs, w, h, out, master):
    fps = cut["fps"]
    inputs, chain = [], []
    for s in segs:
        inputs += ["-i", str(s)]
    for i in range(len(segs)):
        chain.append(f"[{i}:v]settb=1/{fps},setpts=N/({fps}*TB),format=yuv420p[s{i}]")
    label = "[s0]"
    for i in range(1, len(segs)):
        shot = cut["shots"][i]
        x = shot.get("xfade", 0.0)
        nxt = f"[v{i}]"
        if x > 0:
            chain.append(f"{label}[s{i}]xfade=transition={shot.get('xfade_kind', 'fade')}:duration={x}:offset={shot['start']}{nxt}")
        else:
            # A hard cut: the outgoing chain already ends at this slot's start.
            chain.append(f"{label}[s{i}]concat=n=2:v=1:a=0,settb=1/{fps},setpts=N/({fps}*TB){nxt}")
        label = nxt
    fin = cut.get("fade_in", 0.0)
    fout = cut.get("fade_out", 0.0)
    total = cut["total"]
    post = f"{label}trim=end={total},setpts=PTS-STARTPTS"
    if fin:
        post += f",fade=t=in:st=0:d={fin}"
    if fout:
        post += f",fade=t=out:st={total - fout}:d={fout}"
    chain.append(post + "[v]")

    n = len(segs)
    audio = []
    music = expand(cut["music"])
    inputs += ["-i", str(music)]
    a = f"[{n}:a]atrim=end={total},asetpts=PTS-STARTPTS,volume={cut.get('music_gain', 1.0)}"
    if fout:
        a += f",afade=t=out:st={total - fout}:d={fout}"
    voice = cut.get("voice")
    if voice:
        inputs += ["-itsoffset", str(voice.get("at", 0.0)), "-i", str(expand(voice["src"]))]
        audio.append(a + "[m]")
        audio.append(f"[{n + 1}:a]volume={voice.get('gain', 1.0)},asplit=2[vo][key]")
        audio.append("[m][key]sidechaincompress=threshold=0.04:ratio=6:attack=20:release=400[duck]")
        audio.append("[duck][vo]amix=inputs=2:normalize=0,loudnorm=I=-14:TP=-1.0:LRA=11[a]")
    else:
        audio.append(a + ",loudnorm=I=-14:TP=-1.0:LRA=11[a]")
    graph = ";".join(chain + audio)
    out.parent.mkdir(parents=True, exist_ok=True)
    run(["ffmpeg", "-v", "error", "-y", *inputs, "-filter_complex", graph, "-map", "[v]", "-map", "[a]",
         "-r", str(fps), "-c:v", "libx264", "-crf", "14", "-preset", "slow", "-pix_fmt", "yuv420p",
         "-c:a", "aac", "-b:a", "320k", "-ar", "48000", "-movflags", "+faststart", str(out)])
    if master:
        run(["ffmpeg", "-v", "error", "-y", *inputs, "-filter_complex", graph, "-map", "[v]", "-map", "[a]",
             "-r", str(fps), "-c:v", "prores_ks", "-profile:v", "3", "-c:a", "pcm_s24le",
             str(out.with_suffix(".mov"))])


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("cmd", choices=["inspect", "render"])
    ap.add_argument("cut")
    ap.add_argument("--preview", action="store_true", help="960x540, for quick looks")
    ap.add_argument("--master", action="store_true", help="also write a ProRes 422 HQ .mov")
    ap.add_argument("--allow-missing", action="store_true", help="draw a card for sources not rendered yet")
    ap.add_argument("--force", action="store_true", help="render even when inspect fails")
    args = ap.parse_args()

    cut = load(args.cut)
    w, h = cut["size"]
    if args.preview:
        w, h = w // 2, h // 2
    work = Path(tempfile.mkdtemp(prefix="film-cut-"))
    try:
        problems, notes, segs, sheets = inspect(cut, args.cut, work, w, h, args.allow_missing)
        for s in sheets:
            print(f"sheet  {s}")
        for n in notes:
            print(f"note   {n}")
        for p in problems:
            print(f"FAIL   {p}")
        print(f"{len(cut['shots'])} shots, {cut['total']:.1f}s, {len(problems)} problem(s)")
        if args.cmd == "inspect":
            sys.exit(1 if problems else 0)
        if problems and not args.force:
            sys.exit("inspect failed; fix the cut or pass --force")
        out = expand(cut["out"])
        if args.preview:
            out = out.with_name(out.stem + "-preview.mp4")
        assemble(cut, segs, w, h, out, args.master)
        print(out)
    finally:
        shutil.rmtree(work, ignore_errors=True)


if __name__ == "__main__":
    main()
