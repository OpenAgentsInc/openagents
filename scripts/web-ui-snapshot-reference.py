#!/usr/bin/env python3
"""Turn a saved ChatGPT web page into a Markdown design reference.

Reads a single-file HTML snapshot (for example one saved with SingleFile),
maps every Tailwind utility on its elements to the compiled CSS rule and the
design tokens it uses, groups usage by page area and CSS-module component,
and compares the tokens with Apps SDK UI's published token files.

Privacy: the script reads only tag names, class names, `role`, and a fixed
set of enum-like `data-*` attributes. It never reads text, ids, links, image
sources, labels, inline styles, or any other attribute, and it drops class
names that contain URLs or long hex/uuid-like strings. Do not commit the
snapshot itself; it carries account identifiers in its <html> attributes.

The output documents structure and usage for reference. Token values and
component CSS for our own implementation come from Apps SDK UI (MIT), not
from the snapshot's stylesheet.

Usage:
  scripts/web-ui-snapshot-reference.py SNAPSHOT.html \
      --apps-sdk-ui ~/work/projects/repos/apps-sdk-ui \
      --out docs/web/chatgpt-ui-reference.md
"""

import argparse
import collections
import datetime
import glob
import os
import re
import sys
from html.parser import HTMLParser

KEPT_DATA = {
    "data-color", "data-variant", "data-size", "data-state", "data-pill",
    "data-icon-size", "data-col-size", "data-uniform", "data-theme",
}
MODULE = re.compile(r"^([A-Z][A-Za-z0-9]+)-[A-Za-z0-9_]{5,8}$")
UNSAFE = re.compile(r"url\(|https?:|//|[0-9a-f]{8}-[0-9a-f]{4}|[0-9a-f]{16,}", re.I)
LANDMARKS = ["header", "nav", "aside", "main", "form", "table", "pre"]
AREA_MODULES = [
    "Layout", "ConversationSidebar", "Navigation", "LeftPanel",
    "MainContentSurface", "Workspace", "ViewerContent",
]


class Node:
    __slots__ = ("tag", "classes", "role", "data", "children", "parent")

    def __init__(self, tag, classes, role, data, parent):
        self.tag, self.classes, self.role, self.data = tag, classes, role, data
        self.children, self.parent = [], parent

    def walk(self):
        yield self
        for child in self.children:
            yield from child.walk()


VOID = {"area", "base", "br", "col", "embed", "hr", "img", "input", "link",
        "meta", "source", "track", "wbr", "path", "circle", "rect", "line",
        "polyline", "polygon", "ellipse", "stop", "use"}


class Tree(HTMLParser):
    def __init__(self):
        super().__init__(convert_charrefs=True)
        self.root = Node("#root", [], None, {}, None)
        self.cur = self.root
        self.styles = []
        self._in_style = False

    def handle_starttag(self, tag, attrs):
        a = dict(attrs)
        if tag == "style":
            self._in_style = True
            return
        classes = [c for c in (a.get("class") or "").split() if not UNSAFE.search(c)]
        data = {k: v for k, v in a.items() if k in KEPT_DATA and v and len(v) <= 40}
        node = Node(tag, classes, a.get("role"), data, self.cur)
        self.cur.children.append(node)
        if tag not in VOID:
            self.cur = node

    def handle_startendtag(self, tag, attrs):
        self.handle_starttag(tag, attrs)
        if tag not in VOID and self.cur.parent is not None:
            self.cur = self.cur.parent

    def handle_endtag(self, tag):
        if tag == "style":
            self._in_style = False
            return
        node = self.cur
        while node is not None and node.tag != tag:
            node = node.parent
        if node is not None and node.parent is not None:
            self.cur = node.parent

    def handle_data(self, data):
        if self._in_style:
            self.styles.append(data)


def unescape_css(sel):
    sel = re.sub(r"\\([0-9a-fA-F]{1,6})\s?", lambda m: chr(int(m.group(1), 16)), sel)
    return re.sub(r"\\(.)", r"\1", sel)


def parse_css(css):
    """Map unescaped class name -> list of declaration strings."""
    rules = collections.defaultdict(list)

    def block(text, i):
        sel_start = i
        while i < len(text):
            ch = text[i]
            if ch == "{":
                selector = text[sel_start:i].strip()
                depth, j = 1, i + 1
                while j < len(text) and depth:
                    if text[j] == "{":
                        depth += 1
                    elif text[j] == "}":
                        depth -= 1
                    j += 1
                body = text[i + 1:j - 1]
                if selector.startswith("@"):
                    block(body, 0)
                else:
                    top = re.sub(r"\{[^{}]*\}", "", body)
                    decls = [d.strip() for d in top.split(";") if ":" in d]
                    for part in selector.split(","):
                        m = re.match(r"\s*\.((?:\\.|[^\s.:#\[>+~,()]|\[(?:\\.|[^\]])*\])+)", part)
                        if m and decls:
                            rules[unescape_css(m.group(1))].extend(decls)
                    if "{" in body:
                        block(body, 0)
                i, sel_start = j, j
                continue
            if ch in ";}":
                sel_start = i + 1
            i += 1

    block(css, 0)
    return rules


