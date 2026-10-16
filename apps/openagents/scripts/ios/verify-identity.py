#!/usr/bin/env python3
"""Verify identity, icon, notices, and endpoints in a built iOS app."""
import pathlib
import plistlib
import subprocess
import sys

app = pathlib.Path(sys.argv[1])
with (app / "Info.plist").open("rb") as source:
    info = plistlib.load(source)
assert info["CFBundleIdentifier"] == "com.openagents.coder", info
assert info["CFBundleDisplayName"] == "OpenAgents Coder", info
assert info["CFBundleURLTypes"][0]["CFBundleURLSchemes"] == ["openagents-coder"]
assert (app / "Assets.car").is_file(), "Missing asset catalog"
assert "Copyright (c) 2026 Wing" in (app / "Zeron-MIT.txt").read_text()
for extension in (app / "PlugIns").glob("*.appex"):
    with (extension / "Info.plist").open("rb") as source:
        assert plistlib.load(source)["CFBundleIdentifier"].startswith("com.openagents.coder.")
for path in app.rglob("*"):
    if path.is_file():
        result = subprocess.run(["/usr/bin/strings", "-a", str(path)], capture_output=True, check=True)
        assert b"zeron.sh" not in result.stdout, f"Upstream endpoint in {path}"
print("Coder identity, attribution, extensions, assets, and endpoint scan passed")
