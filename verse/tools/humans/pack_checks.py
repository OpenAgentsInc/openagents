"""Checks a human pack as MetaHuman Lab draws it, without anyone looking at a frame.

    python3 verse/tools/humans/pack_checks.py [PACK] [--url http://127.0.0.1:8802] [--source IMAGE]

PACK is a pack name under ~/.openagents/verse/private-build/web (default human; standin is the CC0
stand-in). The lab must be served locally (coax/web/serve.sh apps/openagents-web --listen
127.0.0.1:8802). Frames are shot headless into ~/.openagents/verse/private-build/renders (private:
never committed, never read by an agent or used to train AI models) in the lab's probe views (its
unlisted V key) and only reduced to numbers here:

  capsules  no pixel of the capsule figure is drawn with the pack in (and some are without one,
            so the probe works)
  albedo    on the face's base layer, the rendered albedo against the source texture at each
            pixel's own uv: the mean difference, and the hue and saturation per uv region (a map in
            the wrong slot, a wrong colour space or a flipped uv shows here)
  skin      the face's pixels are drawn as skin (material 1, the scatter path), from one frame
            that carries each pixel's material and base layer
  face      with a face rig in the pack: the rig runs (the material view's blue), and an
            expression (the lab's X) moves the face: the share of the face's pixels whose uv
            changes
  frame     the share of the frame the figure covers

Exits non-zero if a check fails.
"""
import colorsys, os, re, subprocess, sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
WEB = Path.home() / ".openagents/verse/private-build/web"
RENDERS = Path.home() / ".openagents/verse/private-build/renders"
HUD_TOP, HUD_BOTTOM = 70, 960


def shot(url, out, views, face=True, expressions=0):
    """A frame after pressing V `views` times; face: the lab's unlisted F first (the clip paused at
    its start, the camera on the face: every load the same frame, so the views line up)."""
    keys = ("window.dispatchEvent(new KeyboardEvent('keydown',{key:'f'}));window.dispatchEvent(new KeyboardEvent('keyup',{key:'f'}));"
            if face else "")
    keys += (f"for(let i=0;i<{expressions};i++){{window.dispatchEvent(new KeyboardEvent('keydown',{{key:'x'}}));"
             "window.dispatchEvent(new KeyboardEvent('keyup',{key:'x'}));}")
    keys += (f"for(let i=0;i<{views};i++){{window.dispatchEvent(new KeyboardEvent('keydown',{{key:'v'}}));"
             "window.dispatchEvent(new KeyboardEvent('keyup',{key:'v'}));}")
    lift = "new Promise(r=>setTimeout(()=>r('ok'),900))"
    subprocess.run([str(ROOT / "coax/web/tool.sh"), "shot", url, str(out), "1600x1000", "6000", "--eval", keys + lift],
                   check=True, capture_output=True, cwd=ROOT)
    from PIL import Image
    return Image.open(out).convert("RGB")


def catalog(pack):
    """The catalog's text, near enough: from before its box line to its end (a NUL when a face rig's
    block follows)."""
    b = pack.read_bytes(); i = b.find(b"\nbox")
    end = b.find(b"\0", i, i + 4000)
    return b[max(0, i - 12000):end if end > 0 else i + 400].decode("latin1")


def face_layer(cat):
    """The face's base layer: the head piece's, as the catalog names its source."""
    for want in ("head_base_ao.png", "Head_Basecolor.png", "T_Superhero_Male_Dark.png"):
        for l in cat.splitlines():
            m = re.match(r"L (\d+) base (\S+)", l)
            if m and m.group(2) == want:
                return int(m.group(1)), want
    raise SystemExit("pack_checks: no face base layer in the catalog")


def capsules(im):
    px = im.load(); w, _ = im.size
    return sum(1 for y in range(HUD_TOP, HUD_BOTTOM) for x in range(w)
               if px[x, y][0] > 200 and px[x, y][1] < 60 and px[x, y][2] > 200)


