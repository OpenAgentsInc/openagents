#!/usr/bin/env python3
"""The launch film's edit, built from a cut list (#11432).

  scripts/film/cut.py check   CUT.json [--json] [--allow-missing]
  scripts/film/cut.py inspect CUT.json [--allow-missing]
  scripts/film/cut.py render  CUT.json [--preview | --size WxH] [--out PATH] [--master] [--allow-missing] [--force]

`check` runs the checks that need no rendering (everything below but black
and freeze detection and the contact sheets), in about a second. It also reads
`<clip>.beats.json` beside a source, when its renderer wrote one: the clip's
animation beats (`[{"from", "to", "what"}]`, clip seconds), and fails a shot
that ends inside one, so a cut never chops an animation off mid-move. And it
fails a shot that pauses: one that holds a still frame (`play`) longer than
STILL_OK (0.4 s; a shot's `"still_ok"` allows more), or whose part of its clip
has a still stretch that long (no 64x36 cell changing by more than 2 levels,
so grain doesn't count as motion; measured once per clip and cached under the
Lab's root, `motion/`). Video Lab (scripts/film/lab.py) runs
it on every edit.

`inspect` writes contact sheets sized for a model to read, then checks the cut:
  - sources exist and are long enough
  - slots land on the music's beat grid
  - title text sits inside the title-safe area, is big enough to read, and
    lines don't overlap
  - no shot goes black or freezes
It exits 1 when a check fails. `render` runs `inspect` first and refuses to run
on a failing cut unless --force is given. It then cuts and crossfades the
shots, mixes the music (and the voice, if the cut has one, or its `voices`, with the music
ducked under it), and writes H.264 (plus a ProRes master with --master).

In the cut list, paths starting with ~ expand. Each shot is one of:
  {"name", "src", "in", "dur", "xfade"?}  a clip, trimmed to [in, in+dur)
  {"name", "title": {...}, "dur", "xfade"?}  a title card drawn here
A clip may carry `"over": {"src", "at"?}`: a clip with alpha (such as
title-overlay.py's) composited on top of it from `at` seconds into the slot.
`xfade` is the length of the crossfade into this shot, in seconds. Slots keep
their place on the timeline: the outgoing shot runs past its slot to cover the
crossfade.

A clip may also carry `"play": S`: only S seconds of the source play, and the
rest of the slot holds that last frame (never sped up). `"push": 0.04` eases a
slow push-in over the hold, reaching 4% at the end of the slot, so a long hold
doesn't read as a stalled picture. A missing clip with `"label"` draws that
text on its placeholder card (a labelled slot for a shot still being made).

A clip whose shape isn't the film's takes `"fit"`: "pad" (the default: whole,
on black), "blur" (whole, over a blurred, dimmed copy of itself filling the
frame) or "crop" (fills the frame, cropped; `"focus": [x, y]`, 0-1, picks the
part kept, default the middle). Sizes are read with the camera's rotation flag
applied, so a phone clip shot sideways is the landscape frame it plays as.

A clip with `"sound": {"gain"?}` keeps its own sound (a person speaking): it
plays under its slot, and the music is ducked under it (`"music_duck"`, the
music's level there, default 0.2). Such a shot must start and end in a pause
in its speech: `check` reads the clip's transcript (`<clip>.speech.json`
beside it, or the one Video Lab made on import, see SOURCES) and fails a cut
in the middle of a word, and a caption over it that doesn't match what's said.

The cut may carry `"captions":[{"text", "at", "dur"}, ...]`: lines of the
script drawn small in the bottom third (Geist Regular, from FILM_FONTS, default
~/work/film-out/fonts) on a soft dark band, at timeline seconds `at` for `dur`
seconds, wrapped to at most two lines. `"caption_style"` can set `size` (a
fraction of frame height), `y` (the band's center, a fraction of height) and
`width` (the widest line, a fraction of width). They are for timing a voice
over before it's recorded; inspect checks they fit, stay on the timeline and
don't overlap.
"""

import argparse
import hashlib
import io
import json
import math
import os
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw, ImageFilter, ImageFont

