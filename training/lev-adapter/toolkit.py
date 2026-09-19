#!/usr/bin/env python3
"""Finds Apple's adapter training toolkit and checks it against this device.

The toolkit is not on a package index and is not vendored here: it is
distributed to Apple Developer Program members who agree to its terms, and
this repository is open source. The operator fetches it; this module locates
it and refuses clearly when it is missing.

The check that matters is the base model signature. A toolkit ships assets for
one base model, an adapter is pinned to that base, and the base arrives with
the operating system. Training against the wrong toolkit version produces a
package the device will not load.
"""

import json
import os
import pathlib
import subprocess
import sys

HERE = pathlib.Path(__file__).resolve().parent
REPO = HERE.parents[1]

# Candidate locations, in order. The first is the in-repo ignored directory.
CANDIDATES = [
    HERE / "toolkit",
    pathlib.Path.home() / "code" / "adapter_training_toolkit_v26_0_0",
    pathlib.Path.home() / "Downloads" / "adapter_training_toolkit_v26_0_0",
]

MARKERS = ["export/export_fmadapter.py", "export/constants.py"]


class ToolkitMissing(RuntimeError):
    pass


def find(explicit=None):
    """Returns the toolkit root, or raises with what to do about it."""
    roots = []
    if explicit:
        roots.append(pathlib.Path(explicit))
    if os.environ.get("LEV_TOOLKIT_ROOT"):
        roots.append(pathlib.Path(os.environ["LEV_TOOLKIT_ROOT"]))
    roots.extend(CANDIDATES)

    for root in roots:
        if all((root / marker).exists() for marker in MARKERS):
            return root

    raise ToolkitMissing(
        "Apple's adapter training toolkit was not found.\n\n"
        "It is not on PyPI and is not vendored in this repository: Apple\n"
        "distributes it to Developer Program members who accept its terms,\n"
        "and those terms do not permit redistribution here.\n\n"
        "To get it:\n"
        "  1. Sign in at https://developer.apple.com/apple-intelligence/foundation-models-adapter\n"
        "  2. Accept the toolkit terms and download the version whose base\n"
        "     model signature matches this device (see `signature` below).\n"
        f"  3. Unpack it to {HERE / 'toolkit'} or set LEV_TOOLKIT_ROOT.\n\n"
        "Searched:\n" + "\n".join(f"  {root}" for root in roots)
    )


def device_signature_prefix():
    """Asks the running device which base it will accept an adapter for.

    Returns the signature prefix from `fmadapter-<name>-<prefix>`, which is
    the only way to read the live base model signature from outside the
    framework.
    """
    helper = REPO / "swift/lev-bridge/.build/release/lev-bridge"
    if not helper.exists():
        raise RuntimeError(
            f"no bridge helper at {helper}; run ./scripts/build-lev-bridge.sh"
        )
    request = json.dumps({"id": "1", "op": "adapter_compat", "adapterName": "lev"})
    result = subprocess.run(
        [str(helper)], input=request + "\n", capture_output=True, text=True, check=True
    )
    response = json.loads(result.stdout.strip().splitlines()[0])
    identifiers = response.get("compatibleAdapters") or []
    if not identifiers:
        raise RuntimeError("the device reported no compatible adapter identifiers")
    return identifiers[0].rsplit("-", 1)[-1]


def main():
    try:
        root = find()
        print(f"toolkit: {root}")
    except ToolkitMissing as missing:
        print(missing, file=sys.stderr)
        root = None
    try:
        prefix = device_signature_prefix()
        print(f"device base signature prefix: {prefix}")
        print(f"an adapter for this device is identified fmadapter-<name>-{prefix}")
    except Exception as error:  # noqa: BLE001 - report whatever went wrong
        print(f"device signature unavailable: {error}", file=sys.stderr)
    return 0 if root else 3


if __name__ == "__main__":
    sys.exit(main())
