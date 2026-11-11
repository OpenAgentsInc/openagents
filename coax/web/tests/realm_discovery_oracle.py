# Firm reason: 2 (independent set oracle for deployed public discovery and registry/chapter projections).
"""Exercise deployment's actual discovery section without building/uploading a site."""
import json
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[3]
APP = ROOT / "apps/openagents-web"
worlds = json.loads((APP / "realms.json").read_text())["worlds"]
expected_hidden = {
    "realms/" + w["realm"]["slug"] + "/"
    for w in worlds if w.get("listed") is False and w.get("realm")
}
observed = set(subprocess.check_output(
    [str(ROOT / "coax/web/tool.sh"), "discovery-realms", str(APP / "realms.json")],
    cwd=ROOT, text=True).splitlines())
assert observed == expected_hidden
assert "realms/bitcoin-lab/" in observed

# Execute exactly the discovery code that deploy.sh runs, with isolated files.
source = (ROOT / "coax/web/deploy.sh").read_text()
discovery = source.split('registry = os.path.join(app, "realms.json")\n', 1)[1].split('\nPY\n', 1)[0]
discovery = 'registry = os.path.join(app, "realms.json")\n' + discovery
with tempfile.TemporaryDirectory(prefix="realm-discovery-oracle-") as tmp:
    root = Path(tmp)
    app, site = root / "app", root / "site"
    app.mkdir(); site.mkdir()
    (app / "realms.json").write_text((APP / "realms.json").read_text())
    (app / "llms.txt").write_text("Public test catalog\n")
    candidates = {"docs/public/index.md", "realms/public/index.md", "projects/public/index.md",
                  "projects/private/index.md", "projects/hidden/index.md", "admin/index.md"}
    for prefix in expected_hidden:
        candidates.update({prefix + "index.md", prefix + "child/index.md"})
    for name in candidates:
        path = app / name; path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text("# " + name)
    for project, visibility in [("public", "public"), ("hidden", "unlisted"), ("private", "private")]:
        (app / "projects" / project / "project.json").write_text(json.dumps({"visibility": visibility}))
    import os
    scope = dict(os=os, json=json, subprocess=subprocess, app=str(app), site=str(site),
                 tools=str(ROOT / "coax/web"), pages=sorted(candidates))
    exec(compile(discovery, "deploy.sh discovery section", "exec"), scope)
    assert set(scope["public"]) == {"docs/public/index.md", "realms/public/index.md", "projects/public/index.md"}
    for output in ["sitemap.xml", "llms-full.txt"]:
        text = (site / output).read_text()
        assert "bitcoin-lab" not in text
        assert all(prefix.rstrip("/") not in text for prefix in expected_hidden)

# Compare generated status/source rows independently with the canonical registry.
chapters = json.loads((ROOT / "bitcoin/book/chapters.json").read_text())["chapters"]
from html.parser import HTMLParser
class MapParser(HTMLParser):
    def __init__(self):
        super().__init__(); self.rows=[]; self.row=None; self.cell=None; self.links=[]
    def handle_starttag(self, tag, attrs):
        if tag == "tr": self.row=[]
        if tag == "td": self.cell=""
        if tag == "a" and self.cell is not None: self.links.append(dict(attrs)["href"])
    def handle_data(self, data):
        if self.cell is not None: self.cell += data
    def handle_endtag(self, tag):
        if tag == "td": self.row.append(self.cell); self.cell=None
        if tag == "tr" and self.row: self.rows.append(self.row)
parser = MapParser(); parser.feed((APP / "realms/bitcoin-lab/map.fallback").read_text())
assert len(parser.rows) == len(chapters) == 125
for row, link, chapter in zip(parser.rows, parser.links, chapters):
    assert row[0] == f'{chapter["number"]}. {chapter["title"]}'
    # chapters 1, 4 and 5 are taught at the walk's stations (apps/openagents-web/bitcoin-lab-walk.coax)
    assert row[2] == chapter["status"] and row[3] == ("at the stations" if chapter["number"] in (1, 4, 5) else "not available")
    if chapter["file"]:
        assert link.endswith("/bitcoin/book/" + chapter["file"])
        assert (ROOT / "bitcoin/book" / chapter["file"]).is_file()
    else:
        line = int(link.split("PLAN.md#L")[1])
        assert (ROOT / "bitcoin/book/PLAN.md").read_text().splitlines()[line-1].startswith(f'| {chapter["number"]}. ')
for path in ["realms/index.html", "realms/index.md", "realms/list.coax", "index.html", "llms.txt"]:
    assert "bitcoin-lab" not in (APP / path).read_text(), path
for world in worlds:
    if world.get("listed") is False and world.get("realm"):
        html = (APP / "realms" / world["realm"]["slug"] / "index.html").read_text()
        assert '<meta name="robots" content="noindex, nofollow">' in html
print(f"PASS: {len(expected_hidden)} unlisted realm roots and children excluded; all 125 chapter/status/source rows match")