ROOT = Path(__file__).resolve().parents[2]
FONTS = ROOT / "crates/paper-mono/fonts"
CACHE_VERSION = 1  # bump when segment rendering changes
FILM_FONTS = Path(os.path.expanduser(os.environ.get("FILM_FONTS", "~/work/film-out/fonts")))
# Clips Video Lab imported (scripts/film/lab.py lab_import): their transcript with
# word times and their pauses, in <id>/speech.json, found by path in index.json.
SOURCES = Path(os.path.expanduser(os.environ.get("VIDEO_LAB_ROOT", "~/work/film-out/lab"))) / "sources"
FITS = ("pad", "blur", "crop")


def expand(p):
    return Path(os.path.expanduser(p))


def run(cmd, text=True):
    r = subprocess.run(cmd, capture_output=True, text=text)
    if r.returncode:
        err = r.stderr if text else r.stderr.decode(errors="replace")
        raise SystemExit(f"{cmd[0]} failed ({r.returncode}):\n{err[-2000:]}")
    return r


def probe(path):
    """Size as shown (a phone's rotation flag applied, so a clip shot sideways
    reads as the frame it plays as), fps, length, and whether it has sound."""
    out = run(["ffprobe", "-v", "error", "-show_entries",
               "stream=codec_type,width,height,r_frame_rate:stream_side_data=rotation:format=duration",
               "-of", "json", str(path)]).stdout
    j = json.loads(out)
    streams = j.get("streams", [])
    s = next(x for x in streams if x.get("codec_type") == "video")
    num, den = s["r_frame_rate"].split("/")
    w, h, rot = s["width"], s["height"], 0
    for side in s.get("side_data_list", []) or []:
        if "rotation" in side:
            rot = int(side["rotation"])
    if abs(rot) % 180 == 90:
        w, h = h, w
    return {"w": w, "h": h, "fps": float(num) / float(den), "dur": float(j["format"]["duration"]),
            "rotation": rot, "audio": any(x.get("codec_type") == "audio" for x in streams)}


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
    lines = [{"text": f"MISSING  {shot['name']}", "size": 0.04}]
    if shot.get("label"):
        lines.append({"text": shot["label"], "size": 0.026, "color": [150, 150, 155]})
    spec = {"lines": lines, "grain": False}
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
    play = shot.get("play")
    if play is not None and play < length - 1e-6:
        return render_held(shot, cut, w, h, out, src, play, length)
    vf = fit_filter(shot, w, h) + f",fps={fps},setsar=1"
    if shot.get("speed"):
        vf = f"setpts=PTS/{shot['speed']}," + vf
    over = shot.get("over")
    if over:
        # The overlay keeps its alpha; it starts `at` seconds into the slot.
        at = over.get("at", 0.0)
        graph = (f"[0:v]{vf}[base];[1:v]scale={w}:{h}:flags=lanczos,fps={fps},format=yuva444p,"
                 f"setpts=PTS-STARTPTS+{at}/TB[top];[base][top]overlay=eof_action=pass:format=auto,format=yuv420p")
        run(["ffmpeg", "-v", "error", "-y", "-ss", str(shot.get("in", 0.0)), "-i", str(src),
             "-i", str(expand(over["src"])), "-t", str(length), "-filter_complex", graph, "-an",
             "-c:v", "libx264", "-crf", "8", "-preset", "fast", "-pix_fmt", "yuv420p", str(out)])
        return
    run(["ffmpeg", "-v", "error", "-y", "-ss", str(shot.get("in", 0.0)), "-i", str(src), "-t", str(length),
         "-vf", vf, "-an", "-c:v", "libx264", "-crf", "8", "-preset", "fast", "-pix_fmt", "yuv420p", str(out)])