def albedo(uv, alb, layer, source):
    from PIL import Image
    src = Image.open(source).convert("RGB").resize((256, 256), Image.BOX); sp = src.load()
    U, A = uv.load(), alb.load(); w, _ = uv.size
    pix = [(U[x, y][0] / 255, U[x, y][1] / 255, A[x, y]) for y in range(HUD_TOP, HUD_BOTTOM) for x in range(w)
           if U[x, y][2] == layer and (U[x, y][0] or U[x, y][1])]
    if not pix:
        return False, "no pixels on the face's layer"
    def at(u, v, flip):
        return sp[min(255, int(u * 256)), min(255, int((1 - v if flip else v) * 256))]
    d, flip = min((sum(sum(abs(p - q) for p, q in zip(a, at(u, v, f))) / 3 for u, v, a in pix) / len(pix), f) for f in (False, True))
    reg = {}
    for u, v, a in pix:
        reg.setdefault((min(3, int(u * 4)), min(3, int(v * 4))), []).append((a, at(u, v, flip)))
    lines, worst = [], 0.0
    for k in sorted(reg):
        if len(reg[k]) < 200:
            continue
        def hs(cs):
            r, g, b = (sum(c[i] for c in cs) / len(cs) / 255 for i in range(3))
            h, _, s = colorsys.rgb_to_hls(r, g, b); return h * 360, s
        (ha, sa), (hb, sb) = hs([a for a, _ in reg[k]]), hs([s for _, s in reg[k]])
        dh = min(abs(ha - hb), 360 - abs(ha - hb)); worst = max(worst, dh)
        lines.append(f"    region {k}: {len(reg[k])} px  hue {ha:.0f} vs {hb:.0f}  saturation {sa:.2f} vs {sb:.2f}")
    ok = d < 14 and worst < 10
    return ok, f"{len(pix)} px, mean |albedo - source| {d:.1f}/255, worst region hue off {worst:.1f} deg\n" + "\n".join(lines)


def main(argv):
    name = argv[0] if argv and not argv[0].startswith("--") else "human"
    url = argv[argv.index("--url") + 1] if "--url" in argv else "http://127.0.0.1:8802"
    pack = WEB / f"{name}.pack"
    cat = catalog(pack)
    layer, src_name = face_layer(cat)
    source = Path(argv[argv.index("--source") + 1]) if "--source" in argv else None
    if source is None:
        found = list((Path.home() / ".openagents/verse/private-build/metahuman-dump/portrait").glob(src_name))
        found += list(Path.home().glob(f"Downloads/*/*/Textures/{src_name}"))
        source = found[0] if found else None
    RENDERS.mkdir(parents=True, exist_ok=True); os.chmod(RENDERS, 0o700)
    page = f"{url}/worlds/metahuman-lab/?pack=" + ("" if name == "human" else name)
    failed = []
    none = capsules(shot(f"{url}/worlds/metahuman-lab/", RENDERS / "check-nopack-capsules.png", 3, face=False))
    with_pack = capsules(shot(page, RENDERS / "check-capsules.png", 3, face=False))
    print(f"capsules: {with_pack} capsule pixels with the pack ({none} without one)")
    if with_pack or not none:
        failed.append("capsules")
    uv = shot(page, RENDERS / "check-uv.png", 2); alb = shot(page, RENDERS / "check-albedo.png", 1)
    if source and source.exists():
        ok, text = albedo(uv, alb, layer, source)
        print(f"albedo ({src_name}): " + text)
        if not ok:
            failed.append("albedo")
    else:
        print(f"albedo: no source image for {src_name}; pass --source")
    mat = shot(page, RENDERS / "check-material.png", 5).load()
    n = skin = 0
    for y in range(HUD_TOP, HUD_BOTTOM):
        for x in range(1600):
            if mat[x, y][1] == layer and mat[x, y][0]:
                n += 1; skin += mat[x, y][0] == 1
    print(f"skin: {skin / max(n, 1):.1%} of the face's {n} pixels drawn as skin (the scatter path)")
    if not n or skin / n < 0.95:
        failed.append("skin")
    if re.search(r"^F \d+ \d+ \d+$", cat, re.M):
        runs = sum(1 for y in range(HUD_TOP, HUD_BOTTOM) for x in range(1600) if mat[x, y][1] == layer and mat[x, y][0] and mat[x, y][2] == 255)
        smile = shot(page, RENDERS / "check-uv-expression.png", 2, expressions=1).load()
        still = uv.load()
        moved = sum(1 for y in range(HUD_TOP, HUD_BOTTOM) for x in range(1600)
                    if still[x, y][2] == layer and sum(abs(a - b) for a, b in zip(still[x, y], smile[x, y])) > 6)
        print(f"face: the rig runs on {runs / max(n, 1):.1%} of the face's pixels; an expression moves {moved / max(n, 1):.1%} of them")
        if not n or runs / n < 0.95 or moved / n < 0.02:
            failed.append("face")
    probe = shot(page, RENDERS / "check-figure.png", 2, face=False).load()
    cover = sum(1 for y in range(HUD_TOP, HUD_BOTTOM) for x in range(1600) if any(probe[x, y])) / (1600 * (HUD_BOTTOM - HUD_TOP))
    print(f"frame: the figure covers {cover:.1%} of the frame")
    print("checks", "failed: " + ", ".join(failed) if failed else "passed")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
