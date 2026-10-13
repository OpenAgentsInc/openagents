#!/usr/bin/env python3
"""Film review: watch a video locally and leave notes at timestamps.

    scripts/film/review.py [VIDEO] [--port 8791]

Opens a page on http://127.0.0.1:PORT where the owner plays a video, jumps
around, and adds a note at the current time (press N; type or dictate into
the box; Enter saves). Every note grabs the exact frame and a short strip of
frames around it, so agents can see what the note is about.

Everything is written under ~/work/film-out/reviews/<video>-<hash>/:

    notes.json     the notes: id, t (seconds), text, created
    notes.md       the same for reading: time, note, frame paths
    frames/        <t>.png (the frame) and <t>-strip.png (t-1 s .. t+1 s)

Agents read notes.md (or notes.json) and open the frames; see
docs/verse/film/review.md. Stdlib only; ffmpeg and ffprobe must be on PATH.
Binds to 127.0.0.1 only.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import mimetypes
import os
import re
import subprocess
import threading
import time
import uuid
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import parse_qs, urlparse

HOME = Path.home()
REVIEWS = HOME / "work/film-out/reviews"
SOURCES = [HOME / "Downloads", HOME / "work/film-out"]
VIDEO_EXT = {".mp4", ".mov", ".webm", ".m4v"}
DEFAULT = HOME / "Downloads/episode-290-draft.mp4"
LOCK = threading.Lock()


def allowed(path: Path) -> bool:
    try:
        path = path.resolve()
    except OSError:
        return False
    return path.suffix.lower() in VIDEO_EXT and path.is_file() and HOME in path.parents


def review_dir(video: Path) -> Path:
    tag = hashlib.sha1(str(video.resolve()).encode()).hexdigest()[:8]
    safe = re.sub(r"[^A-Za-z0-9._-]+", "-", video.stem)[:60]
    return REVIEWS / f"{safe}-{tag}"


def duration(video: Path) -> float:
    out = subprocess.run(["ffprobe", "-v", "error", "-show_entries", "format=duration",
                          "-of", "csv=p=0", str(video)], capture_output=True, text=True)
    try:
        return float(out.stdout.strip())
    except ValueError:
        return 0.0


def clock(t: float) -> str:
    m, s = divmod(t, 60)
    return f"{int(m)}:{s:06.3f}"


def load(video: Path) -> dict:
    path = review_dir(video) / "notes.json"
    if path.exists():
        return json.loads(path.read_text())
    return {"video": str(video.resolve()), "duration": duration(video), "notes": []}


def save(video: Path, review: dict) -> None:
    folder = review_dir(video)
    folder.mkdir(parents=True, exist_ok=True)
    review["notes"].sort(key=lambda n: n["t"])
    (folder / "notes.json").write_text(json.dumps(review, indent=2) + "\n")
    lines = [
        f"# Review notes: {video.name}",
        "",
        f"- Video: `{review['video']}`",
        f"- Length: {clock(review['duration'])} ({review['duration']:.2f} s)",
        f"- Notes: {len(review['notes'])}, newest {time.strftime('%Y-%m-%d %H:%M')}",
        "- Each note's `frame` is the exact frame at that time; `strip` shows 1 s either side.",
        "- Times are seconds into this exact file; re-cut videos need a new review.",
        "",
    ]
    for n in review["notes"]:
        lines += [f"## {clock(n['t'])} ({n['t']:.2f} s)", "", n["text"].strip() or "(no text)", "",
                  f"- frame: `{folder / n['frame']}`", f"- strip: `{folder / n['strip']}`", ""]
    (folder / "notes.md").write_text("\n".join(lines))


def frame_names(t: float, nid: str) -> tuple[str, str]:
    name = f"{t:08.3f}-{nid}"
    return f"frames/{name}.png", f"frames/{name}-strip.png"


def grab(video: Path, t: float, folder: Path, nid: str) -> None:
    frames = folder / "frames"
    frames.mkdir(parents=True, exist_ok=True)
    name = f"{t:08.3f}-{nid}"
    frame, strip = frames / f"{name}.png", frames / f"{name}-strip.png"
    subprocess.run(["ffmpeg", "-y", "-loglevel", "error", "-ss", f"{t:.3f}", "-i", str(video),
                    "-frames:v", "1", str(frame)], check=False)
    start = max(0.0, t - 1.0)
    subprocess.run(["ffmpeg", "-y", "-loglevel", "error", "-ss", f"{start:.3f}", "-t", "2", "-i", str(video),
                    "-vf", "fps=4,scale=480:-1,tile=8x1", "-frames:v", "1", str(strip)], check=False)


def repair() -> None:
    """Give every note its own frame files (older notes shared them by time) and remake missing ones."""
    for path in REVIEWS.glob("*/notes.json"):
        review = json.loads(path.read_text())
        video = Path(review["video"])
        if not allowed(video):
            continue
        changed = False
        for n in review["notes"]:
            frame, strip = frame_names(n["t"], n["id"])
            if (n.get("frame"), n.get("strip")) != (frame, strip):
                n["frame"], n["strip"] = frame, strip
                changed = True
            if not (path.parent / frame).exists() or not (path.parent / strip).exists():
                grab(video, n["t"], path.parent, n["id"])
        if changed:
            with LOCK:
                fresh = load(video)
                byid = {n["id"]: n for n in review["notes"]}
                for n in fresh["notes"]:
                    if n["id"] in byid:
                        n["frame"], n["strip"] = byid[n["id"]]["frame"], byid[n["id"]]["strip"]
                save(video, fresh)


def videos() -> list[dict]:
    found = []
    for root in SOURCES:
        if not root.exists():
            continue
        depth = len(root.parts)
        for dirpath, dirnames, filenames in os.walk(root):
            if len(Path(dirpath).parts) - depth >= 3 or "/." in dirpath or ".cache" in dirpath:
                dirnames[:] = []
            for f in filenames:
                p = Path(dirpath) / f
                if p.suffix.lower() in VIDEO_EXT:
                    try:
                        found.append({"path": str(p), "name": f, "mtime": p.stat().st_mtime,
                                      "notes": len(load(p)["notes"]) if (review_dir(p) / "notes.json").exists() else 0})
                    except OSError:
                        pass
    found.sort(key=lambda v: -v["mtime"])
    return found[:400]


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *args):
        pass

    def send(self, code, body, kind="application/json"):
        data = body if isinstance(body, bytes) else json.dumps(body).encode()
        self.send_response(code)
        self.send_header("Content-Type", kind)
        self.send_header("Content-Length", str(len(data)))
        self.send_header("Cache-Control", "no-store")
        self.end_headers()
        self.wfile.write(data)

    def video_arg(self, q) -> Path | None:
        raw = (q.get("video") or [""])[0]
        p = Path(raw).expanduser()
        return p if raw and allowed(p) else None

    def body(self) -> dict:
        n = int(self.headers.get("Content-Length") or 0)
        return json.loads(self.rfile.read(n) or b"{}")

    def do_GET(self):
        u = urlparse(self.path)
        q = parse_qs(u.query)
        if u.path == "/":
            return self.send(200, PAGE.encode(), "text/html; charset=utf-8")
        if u.path == "/api/videos":
            return self.send(200, {"videos": videos(), "default": str(DEFAULT)})
        if u.path == "/api/review":
            v = self.video_arg(q)
            if not v:
                return self.send(404, {"error": "no such video"})
            r = load(v)
            r["folder"] = str(review_dir(v))
            return self.send(200, r)
        if u.path == "/video":
            v = self.video_arg(q)
            return self.stream(v) if v else self.send(404, {"error": "no such video"})
        if u.path.startswith("/files/"):
            v = self.video_arg(q)
            if not v:
                return self.send(404, b"", "text/plain")
            rel = u.path[len("/files/"):]
            f = (review_dir(v) / rel).resolve()
            if review_dir(v).resolve() not in f.parents or not f.is_file():
                return self.send(404, b"", "text/plain")
            return self.send(200, f.read_bytes(), mimetypes.guess_type(f.name)[0] or "application/octet-stream")
        self.send(404, {"error": "not found"})

    def stream(self, v: Path):
        size = v.stat().st_size
        rng = self.headers.get("Range")
        start, end = 0, size - 1
        if rng and (m := re.match(r"bytes=(\d*)-(\d*)", rng)):
            if m.group(1):
                start = int(m.group(1))
                end = int(m.group(2)) if m.group(2) else size - 1
            else:
                start = size - int(m.group(2))
            end = min(end, size - 1)
            self.send_response(206)
            self.send_header("Content-Range", f"bytes {start}-{end}/{size}")
        else:
            self.send_response(200)
        self.send_header("Content-Type", mimetypes.guess_type(v.name)[0] or "video/mp4")
        self.send_header("Accept-Ranges", "bytes")
        self.send_header("Content-Length", str(end - start + 1))
        self.end_headers()
        with open(v, "rb") as fh:
            fh.seek(start)
            left = end - start + 1
            try:
                while left > 0:
                    chunk = fh.read(min(1 << 20, left))
                    if not chunk:
                        break
                    self.wfile.write(chunk)
                    left -= len(chunk)
            except (BrokenPipeError, ConnectionResetError):
                pass

    def do_POST(self):
        u = urlparse(self.path)
        if u.path != "/api/notes":
            return self.send(404, {"error": "not found"})
        b = self.body()
        v = Path(b.get("video", "")).expanduser()
        if not allowed(v):
            return self.send(404, {"error": "no such video"})
        t = max(0.0, float(b.get("t", 0)))
        nid = uuid.uuid4().hex[:8]
        frame, strip = frame_names(t, nid)
        note = {"id": nid, "t": round(t, 3), "text": str(b.get("text", "")).strip(),
                "created": time.strftime("%Y-%m-%dT%H:%M:%S"), "frame": frame, "strip": strip}
        with LOCK:
            review = load(v)
            review["notes"].append(note)
            save(v, review)
        # The note is saved now; its frames follow in the background (a few seconds on long files).
        threading.Thread(target=grab, args=(v, t, review_dir(v), nid), daemon=True).start()
        self.send(200, note)

    def do_PATCH(self):
        b = self.body()
        v = Path(b.get("video", "")).expanduser()
        if not allowed(v):
            return self.send(404, {"error": "no such video"})
        with LOCK:
            review = load(v)
            for n in review["notes"]:
                if n["id"] == b.get("id"):
                    n["text"] = str(b.get("text", "")).strip()
                    save(v, review)
                    return self.send(200, n)
        self.send(404, {"error": "no such note"})

    def do_DELETE(self):
        q = parse_qs(urlparse(self.path).query)
        v = self.video_arg(q)
        nid = (q.get("id") or [""])[0]
        if not v:
            return self.send(404, {"error": "no such video"})
        with LOCK:
            review = load(v)
            keep = [n for n in review["notes"] if n["id"] != nid]
            gone = [n for n in review["notes"] if n["id"] == nid]
            review["notes"] = keep
            save(v, review)
        for n in gone:
            for k in ("frame", "strip"):
                (review_dir(v) / n[k]).unlink(missing_ok=True)
        self.send(200, {"deleted": len(gone)})


AGENT_README = """# Owner film reviews

