# Firm reason: 1 (it drives the existing Python retargeter, metaverse/tools/people.py, whose clip and rig writers it reuses).
"""A clip-only rig: a pack manifest's clips retargeted onto the people's two skeletons, with no parts.

An output named .vrg is the rig as an asset: one set of a VRG1 file (people.rig_vrg), named by the
third argument. The developer's preview clips are written this way and no other (#13306): their keys
are not source.

  python3 metaverse/tools/clip_rig.py metaverse/packs/everglade-animation-preview.json /tmp/preview.vrg preview
  coax/web/tool.sh everglade/model/tools/rig-pack.coax --set everglade/model/rigs.vrg /tmp/preview.vrg everglade/model/rigs.vrg
  coax/web/tool.sh everglade/model/tools/rig-pack.coax --check everglade/model/rigs.vrg

The last line holds the file to everglade/scene/preview-clips.coax, which states the clips' numbers,
names and seconds by hand: change it when the clips change.

Any other output is the rig as code names it (people.rig_coax): the clips' numbers, names, seconds,
strides and parents, with no keys (#13508). Write the set's keys as above, then its names file:

  python3 metaverse/tools/clip_rig.py metaverse/packs/everglade-two-hand.json /tmp/two-hand.vrg two-hand
  python3 metaverse/tools/clip_rig.py metaverse/packs/everglade-two-hand.json everglade/scene/two-hand-clips.coax TWOHAND two-hand

The manifest names "libraries" (the first gives the library rest the clips' pelvis moves are taken
from), "clips" ({name, library, clip}) and the "skeletons" (male, female), as
metaverse/packs/everglade-animation-preview.json does.
"""
import json, sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parent))
import people


def main():
    if len(sys.argv) not in (4, 5) or (len(sys.argv) == 5) == sys.argv[2].endswith(".vrg"):
        raise SystemExit("usage: clip_rig.py MANIFEST OUT.coax PREFIX SET | MANIFEST OUT.vrg SET")
    manifest = json.loads(Path(sys.argv[1]).read_text())
    skeletons = {k: people.Skeleton(people.load(v)) for k, v in manifest["skeletons"].items()}
    if skeletons["male"].names != skeletons["female"].names:
        raise SystemExit("clip_rig: the male and female skeletons aren't the same joints")
    library_rest = people.Skeleton(people.load(next(iter(manifest["libraries"].values())))).t
    cl = people.clips(manifest, skeletons, library_rest)
    if sys.argv[2].endswith(".vrg"):
        Path(sys.argv[2]).write_bytes(people.rig_vrg((skeletons, cl), sys.argv[3]))
        return
    Path(sys.argv[2]).write_text(people.rig_coax(manifest, (skeletons, cl), [], sys.argv[3], sys.argv[4], "clip_rig.py"))


main()
