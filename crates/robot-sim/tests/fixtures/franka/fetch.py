"""Fetch the pinned, licensed Franka fixture into an explicit scratch folder."""

import concurrent.futures
import hashlib
import json
import pathlib
import sys
import urllib.request

pin = json.loads(pathlib.Path(__file__).with_name("source.json").read_text())
destination = pathlib.Path(sys.argv[1]).resolve()
destination.mkdir(parents=True, exist_ok=True)


def fetch(item):
    name, expected = item
    path = destination / name
    assert not path.is_symlink() and path.resolve().is_relative_to(destination)
    url = (
        "https://raw.githubusercontent.com/google-deepmind/mujoco_menagerie/"
        f"{pin['revision']}/{pin['directory']}/{name}"
    )
    with urllib.request.urlopen(url, timeout=30) as response:
        data = response.read(expected["bytes"] + 1)
    assert len(data) == expected["bytes"]
    assert hashlib.sha256(data).hexdigest() == expected["sha256"]
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(data)


with concurrent.futures.ThreadPoolExecutor(max_workers=6) as pool:
    list(pool.map(fetch, pin["files"].items()))
print(destination)