def fit_filter(shot, w, h):
    """The filter that puts a clip into a w x h frame, by the shot's `fit`."""
    fit = shot.get("fit", "pad")
    if fit == "crop":
        fx, fy = shot.get("focus", [0.5, 0.5])
        return (f"scale={w}:{h}:force_original_aspect_ratio=increase:flags=lanczos,"
                f"crop={w}:{h}:(iw-{w})*{fx}:(ih-{h})*{fy}")
    if fit == "blur":
        # The clip whole, over itself scaled to fill, blurred and dimmed.
        return (f"split[fitbg][fitfg];[fitbg]scale={w}:{h}:force_original_aspect_ratio=increase,crop={w}:{h},"
                f"gblur=sigma={max(8, h // 40)},eq=brightness=-0.12:saturation=0.75[fitb];"
                f"[fitfg]scale={w}:{h}:force_original_aspect_ratio=decrease:flags=lanczos[fitf];"
                f"[fitb][fitf]overlay=(W-w)/2:(H-h)/2")
    return f"scale={w}:{h}:force_original_aspect_ratio=decrease:flags=lanczos,pad={w}:{h}:-1:-1:black"


def render_held(shot, cut, w, h, out, src, play, length):
    """Plays `play` seconds of the clip, then holds its last frame for the rest
    of the slot, with an eased push-in of `push` (a fraction of the frame)."""
    fps = cut["fps"]
    tmp = out.with_suffix("")
    played = Path(f"{tmp}-play.mp4")
    if play > 0:
        render_segment({**shot, "play": None, "dur": play, "tail": 0.0}, cut, w, h, played, False)
        run(["ffmpeg", "-v", "error", "-y", "-sseof", "-0.25", "-i", str(played), "-update", "1",
             "-frames:v", "100", f"{tmp}-last.png"])
    else:
        vf = fit_filter(shot, w, h)
        run(["ffmpeg", "-v", "error", "-y", "-ss", str(shot.get("in", 0.0)), "-i", str(src), "-frames:v", "1",
             "-vf", vf, f"{tmp}-last.png"])
    still = Image.open(f"{tmp}-last.png").convert("RGB")
    n = int(round((length - play) * fps))
    push = shot.get("push", 0.0)
    held = Path(f"{tmp}-hold.mp4")
    enc = subprocess.Popen(["ffmpeg", "-v", "error", "-y", "-f", "rawvideo", "-pix_fmt", "rgb24", "-s", f"{w}x{h}",
                            "-r", str(fps), "-i", "-", "-c:v", "libx264", "-crf", "8", "-preset", "fast",
                            "-pix_fmt", "yuv420p", str(held)], stdin=subprocess.PIPE)
    cx, cy = w / 2, h / 2
    for k in range(n):
        u = (k + 1) / max(n, 1)
        z = 1 + push * u * u * (3 - 2 * u)
        frame = still if push <= 0 else still.transform(
            (w, h), Image.AFFINE, (1 / z, 0, cx - cx / z, 0, 1 / z, cy - cy / z), resample=Image.BICUBIC)
        enc.stdin.write(frame.tobytes())
    enc.stdin.close()
    enc.wait()
    parts = ([played] if play > 0 else []) + [held]
    listing = Path(f"{tmp}-parts.txt")
    listing.write_text("".join(f"file '{p}'\n" for p in parts))
    run(["ffmpeg", "-v", "error", "-y", "-f", "concat", "-safe", "0", "-i", str(listing), "-c", "copy", str(out)])
    for p in (played, held, listing, Path(f"{tmp}-last.png")):
        p.unlink(missing_ok=True)


def segment_cached(shot, cut, w, h, seg, cache, allow_missing):
    """Renders a segment, reusing an earlier render of the same shot (same
    spec, size, tail and source files) so re-cutting doesn't redo long holds."""
    srcs = [shot.get("src"), (shot.get("over") or {}).get("src"), ((shot.get("title") or {}).get("under") or {}).get("src")]
    stamp = []
    for p in srcs:
        if p and expand(p).exists():
            st = expand(p).stat()
            stamp.append([str(expand(p)), st.st_size, st.st_mtime])
    spec = {k: v for k, v in shot.items() if k not in ("start", "name", "xfade", "xfade_kind", "sound")}
    key = hashlib.sha1(json.dumps([spec, w, h, cut["fps"], stamp, CACHE_VERSION], sort_keys=True,
                                  default=str).encode()).hexdigest()[:16]
    hit = cache / f"{key}.mp4"
    missing = shot.get("src") and not expand(shot["src"]).exists()
    if hit.exists() and not missing:
        return hit
    render_segment(shot, cut, w, h, seg, allow_missing)
    if missing:
        return seg
    shutil.move(seg, hit)
    return hit


