#!/usr/bin/env python3
"""Prepare a credential-free native canary installer and reviewed source archive."""
import argparse
import hashlib
from pathlib import Path
import tarfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("destination", type=Path)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[2]
    destination = args.destination.resolve()
    destination.mkdir(mode=0o700, parents=True, exist_ok=True)
    archive = destination / "credential-adapter.tar.gz"
    names = ["crates/computer-credentials", "deploy/space-credentials/apply.py",
             "deploy/space-credentials/gateway.patch", "deploy/space-service/seed-kernel.patch"]
    with tarfile.open(archive, "w:gz") as output:
        for name in names:
            output.add(root / name, arcname=name)
    digest = hashlib.sha256(archive.read_bytes()).hexdigest()
    script = (root / "deploy/space-host/install.sh").read_text()
    anchor = "runuser -l space-spike -c 'bash -s' <<'SETUP'"
    if script.count(anchor) != 1 or script.count('cd "$HOME/SPACE"') != 1:
        raise SystemExit("Review the changed native installer before preparing a canary.")
    preparation = f'''[[ $(hostname -s) == oa-space-spike-11813 ]]
printf '%s  %s\\n' {digest} /home/space-admin/credential-adapter.tar.gz | sha256sum --check
install -d -m 0700 -o space-spike -g space-spike /var/lib/space-spike/home/credential-adapter
install -m 0600 -o space-spike -g space-spike /home/space-admin/credential-adapter.tar.gz /var/lib/space-spike/home/credential-adapter-source.tar.gz
runuser -u space-spike -- tar -xzf /var/lib/space-spike/home/credential-adapter-source.tar.gz -C /var/lib/space-spike/home/credential-adapter
'''
    # Check the dedicated hostname before any package, disk, or user mutation.
    script = script.replace("UPSTREAM=", "[[ $(hostname -s) == oa-space-spike-11813 ]]\nUPSTREAM=", 1)
    script = script.replace(anchor, preparation + anchor)
    script = script.replace('cd "$HOME/SPACE"', '''cd "$HOME/SPACE"
python3 "$HOME/credential-adapter/deploy/space-credentials/apply.py" "$HOME/SPACE"
git apply "$HOME/credential-adapter/deploy/space-service/seed-kernel.patch"''', 1)
    (destination / "install-credential-canary.sh").write_text(script)
    print("Prepared the isolated native credential canary; no keys are included.")


if __name__ == "__main__":
    main()