def tokens_in(decls):
    return sorted({t for d in decls for t in re.findall(r"var\((--[A-Za-z0-9-]+)", d)})


def defined_props(css):
    return set(re.findall(r"(--[A-Za-z0-9-]+)\s*:", css))


def module_name(classes):
    for c in classes:
        m = MODULE.match(c)
        if m:
            return m.group(1)
    return None


def utilities(classes):
    return [c for c in classes if not MODULE.match(c)]


def describe(cls, rules):
    decls = rules.get(cls, [])
    toks = tokens_in(decls)
    props = sorted({d.split(":", 1)[0].strip() for d in decls if not d.strip().startswith("--")})
    return props, toks


def fmt_class(cls):
    return "`" + cls.replace("|", "\\|").replace("`", "'") + "`"


def area_section(name, nodes, rules, limit=30):
    counts = collections.Counter()
    elements = 0
    for root in nodes:
        for n in root.walk():
            if n.classes:
                elements += 1
                counts.update(utilities(n.classes))
    if not counts:
        return []
    out = [f"### {name}", "",
           f"{len(nodes)} instance(s), {elements} styled elements, {len(counts)} distinct utilities.", "",
           "| Utility | Uses | CSS properties | Tokens |", "| --- | --- | --- | --- |"]
    for cls, n in counts.most_common(limit):
        props, toks = describe(cls, rules)
        out.append(f"| {fmt_class(cls)} | {n} | {', '.join(props[:3]) or '—'} | {', '.join('`'+t+'`' for t in toks[:3]) or '—'} |")
    return out + [""]


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("snapshot")
    ap.add_argument("--apps-sdk-ui", default=os.path.expanduser("~/work/projects/repos/apps-sdk-ui"))
    ap.add_argument("--out", default="docs/web/chatgpt-ui-reference.md")
    ap.add_argument("--label", default="ChatGPT web app, Codex view")
    args = ap.parse_args()

    tree = Tree()
    tree.feed(open(args.snapshot, encoding="utf-8", errors="replace").read())
    css = "".join(tree.styles)
    rules = parse_css(css)
    page_props = defined_props(css)
    version = (re.findall(r"tailwindcss v([0-9.]+)", css) or ["unknown"])[0]
    theme = next((n.data.get("data-theme") for n in tree.root.walk() if n.tag == "html"), None)

    sdk_props = set()
    for path in glob.glob(os.path.join(args.apps_sdk_ui, "src/styles/variables-*.css")):
        sdk_props |= defined_props(open(path, encoding="utf-8").read())

    nodes = [n for n in tree.root.walk() if n.tag != "#root"]
    all_classes = collections.Counter(c for n in nodes for c in n.classes)
    utils = collections.Counter(c for n in nodes for c in utilities(n.classes))
    modules = collections.defaultdict(list)
    for n in nodes:
        name = module_name(n.classes)
        if name:
            modules[name].append(n)

    used_tokens = collections.Counter()
    for cls, n in utils.items():
        for t in describe(cls, rules)[1]:
            used_tokens[t] += n
    for name, ns in modules.items():
        for n in ns:
            for c in n.classes:
                if MODULE.match(c):
                    for t in describe(c, rules)[1]:
                        used_tokens[t] += 1

    out = [
        "# ChatGPT web UI reference",
        "",
        f"Generated by `scripts/web-ui-snapshot-reference.py` on {datetime.date.today().isoformat()} "
        f"from a saved snapshot of the {args.label} ({theme or 'unknown'} theme).",
        "Regenerate it from a new snapshot rather than editing it by hand.",
        "",
        "This is a study of how the live product applies the Apps SDK UI design language: which",
        "utilities, token roles, variants and component shapes it uses on real surfaces. It is",
        "reference material for [the adoption plan](apps-sdk-ui-adoption-plan.md). Our token values",
        "and component CSS are ported from Apps SDK UI (MIT), not from this page's stylesheet.",
        "",
        "Privacy: only tag names, class names, roles and enum-like `data-*` values were read. No",
        "page text, ids, links, labels or styles are included. The snapshot is not committed.",
        "",
        "## Summary",
        "",
        f"- Elements with classes: {sum(1 for n in nodes if n.classes)}; distinct classes: {len(all_classes)}.",
        f"- Tailwind utilities: {len(utils)} distinct; CSS-module components: {len(modules)} distinct.",
        f"- Compiled CSS: {len(css):,} bytes, Tailwind v{version}; {len(page_props):,} custom properties defined.",
        f"- Apps SDK UI tokens: {len(sdk_props)} defined upstream, {len(sdk_props & page_props)} present on the page; "
        f"{len(page_props - sdk_props):,} page-only properties.",
        f"- Utilities with a compiled rule found: {sum(1 for c in utils if c in rules)} of {len(utils)}.",
        "",
    ]

    # Token roles actually used, grouped by family.
    out += ["## Token roles in use", "",
            "Tokens referenced by the utilities and component classes on this page, by family.",
            "`sdk` marks a token defined by Apps SDK UI; others are product-only.", ""]
    fam = collections.defaultdict(list)
    for t, n in used_tokens.most_common():
        if t.startswith("--tw-"):
            continue  # Tailwind's internal plumbing, not a design token.
        key = "-".join(t[2:].split("-")[:2])
        fam[key].append((t, n))
    for key in sorted(fam, key=lambda k: -sum(n for _, n in fam[k])):
        items = fam[key]
        cells = ", ".join(f"`{t}`{' sdk' if t in sdk_props else ''} ({n})" for t, n in items[:12])
        more = f" … +{len(items) - 12}" if len(items) > 12 else ""
        out.append(f"- **{key}**: {cells}{more}")
    out.append("")

    # Areas.
    out += ["## Page areas", "",
            "Utility usage inside each structural area, most used first.", ""]
    for name in AREA_MODULES:
        if name in modules:
            out += area_section(f"{name} (component)", modules[name], rules, 25)
    for tag in LANDMARKS:
        roots = [n for n in nodes if n.tag == tag]
        if roots:
            out += area_section(f"`<{tag}>`", roots, rules, 20)

    # Components.
    out += ["## Components (CSS modules)", "",
            "Hashed CSS-module classes, with the `data-*` options seen and the utilities that most",
            "often sit on the same element.", "",
            "| Component | Count | Options seen | Co-occurring utilities | Own tokens |",
            "| --- | --- | --- | --- | --- |"]
    for name, ns in sorted(modules.items(), key=lambda kv: -len(kv[1])):
        opts = collections.defaultdict(set)
        co = collections.Counter()
        own = set()
        for n in ns:
            for k, v in n.data.items():
                opts[k.removeprefix("data-")].add(v)
            co.update(utilities(n.classes))
            for c in n.classes:
                if MODULE.match(c):
                    own.update(describe(c, rules)[1])
        o = "; ".join(f"{k}={'/'.join(sorted(v))}" for k, v in sorted(opts.items())) or "—"
        c = ", ".join(fmt_class(x) for x, _ in co.most_common(6)) or "—"
        t = ", ".join(f"`{x}`" for x in sorted(own)[:5]) or "—"
        out.append(f"| {name} | {len(ns)} | {o} | {c} | {t} |")
    out.append("")

    # Variants.
    variants = collections.Counter()
    for c, n in utils.items():
        if ":" in c and not c.startswith("["):
            variants[c.rsplit(":", 1)[0]] += n
    out += ["## Variants", "", "State, responsive and platform variants on utilities.", "",
            "| Variant | Uses |", "| --- | --- |"]
    out += [f"| {fmt_class(v)} | {n} |" for v, n in variants.most_common(40)]
    out.append("")

    # Full utility index.
    out += ["## Utility index", "",
            "Every token-backed utility on the page with its compiled properties and tokens.", "",
            "| Utility | Uses | CSS properties | Tokens |", "| --- | --- | --- | --- |"]
    for cls, n in sorted(utils.items(), key=lambda kv: (-kv[1], kv[0])):
        props, toks = describe(cls, rules)
        if toks:
            out.append(f"| {fmt_class(cls)} | {n} | {', '.join(props[:3]) or '—'} | {', '.join('`'+t+'`' for t in toks[:3])} |")
    out.append("")

    os.makedirs(os.path.dirname(args.out) or ".", exist_ok=True)
    with open(args.out, "w", encoding="utf-8") as fh:
        fh.write("\n".join(out))
    print(f"wrote {args.out}: {len(out)} lines, {len(utils)} utilities, {len(modules)} components",
          file=sys.stderr)


if __name__ == "__main__":
    main()