# ---- captions -----------------------------------------------------------------

def caption_style(cut, h):
    st = {"size": 0.03, "y": 0.855, "width": 0.66, **cut.get("caption_style", {})}
    st["px"] = max(8, int(st["size"] * h))
    st["font"] = ImageFont.truetype(str(FILM_FONTS / "Geist-Regular.ttf"), st["px"])
    return st


def wrap(text, f, maxw):
    lines, cur = [], ""
    for word in text.split():
        trial = f"{cur} {word}".strip()
        if cur and f.getlength(trial) > maxw:
            lines.append(cur)
            cur = word
        else:
            cur = trial
    lines += [cur] if cur else []
    if len(lines) == 2:
        # Balance two lines so the second isn't a stray word or two.
        words = text.split()
        best = min(range(1, len(words)), key=lambda k: max(f.getlength(" ".join(words[:k])),
                                                           f.getlength(" ".join(words[k:]))))
        lines = [" ".join(words[:best]), " ".join(words[best:])]
    return lines


def caption_layout(cap, st, w, h):
    lines = wrap(cap["text"], st["font"], st["width"] * w)
    asc, desc = st["font"].getmetrics()
    lh = int((asc + desc) * 1.18)
    padx, pady = int(st["px"] * 0.8), int(st["px"] * 0.42)
    bw = int(max(st["font"].getlength(l) for l in lines)) + 2 * padx
    bh = lh * len(lines) + 2 * pady
    x, y = (w - bw) // 2, int(st["y"] * h - bh / 2)
    return lines, (x, y, bw, bh), lh, padx, pady


def render_caption(cap, st, w, h, path):
    lines, (x, y, bw, bh), lh, padx, pady = caption_layout(cap, st, w, h)
    im = Image.new("RGBA", (bw, bh), (0, 0, 0, 0))
    band = Image.new("L", (bw, bh), 0)
    ImageDraw.Draw(band).rounded_rectangle([0, 0, bw - 1, bh - 1], radius=int(st["px"] * 0.5), fill=185)
    im.putalpha(band)
    d = ImageDraw.Draw(im)
    for i, l in enumerate(lines):
        lx = (bw - st["font"].getlength(l)) / 2
        d.text((lx, pady + i * lh), l, font=st["font"], fill=(238, 239, 242, 255))
    im.save(path)
    return x, y


# ---- inspect ------------------------------------------------------------------

def speech_for(src):
    """A clip's speech, when it has been transcribed: {"words": [{"w", "from",
    "to"}], "quiet": [{"from", "to"}], "duration"} in the clip's seconds.
    `quiet` are measured pauses (no voice), which is where a cut may go."""
    side = Path(src).with_suffix(".speech.json")
    if side.exists():
        try:
            return json.loads(side.read_text())
        except (OSError, ValueError):
            return None
    index = SOURCES / "index.json"
    if index.exists():
        try:
            sid = json.loads(index.read_text()).get(str(Path(src).resolve()))
        except (OSError, ValueError):
            sid = None
        if sid and (SOURCES / sid / "speech.json").exists():
            return json.loads((SOURCES / sid / "speech.json").read_text())
    return None


def near_pauses(t, speech, n=2):
    """The pauses nearest t, as clip seconds a cut could go at (their middles)."""
    qs = sorted(speech.get("quiet", []), key=lambda q: abs((q["from"] + q["to"]) / 2 - t))[:n]
    return sorted(round((q["from"] + q["to"]) / 2, 2) for q in qs)


def beats_for(src):
    """The animation beats a source's renderer recorded beside it, if any:
    `<clip>.beats.json` = [{"from": s, "to": s, "what": "..."}, ...] in the
    clip's own seconds. A shot that ends inside one cuts an animation off."""
    side = src.with_suffix(".beats.json")
    if not side.exists():
        return []
    try:
        return json.loads(side.read_text())
    except (OSError, ValueError):
        return []


