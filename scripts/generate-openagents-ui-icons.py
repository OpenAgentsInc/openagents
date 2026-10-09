#!/usr/bin/env python3
"""Generate crates/openagents-ui/src/icons/generated.rs from Apps SDK UI.

Reads the MIT-licensed icon sources in
`apps-sdk-ui/src/components/Icon/svg/*.tsx` (one React component per icon)
and writes a Rust `enum Icon` with one variant per icon, plus the static SVG
parts the hand-written `icons/mod.rs` renders.

The output is deterministic: the same sources always produce the same file.

Usage:
    scripts/generate-openagents-ui-icons.py [--source DIR] [--check]

`--source` defaults to $APPS_SDK_UI_DIR or ~/work/projects/repos/apps-sdk-ui.
`--check` exits non-zero when the checked-in file differs from a fresh run.
"""

from __future__ import annotations

import argparse
import os
import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
OUTPUT = REPO_ROOT / "crates/openagents-ui/src/icons/generated.rs"

# JSX attribute names that differ from their SVG spelling. Any other
# camelCase attribute is an error so a new upstream attribute is noticed.
ATTR_NAMES = {
    "fillRule": "fill-rule",
    "clipRule": "clip-rule",
    "clipPath": "clip-path",
    "strokeWidth": "stroke-width",
    "strokeLinecap": "stroke-linecap",
    "strokeLinejoin": "stroke-linejoin",
}
KEEP_CAMEL = {"viewBox"}
# Root attributes the Rust renderer emits itself.
ROOT_DROPPED = {"className", "width", "height"}

TAG_RE = re.compile(r"<(/?)([A-Za-z][A-Za-z0-9]*)((?:[^>\"{]|\"[^\"]*\"|\{[^}]*\})*?)\s*(/?)>")
ATTR_RE = re.compile(r"\{\.\.\.props\}|([A-Za-z][A-Za-z0-9:-]*)=(\"[^\"]*\"|\{[^}]*\})")


def convert_attrs(icon: str, raw: str, root: bool) -> list[tuple[str, str]]:
    attrs: list[tuple[str, str]] = []
    pos = 0
    for match in ATTR_RE.finditer(raw):
        if raw[pos : match.start()].strip():
            raise SystemExit(f"{icon}: unparsed attribute text {raw[pos:match.start()]!r}")
        pos = match.end()
        name, value = match.group(1), match.group(2)
        if name is None:
            continue  # {...props}
        if value.startswith("{"):
            inner = value[1:-1].strip()
            if not re.fullmatch(r"-?[0-9.]+", inner):
                raise SystemExit(f"{icon}: unsupported JSX expression {value!r}")
            value = inner
        else:
            value = value[1:-1]
        if root and name in ROOT_DROPPED:
            continue
        if name == "id":
            value = f"oa-icon-{icon}-{value}"
        value = re.sub(r"url\(#([^)]+)\)", lambda m: f"url(#oa-icon-{icon}-{m.group(1)})", value)
        if name in ATTR_NAMES:
            name = ATTR_NAMES[name]
        elif name not in KEEP_CAMEL and re.search(r"[A-Z]", name):
            raise SystemExit(f"{icon}: unmapped JSX attribute {name!r}")
        if '"' in value or "<" in value or "&" in value:
            raise SystemExit(f"{icon}: attribute value needs escaping: {value!r}")
        attrs.append((name, value))
    if raw[pos:].strip():
        raise SystemExit(f"{icon}: unparsed attribute text {raw[pos:]!r}")
    return attrs


def fmt_attrs(attrs: list[tuple[str, str]]) -> str:
    return "".join(f' {name}="{value}"' for name, value in attrs)