The owner watches videos in `scripts/film/review.py` and leaves notes at timestamps.
Each reviewed video has a folder here, `<video>-<hash>/`, with:

- `notes.md`: every note in time order, with the frame paths. Read this first.
- `notes.json`: the same, for scripts (`t` is seconds into that exact file).
- `frames/<t>.png`: the exact frame at the note; `frames/<t>-strip.png`: 1 s either side at 4 fps.

When you work on a video the owner reviewed, read its `notes.md` and open the frames
before changing anything. Treat each note as a change request, and say which notes a
new cut addresses. Times only match the file the notes were made on: a re-cut needs a
new review. See docs/verse/film/review.md.
"""


PAGE = r"""<!doctype html><meta charset=utf-8><title>Film review</title>
<style>
:root{--bg:#0b0b0c;--panel:#141416;--line:#26262b;--text:#e8e8ea;--dim:#8a8a90;--accent:#f2c94c}
*{box-sizing:border-box}body{margin:0;background:var(--bg);color:var(--text);font:14px/1.45 -apple-system,system-ui,sans-serif;height:100vh;display:grid;grid-template-columns:1fr 380px}
main{display:flex;flex-direction:column;min-width:0;padding:14px;gap:10px}
header{display:flex;gap:10px;align-items:center}header select{flex:1;background:var(--panel);color:var(--text);border:1px solid var(--line);border-radius:8px;padding:7px}
video{width:100%;max-height:62vh;background:#000;border-radius:10px}
#bar{position:relative;height:30px;background:var(--panel);border:1px solid var(--line);border-radius:8px;cursor:pointer}
#prog{position:absolute;top:0;bottom:0;left:0;background:#2a2a30;border-radius:8px 0 0 8px}
.mk{position:absolute;top:3px;bottom:3px;width:3px;background:var(--accent);border-radius:2px;transform:translateX(-1px)}
.mk:hover{width:5px}#cur{position:absolute;top:0;bottom:0;width:2px;background:#fff}
.row{display:flex;gap:8px;align-items:center;color:var(--dim)}.row b{color:var(--text);font-variant-numeric:tabular-nums}
#notebox{display:none;background:var(--panel);border:1px solid var(--accent);border-radius:10px;padding:10px}
#notebox textarea{width:100%;min-height:70px;background:#0e0e10;color:var(--text);border:1px solid var(--line);border-radius:8px;padding:8px;font:inherit;resize:vertical}
button{background:#232328;color:var(--text);border:1px solid #333339;border-radius:8px;padding:6px 12px;cursor:pointer}button:hover{background:#2c2c32}
button.primary{background:var(--accent);color:#111;border-color:var(--accent)}
aside{background:#0f0f11;border-left:1px solid var(--line);overflow:auto;padding:14px}
aside h2{font-size:14px;margin:0 0 4px}.where{color:var(--dim);font-size:12px;word-break:break-all;margin-bottom:10px}
.note{border:1px solid var(--line);border-radius:10px;padding:8px;margin-bottom:8px;cursor:pointer}.note:hover{border-color:#3a3a40}
.note .t{color:var(--accent);font-variant-numeric:tabular-nums;font-weight:600}.note img{width:100%;border-radius:6px;margin-top:6px}
.note .txt{white-space:pre-wrap;margin-top:4px}.note .act{display:flex;gap:6px;margin-top:6px}.note .act button{padding:2px 8px;font-size:12px}
kbd{background:#232328;border:1px solid #333;border-radius:4px;padding:0 5px;font-size:12px}
</style>
<main>
<header><select id=pick></select><button id=reload title="Rescan videos">↻</button></header>
<video id=v controls preload=auto></video>
<div id=bar><div id=prog></div><div id=cur></div></div>
<div class=row><b id=time>0:00.000</b> / <span id=dur>0:00</span> · <kbd>Space</kbd> play · <kbd>←</kbd><kbd>→</kbd> 1 s (<kbd>Shift</kbd> 5 s) · <kbd>,</kbd><kbd>.</kbd> frame · <kbd>N</kbd> note · <kbd>[</kbd><kbd>]</kbd> prev/next note</div>
<div id=notebox><div class=row>Note at <b id=noteat></b> · Enter saves · Shift+Enter new line · Esc cancels <label style="margin-left:auto"><input type=checkbox id=resume checked> resume after saving</label></div>
<textarea id=notetext placeholder="Talk or type what you think about this moment…"></textarea>
<div class=row style="margin-top:6px"><button class=primary id=savenote>Save note</button><button id=cancelnote>Cancel</button></div></div>
</main>
<aside><h2 id=count>Notes</h2><div class=where id=where></div><div id=list></div></aside>
<script>
const $=id=>document.getElementById(id), v=$('v');let video='',review={notes:[]},noteT=0,wasPlaying=false;
const clock=t=>{const m=Math.floor(t/60),s=(t%60).toFixed(3).padStart(6,'0');return m+':'+s};
const enc=encodeURIComponent;
async function api(path,opt){const r=await fetch(path,opt);return r.json()}
async function loadVideos(sel){const d=await api('/api/videos');const p=$('pick');p.innerHTML='';
 const want=sel||new URLSearchParams(location.search).get('video')||localStorage.getItem('reviewVideo')||d.default;
 if(!d.videos.find(x=>x.path===want))d.videos.unshift({path:want,name:want.split('/').pop(),notes:0});
 for(const x of d.videos){const o=document.createElement('option');o.value=x.path;o.textContent=x.name+(x.notes?`  (${x.notes} notes)`:'')+'  — '+x.path.replace(/^\/Users\/[^/]+/,'~');p.appendChild(o)}
 p.value=want;open(want)}
async function open(path){video=path;localStorage.setItem('reviewVideo',path);history.replaceState(null,'','?video='+enc(path));
 v.src='/video?video='+enc(path);review=await api('/api/review?video='+enc(path));render()}
function render(){const L=$('list');L.innerHTML='';$('count').textContent=`Notes (${review.notes.length})`;$('where').textContent=review.folder?'Saved to '+review.folder.replace(/^\/Users\/[^/]+/,'~')+'/notes.md':'';
 const bar=$('bar');bar.querySelectorAll('.mk').forEach(e=>e.remove());const D=v.duration||review.duration||1;
 for(const n of review.notes){const m=document.createElement('div');m.className='mk';m.style.left=(n.t/D*100)+'%';m.title=clock(n.t)+' '+n.text;m.onclick=e=>{e.stopPropagation();seek(n.t)};bar.appendChild(m);
  const d=document.createElement('div');d.className='note';d.innerHTML=`<span class=t>${clock(n.t)}</span><div class=txt></div><img loading=lazy src="/files/${n.frame}?video=${enc(video)}"><div class=act><button data-a=edit>Edit</button><button data-a=del>Delete</button></div>`;
  d.querySelector('.txt').textContent=n.text;retryImg(d.querySelector('img'));d.onclick=e=>{const a=e.target.dataset.a;if(a==='del'){if(confirm('Delete this note?'))del(n.id);return}if(a==='edit'){edit(n);return}seek(n.t)};L.appendChild(d)}}
function seek(t){v.currentTime=Math.max(0,Math.min(t,v.duration||t))}
function tick(){const D=v.duration||1;$('prog').style.width=(v.currentTime/D*100)+'%';$('cur').style.left=(v.currentTime/D*100)+'%';$('time').textContent=clock(v.currentTime);$('dur').textContent=clock(D).replace(/\.\d+$/,'');requestAnimationFrame(tick)}
$('bar').onclick=e=>{const r=$('bar').getBoundingClientRect();seek((e.clientX-r.left)/r.width*(v.duration||0))};
function startNote(){wasPlaying=!v.paused;v.pause();noteT=v.currentTime;$('noteat').textContent=clock(noteT);$('notebox').style.display='block';$('notetext').value='';$('notetext').focus()}
function closeNote(){$('notebox').style.display='none';v.focus?.()}
let saving=false;
async function saveNote(){const text=$('notetext').value.trim();if(!text){closeNote();return}if(saving)return;saving=true;
 $('savenote').disabled=true;$('savenote').textContent='Saving…';
 try{const r=await fetch('/api/notes',{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify({video,t:noteT,text})});if(!r.ok)throw new Error(r.status);
  review=await api('/api/review?video='+enc(video));render();closeNote();if($('resume').checked&&wasPlaying)v.play()}
 catch(e){alert('Could not save the note ('+e.message+'). Your text is still in the box.')}
 finally{saving=false;$('savenote').disabled=false;$('savenote').textContent='Save note'}}
function retryImg(img){let n=0;img.onerror=()=>{if(n++<20)setTimeout(()=>{img.src=img.src.split('&r=')[0]+'&r='+n},1000)}}
async function edit(n){const t=prompt('Edit note at '+clock(n.t),n.text);if(t===null)return;await api('/api/notes',{method:'PATCH',headers:{'content-type':'application/json'},body:JSON.stringify({video,id:n.id,text:t})});review=await api('/api/review?video='+enc(video));render()}
async function del(id){await api('/api/notes?video='+enc(video)+'&id='+id,{method:'DELETE'});review=await api('/api/review?video='+enc(video));render()}
$('savenote').onclick=saveNote;$('cancelnote').onclick=closeNote;$('pick').onchange=e=>open(e.target.value);$('reload').onclick=()=>loadVideos(video);
$('notetext').onkeydown=e=>{if(e.key==='Enter'&&!e.shiftKey){e.preventDefault();saveNote()}if(e.key==='Escape'){e.preventDefault();closeNote()}};
// Keys are ours even when the player has focus: Chrome's own player also toggles on Space, which undid ours.
v.addEventListener('focus',()=>v.blur());
window.addEventListener('keyup',e=>{if(e.code==='Space'&&e.target.tagName!=='TEXTAREA'){e.preventDefault();e.stopPropagation()}},true);
window.addEventListener('keydown',e=>{if(e.target.tagName==='TEXTAREA'||e.target.tagName==='SELECT'||e.metaKey||e.ctrlKey)return;const step=e.shiftKey?5:1;if(['Space','ArrowLeft','ArrowRight'].includes(e.code)||e.target===v)e.stopPropagation();
 if(e.code==='Space'){e.preventDefault();v.paused?v.play():v.pause()}else if(e.key==='ArrowRight'){e.preventDefault();seek(v.currentTime+step)}else if(e.key==='ArrowLeft'){e.preventDefault();seek(v.currentTime-step)}
 else if(e.key==='.'){v.pause();seek(v.currentTime+1/30)}else if(e.key===','){v.pause();seek(v.currentTime-1/30)}else if(e.key==='n'||e.key==='N'){e.preventDefault();startNote()}
 else if(e.key===']'){const n=review.notes.find(x=>x.t>v.currentTime+0.05);if(n)seek(n.t)}else if(e.key==='['){const n=[...review.notes].reverse().find(x=>x.t<v.currentTime-0.05);if(n)seek(n.t)}},true);
v.addEventListener('loadedmetadata',render);loadVideos();tick();
</script>"""


def main():
    global DEFAULT
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("video", nargs="?", help="video to open first")
    ap.add_argument("--port", type=int, default=8791)
    a = ap.parse_args()
    if a.video:
        DEFAULT = Path(a.video).expanduser().resolve()
    REVIEWS.mkdir(parents=True, exist_ok=True)
    (REVIEWS / "README.md").write_text(AGENT_README)
    threading.Thread(target=repair, daemon=True).start()
    server = ThreadingHTTPServer(("127.0.0.1", a.port), Handler)
    print(f"Film review on http://127.0.0.1:{a.port}/  (notes in {REVIEWS})", flush=True)
    server.serve_forever()


if __name__ == "__main__":
    main()