# A shot may show a still picture this long (seconds) and no longer, unless it
# sets "still_ok": the owner's rule for Episode 290 is that an animation never
# pauses mid-film. Holds the cut asks for ("play") count, push-in or not.
STILL_OK = 0.4
# What "still" means: on a 64x36 copy (each cell the average of a block of
# pixels, so film grain cancels out), no cell changes by more than STILL_LEVEL
# across STILL_OK seconds. A light the size of a dot moving anywhere in the
# frame, or a slow camera ease, changes a cell by several levels over that
# time; grain and encoding noise stay at 1.
STILL_LEVEL = 2.0


def freezes(path, min_len=STILL_OK):
    """Stretches [{"from", "to"}] (the file's seconds) where its picture doesn't move."""
    w, h = 64, 36
    raw = subprocess.run(["ffmpeg", "-v", "error", "-i", str(path), "-an", "-vf",
                          f"scale={w}:{h}:flags=area,format=gray", "-f", "rawvideo", "-"],
                         capture_output=True).stdout
    frames = np.frombuffer(raw, np.uint8)[: len(raw) // (w * h) * (w * h)].reshape(-1, h, w).astype(np.int16)
    if len(frames) < 2:
        return []
    fps = probe(path).get("fps") or 30.0
    k = max(1, int(round(min_len * fps)))
    if len(frames) <= k:
        return []
    # Frame i starts a still stretch when nothing has changed by frame i + k: a
    # slow ease drifts by more than that over min_len, a held picture doesn't.
    flat = frames.reshape(len(frames), -1)
    calm = np.abs(flat[k:] - flat[:-k]).max(axis=1) <= STILL_LEVEL
    still = np.zeros(len(frames), bool)
    for i in np.nonzero(calm)[0]:
        still[i:i + k + 1] = True
    out, i = [], 0
    while i < len(still):
        if not still[i]:
            i += 1
            continue
        j = i
        while j < len(still) and still[j]:
            j += 1
        out.append({"from": round(i / fps, 3), "to": round(j / fps, 3)})
        i = j
    return out


def stills_for(src, cache_dir):
    """A clip's still stretches, measured once and kept in cache_dir by the
    file's path, size and time (a re-rendered clip is measured again)."""
    st = src.stat()
    key = hashlib.sha256(f"{src.resolve()}|{st.st_size}|{st.st_mtime_ns}|{STILL_LEVEL}|{STILL_OK}|window".encode()).hexdigest()
    path = cache_dir / f"{key[:24]}.json"
    if path.exists():
        try:
            return json.loads(path.read_text())
        except ValueError:
            pass
    found = freezes(src)
    cache_dir.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(found))
    return found


def measure(cut, w, h, allow_missing=False):
    """The facts about a cut's files that its checks need (crates/video-lab
    `Measured`): each clip's probe, animation beats, still stretches and
    speech; title and caption layouts in our fonts; voiceover and music
    lengths. A title whose `under` clip is missing loses it here, as before."""
    m = {"size": [w, h], "allow_missing": bool(allow_missing), "sources": {}, "titles": {}, "captions": {},
         "audio": {}, "files": {}}
    for shot in cut["shots"]:
        if "title" in shot:
            t = {}
            under = shot["title"].get("under")
            if under and not expand(under["src"]).exists():
                t["under_missing"] = under["src"]
                shot["title"].pop("under")
            t["lines"] = [{"text": l["text"], "px": l["px"], "box": [float(x) for x in l["box"]]}
                          for l in title_layout(shot["title"], w, h)]
            m["titles"][shot["name"]] = t
            continue
        if shot.get("over"):
            m["files"][shot["over"]["src"]] = expand(shot["over"]["src"]).exists()
        key = shot.get("src")
        if key is None:
            continue
        src = expand(key)
        e = m["sources"].get(key) or {"exists": src.exists(), "path": str(src), "name": src.name}
        if e["exists"] and "dur" not in e:
            p = probe(src)
            e.update(w=p["w"], h=p["h"], fps=p["fps"], dur=p["dur"], audio=p["audio"], beats=beats_for(src))
        if e["exists"] and shot.get("sound") and "speech" not in e:
            e["speech"] = speech_for(src)
        if e["exists"] and not shot.get("sound") and "stills" not in e:
            e["stills"] = stills_for(src, SOURCES.parent / "motion")
        m["sources"][key] = e
    caps = cut.get("captions", [])
    if caps:
        st = caption_style(cut, h)
        m["caption_px"] = st["px"]
        for c in caps:
            lines, (x, y, bw, bh), *_ = caption_layout(c, st, w, h)
            m["captions"][c["text"]] = {"lines": len(lines), "box": [float(x), float(y), float(bw), float(bh)]}
    for v in ([cut["voice"]] if cut.get("voice") else []) + list(cut.get("voices", [])):
        src = expand(v["src"])
        m["audio"][v["src"]] = audio_length(src) if src.exists() else None
    music = cut.get("music")
    if music:
        m["music"] = audio_length(expand(music)) if expand(music).exists() else None
    return m