def convert_icon(icon: str, source: str) -> tuple[str, str]:
    """Return (root attributes, inner SVG markup) for one icon component."""
    start = source.index("<svg")
    end = source.rindex("</svg>") + len("</svg>")
    jsx = source[start:end]
    root_attrs = None
    body: list[str] = []
    depth = 0
    pos = 0
    for match in TAG_RE.finditer(jsx):
        if jsx[pos : match.start()].strip():
            raise SystemExit(f"{icon}: unexpected text {jsx[pos:match.start()]!r}")
        pos = match.end()
        closing, tag, raw, self_closing = match.groups()
        if closing:
            depth -= 1
            if depth > 0:
                body.append(f"</{tag}>")
            continue
        attrs = convert_attrs(icon, raw, root=depth == 0)
        if depth == 0:
            if tag != "svg" or root_attrs is not None:
                raise SystemExit(f"{icon}: expected a single <svg> root")
            root_attrs = fmt_attrs(attrs)
        else:
            body.append(f"<{tag}{fmt_attrs(attrs)}{' />' if self_closing else '>'}")
        if not self_closing:
            depth += 1
    if depth != 0 or root_attrs is None or jsx[pos:].strip():
        raise SystemExit(f"{icon}: unbalanced SVG markup")
    return root_attrs, "".join(body)


def rust_str(value: str) -> str:
    hashes = "#"
    while f'"{hashes}' in value:
        hashes += "#"
    return f'r{hashes}"{value}"{hashes}'


def generate(source_dir: Path) -> str:
    svg_dir = source_dir / "src/components/Icon/svg"
    files = sorted(svg_dir.glob("*.tsx"), key=lambda p: p.stem)
    if not files:
        raise SystemExit(f"no icon sources under {svg_dir}")
    icons = []
    for path in files:
        name = path.stem
        if not re.fullmatch(r"[A-Z][A-Za-z0-9]*", name):
            raise SystemExit(f"{name}: not a CamelCase identifier")
        root, body = convert_icon(name, path.read_text(encoding="utf-8"))
        icons.append((name, root, body))

    out: list[str] = []
    w = out.append
    w("// @generated by scripts/generate-openagents-ui-icons.py. Do not edit by hand.")
    w("// Source: apps-sdk-ui/src/components/Icon/svg/*.tsx (MIT, see crates/openagents-ui/NOTICE).")
    w("")
    w("/// One Apps SDK UI icon. Variant names match the upstream component names.")
    w("#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]")
    w("pub enum Icon {")
    for name, _, _ in icons:
        w(f"    {name},")
    w("}")
    w("")
    w("impl Icon {")
    w(f"    /// Every icon, in name order, for the catalog.")
    w(f"    pub const ALL: [Icon; {len(icons)}] = [")
    for name, _, _ in icons:
        w(f"        Icon::{name},")
    w("    ];")
    w("")
    w("    /// The upstream component name, for example `\"AddMember\"`.")
    w("    pub const fn name(self) -> &'static str {")
    w("        match self {")
    for name, _, _ in icons:
        w(f'            Icon::{name} => "{name}",')
    w("        }")
    w("    }")
    w("")
    w("    /// Root `<svg>` attributes (viewBox, fill, stroke, ...) and inner markup.")
    w("    pub(super) const fn parts(self) -> (&'static str, &'static str) {")
    w("        match self {")
    for name, root, body in icons:
        w(f"            Icon::{name} => (")
        w(f"                {rust_str(root)},")
        w(f"                {rust_str(body)},")
        w("            ),")
    w("        }")
    w("    }")
    w("}")
    return "\n".join(out) + "\n"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    default_source = os.environ.get(
        "APPS_SDK_UI_DIR", str(Path.home() / "work/projects/repos/apps-sdk-ui")
    )
    parser.add_argument("--source", default=default_source)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    text = generate(Path(args.source).expanduser())
    if args.check:
        current = OUTPUT.read_text(encoding="utf-8") if OUTPUT.exists() else ""
        if current != text:
            print(f"{OUTPUT.relative_to(REPO_ROOT)} is stale; rerun the generator", file=sys.stderr)
            return 1
        print(f"{OUTPUT.relative_to(REPO_ROOT)} is up to date")
        return 0
    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    OUTPUT.write_text(text, encoding="utf-8")
    print(f"wrote {OUTPUT.relative_to(REPO_ROOT)} ({text.count(chr(10))} lines)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
