"""Check compiled Rust tools with temporary world files and public enrollment."""
import argparse
import json
import pathlib
import subprocess
import tempfile

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--binaries", type=pathlib.Path, required=True)
args = parser.parse_args()
compiler = args.binaries / "verse-content"
host = args.binaries / "verse-host"
# Public generator point; check mode grants no session and needs no private key.
public_key = "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798"
with tempfile.TemporaryDirectory(prefix="verse-v17-tools-") as temporary:
    root = pathlib.Path(temporary)
    for index, recipe in enumerate(["ritual", "observatory"]):
        output = root / recipe
        result = subprocess.run(
            [str(compiler), recipe, str(output)],
            capture_output=True, text=True, check=True,
        )
        compiled = json.loads(result.stdout)
        assert compiled["world"] == recipe and compiled["models"] > 0
        assert all((output / name).is_file() for name in ["pack.json", "scene.json", "profile.json"])
        print("compiler:", result.stdout.strip())
        refused = subprocess.run(
            [str(compiler), recipe, str(output)], capture_output=True, text=True,
        )
        assert refused.returncode == 1 and "must not exist" in refused.stderr
        profile = json.loads((output / "profile.json").read_text())
        config = {
            "listen": "127.0.0.1:0", "instance": 8700 + index,
            "scene": str(output / "scene.json"), "pack": str(output / "pack.json"),
            "certificate_der": str(root / "unopened-cert.der"),
            "private_key_der": str(root / "unopened-key.der"),
            "enrollments": [{"public_key": public_key, "role": {"type": "primary"}}],
            "social_profile": profile,
        }
        path = output / "host.json"
        path.write_text(json.dumps(config))
        checked = subprocess.run(
            [str(host), str(path), "--check", "300"],
            capture_output=True, text=True, check=True,
        )
        receipt = json.loads(checked.stdout)
        assert receipt["ticks"] == 300 and receipt["actors"] > 0
        if profile is None:
            assert receipt["content"] == compiled["content"]
        print("default host:", checked.stdout.strip())
    refused = subprocess.run(
        [str(compiler), "unknown", str(root / "unknown")],
        capture_output=True, text=True,
    )
    assert refused.returncode == 1 and not (root / "unknown").exists()
    print("Both compiler commands, default host checks, and compiler refusals passed")