def check(cut, w, h, allow_missing):
    """The checks that need no rendering, run by crates/video-lab over this
    cut and the facts `measure` finds about its files."""
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    import video_lab_bridge
    out = video_lab_bridge.call("check_cut", document=strip_layout(cut), measured=measure(cut, w, h, allow_missing))
    return out["problems"], out["notes"]


def strip_layout(cut):
    """The cut as written: without the start, tail and total that `load` adds."""
    out = {k: v for k, v in cut.items() if k != "total"}
    out["shots"] = [{k: v for k, v in s.items() if k not in ("start", "tail")} for s in cut["shots"]]
    return out


def inspect(cut, cut_path, work, w, h, allow_missing):
    problems, notes = check(cut, w, h, allow_missing)

    # Render the segments, then check each for black or frozen stretches.
    segs = []
    cache = expand(cut.get("cache", "~/work/film-out/.cache/cut-segments"))
    cache.mkdir(parents=True, exist_ok=True)
    for i, shot in enumerate(cut["shots"]):
        seg = segment_cached(shot, cut, w, h, work / f"{i:02d}-{shot['name']}.mp4", cache, allow_missing)
        segs.append(seg)
        if "title" in shot or not expand(shot["src"]).exists():
            continue
        log = subprocess.run(["ffmpeg", "-v", "info", "-i", str(seg), "-vf",
                              "blackdetect=d=0.4:pix_th=0.06,freezedetect=n=0.0008:d=1.5", "-f", "null", "-"],
                             capture_output=True, text=True).stderr
        for m in re.finditer(r"black_start:([\d.]+) black_end:([\d.]+)", log):
            if shot.get("dark_ok"):  # a dark UI on a black stage, checked by eye
                continue
            problems.append(f"{shot['name']}: black from {float(m[1]):.1f}s to {float(m[2]):.1f}s of its slot")
        held_from = shot["play"] - 0.5 if shot.get("play") is not None else None
        for m in re.finditer(r"freeze_start: ([\d.]+).*?freeze_duration: ([\d.]+)", log, re.S):
            if held_from is not None and float(m[1]) >= held_from:
                continue  # the hold the cut asked for
            if float(m[2]) >= shot.get("hold_ok", 1.5):
                problems.append(f"{shot['name']}: frame frozen for {float(m[2]):.1f}s")

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

def voice_slots(cut):
    """(voice, start, length) for each voiceover clip: the cut's `voice` and
    each of its `voices` ([{"src", "at", "gain"?}], placed by Video Lab's
    lab_voice), with the music ducked under each."""
    out = []
    for v in ([cut["voice"]] if cut.get("voice") else []) + list(cut.get("voices", [])):
        src = expand(v["src"])
        if src.exists():
            out.append((v, v.get("at", 0.0), audio_length(src)))
    return out


def audio_length(path):
    return float(run(["ffprobe", "-v", "error", "-show_entries", "format=duration", "-of", "csv=p=0",
                      str(path)]).stdout)


def sound_slots(cut):
    """(shot, start, dur) for each shot that keeps its own sound, on the timeline."""
    out = []
    for shot in cut["shots"]:
        if shot.get("sound") and "src" in shot and expand(shot["src"]).exists():
            out.append((shot, shot["start"], shot["dur"]))
    return out


def mean_db(path, start=None, dur=None, stream="0:a:0"):
    """The mean loudness of a file's sound (or a stretch of it), in dB."""
    cmd = ["ffmpeg", "-v", "info"] + (["-ss", str(start)] if start is not None else []) + \
          (["-t", str(dur)] if dur is not None else []) + ["-i", str(path), "-map", stream, "-af", "volumedetect",
                                                          "-f", "null", "-"]
    m = re.search(r"mean_volume: (-?[\d.]+) dB", subprocess.run(cmd, capture_output=True, text=True).stderr)
    return float(m[1]) if m else None


def level_db(shot, music, cut):
    """How much to lift a shot's own speech so it sits a little above the music
    around it: phone sound is far quieter than a mastered score. Applied after a
    gentle compressor, so loud words don't jump out."""
    if music is None:
        return 0.0
    speech = mean_db(expand(shot["src"]), shot.get("in", 0.0), shot["dur"])
    bed = mean_db(music, stream="0:a:0")
    if speech is None or bed is None:
        return 0.0
    bed += 20 * math.log10(max(cut.get("music_gain", 1.0), 1e-6))
    # The compressor takes about 3 dB off speech; aim 3 dB over the music.
    return max(-6.0, min(24.0, bed + 3.0 - speech + 3.0))


def duck_expr(slots, level, ramp=0.35):
    """An ffmpeg volume expression: 1 outside the slots, `level` inside them,
    easing down over `ramp` seconds before each and back up after it."""
    terms = []
    for _, start, dur in slots:
        a, b = start, start + dur
        terms.append(f"clip(min((t-{a - ramp:.3f})/{ramp},({b + ramp:.3f}-t)/{ramp}),0,1)")
    return f"1-{1 - level:.3f}*min(1,{'+'.join(terms)})" if terms else "1"


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
    n = len(segs)
    caps = sorted(cut.get("captions", []), key=lambda c: c["at"])
    if caps:
        st = caption_style(cut, h)
        cdir = Path(tempfile.mkdtemp(prefix="film-captions-"))
        for j, c in enumerate(caps):
            png = cdir / f"{j:03d}.png"
            x, y = render_caption(c, st, w, h, png)
            inputs += ["-loop", "1", "-framerate", str(fps), "-t", f"{c['dur']:.3f}", "-i", str(png)]
            f = min(0.15, c["dur"] / 4)
            chain.append(f"[{n}:v]format=rgba,fade=t=in:st=0:d={f}:alpha=1,"
                         f"fade=t=out:st={c['dur'] - f:.3f}:d={f}:alpha=1,setpts=PTS-STARTPTS+{c['at']}/TB[c{j}]")
            chain.append(f"{label}[c{j}]overlay=x={x}:y={y}:eof_action=pass[k{j}]")
            label = f"[k{j}]"
            n += 1
    fin = cut.get("fade_in", 0.0)
    fout = cut.get("fade_out", 0.0)
    total = cut["total"]
    post = f"{label}trim=end={total},setpts=PTS-STARTPTS"
    if fin:
        post += f",fade=t=in:st=0:d={fin}"
    if fout:
        post += f",fade=t=out:st={total - fout}:d={fout}"
    chain.append(post + "[v]")

    audio = []
    music = expand(cut["music"]) if cut.get("music") else None
    inputs += ["-i", str(music)] if music else ["-f", "lavfi", "-i", "anullsrc=r=48000:cl=stereo"]
    music_timing = ""
    if "timeline" in cut:
        import timeline_bridge
        music_timing = timeline_bridge.call(Path(__file__).resolve().parents[2], "music", cut)["filter"] + ","
    a = f"[{n}:a]{music_timing}atrim=end={total},asetpts=PTS-STARTPTS,volume={cut.get('music_gain', 1.0)}"
    nxt = n + 1
    spoken = sound_slots(cut)
    voices = voice_slots(cut)
    if spoken or voices:
        # The music ducks under every shot that keeps its own speech and every voiceover clip.
        a += f",volume='{duck_expr(spoken + [(None, v[1], v[2]) for v in voices], cut.get('music_duck', 0.2))}':eval=frame"
    if fout:
        a += f",afade=t=out:st={total - fout}:d={fout}"
    beds = []
    audio.append(a + "[m]")
    for j, (v, at, dur) in enumerate(voices):
        # A voiceover clip, from its time, at its own length.
        inputs += ["-i", str(expand(v["src"]))]
        ms = int(round(at * 1000))
        # Lifted like the whiteboard's speech, so a voiceover sits at the same level as a person talking.
        lift = level_db({"src": v["src"], "in": 0.0, "dur": dur}, music, cut)
        audio.append(f"[{nxt}:a:0]aresample=48000,aformat=channel_layouts=stereo,"
                     f"acompressor=threshold=-26dB:ratio=3:attack=5:release=150,volume={lift:.1f}dB,"
                     f"volume={v.get('gain', 1.0)},adelay={ms}|{ms}[vo{j}]")
        beds.append(f"[vo{j}]")
        nxt += 1
    for j, (shot, start, dur) in enumerate(spoken):
        # The shot's own sound, exactly its slot, with short fades so a cut in a pause doesn't click.
        inputs += ["-ss", str(shot.get("in", 0.0)), "-t", f"{dur:.3f}", "-i", str(expand(shot["src"]))]
        ms = int(round(start * 1000))
        f = min(0.05, dur / 4)
        audio.append(f"[{nxt}:a:0]aresample=48000,aformat=channel_layouts=stereo,atrim=end={dur:.3f},"
                     f"asetpts=PTS-STARTPTS,afade=t=in:d={f},afade=t=out:st={dur - f:.3f}:d={f},"
                     f"acompressor=threshold=-26dB:ratio=3:attack=5:release=150,"
                     f"volume={level_db(shot, music, cut):.1f}dB,"
                     f"volume={shot['sound'].get('gain', 1.0)},adelay={ms}|{ms}[sp{j}]")
        beds.append(f"[sp{j}]")
        nxt += 1
    if beds:
        audio.append(f"[m]{''.join(beds)}amix=inputs={1 + len(beds)}:normalize=0:duration=first,"
                     f"loudnorm=I=-14:TP=-1.0:LRA=11[a]")
    else:
        audio.append("[m]anull[a]" if music is None else "[m]loudnorm=I=-14:TP=-1.0:LRA=11[a]")
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
    ap.add_argument("cmd", choices=["check", "inspect", "render"])
    ap.add_argument("cut")
    ap.add_argument("--preview", action="store_true", help="half size, for quick looks")
    ap.add_argument("--size", help="render at WxH instead (e.g. 960x540); overrides --preview")
    ap.add_argument("--out", help="write here instead of the cut's `out`")
    ap.add_argument("--master", action="store_true", help="also write a ProRes 422 HQ .mov")
    ap.add_argument("--allow-missing", action="store_true", help="draw a card for sources not rendered yet")
    ap.add_argument("--force", action="store_true", help="render even when inspect fails")
    ap.add_argument("--json", action="store_true", help="check: print the result as JSON")
    args = ap.parse_args()

    cut = load(args.cut)
    w, h = cut["size"]
    if args.preview:
        w, h = w // 2, h // 2
    if args.size:
        w, h = (int(v) for v in args.size.lower().split("x"))
    if args.cmd == "check":
        # No rendering: what Video Lab runs on every edit before saving it.
        problems, notes = check(cut, w, h, args.allow_missing)
        if args.json:
            print(json.dumps({"problems": problems, "notes": notes, "shots": len(cut["shots"]),
                              "total": cut["total"]}))
        else:
            for n in notes:
                print(f"note   {n}")
            for p in problems:
                print(f"FAIL   {p}")
            print(f"{len(cut['shots'])} shots, {cut['total']:.1f}s, {len(problems)} problem(s)")
        sys.exit(1 if problems else 0)
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
        out = expand(args.out or cut["out"])
        if args.preview and not args.out:
            out = out.with_name(out.stem + "-preview.mp4")
        assemble(cut, segs, w, h, out, args.master)
        print(out)
    finally:
        shutil.rmtree(work, ignore_errors=True)


if __name__ == "__main__":
    main()
