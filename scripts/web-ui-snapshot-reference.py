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

Usage (one LABEL=PATH per saved page):
  scripts/web-ui-snapshot-reference.py \
      "Conversation=chat.html" "Home, Chat tab=home-chat.html" \
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


class View:
    """One analysed snapshot."""

    def __init__(self, label, path):
        tree = Tree()
        tree.feed(open(path, encoding="utf-8", errors="replace").read())
        self.label = label
        self.css = "".join(tree.styles)
        self.rules = parse_css(self.css)
        self.props = defined_props(self.css)
        self.version = (re.findall(r"tailwindcss v([0-9.]+)", self.css) or ["unknown"])[0]
        self.theme = next((n.data.get("data-theme") for n in tree.root.walk() if n.tag == "html"), None)
        self.nodes = [n for n in tree.root.walk() if n.tag != "#root"]
        self.classes = collections.Counter(c for n in self.nodes for c in n.classes)
        self.utils = collections.Counter(c for n in self.nodes for c in utilities(n.classes))
        self.modules = collections.defaultdict(list)
        for n in self.nodes:
            name = module_name(n.classes)
            if name:
                self.modules[name].append(n)
        self.tokens = collections.Counter()
        for cls, n in self.utils.items():
            for t in describe(cls, self.rules)[1]:
                self.tokens[t] += n
        for ns in self.modules.values():
            for n in ns:
                for c in n.classes:
                    if MODULE.match(c):
                        for t in describe(c, self.rules)[1]:
                            self.tokens[t] += 1


def merged_rules(views):
    rules = collections.defaultdict(list)
    for v in views:
        for k, d in v.rules.items():
            if k not in rules:
                rules[k] = d
    return rules


def token_families(tokens, sdk_props):
    out = []
    fam = collections.defaultdict(list)
    for t, n in tokens.most_common():
        if t.startswith("--tw-"):
            continue  # Tailwind's internal plumbing, not a design token.
        fam["-".join(t[2:].split("-")[:2])].append((t, n))
    for key in sorted(fam, key=lambda k: -sum(n for _, n in fam[k])):
        items = fam[key]
        cells = ", ".join(f"`{t}`{' sdk' if t in sdk_props else ''} ({n})" for t, n in items[:12])
        more = f" … +{len(items) - 12}" if len(items) > 12 else ""
        out.append(f"- **{key}**: {cells}{more}")
    return out


def component_rows(modules, rules):
    rows = ["| Component | Count | Options seen | Co-occurring utilities | Own tokens |",
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
        rows.append(f"| {name} | {len(ns)} | {o} | {c} | {t} |")
    return rows


def parse_view(arg):
    if "=" in arg and not os.path.exists(arg):
        label, path = arg.split("=", 1)
        return label.strip(), path
    return os.path.splitext(os.path.basename(arg))[0], arg


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("snapshots", nargs="+", help="LABEL=PATH (or just PATH) per saved page")
    ap.add_argument("--apps-sdk-ui", default=os.path.expanduser("~/work/projects/repos/apps-sdk-ui"))
    ap.add_argument("--out", default="docs/web/chatgpt-ui-reference.md")
    args = ap.parse_args()

    views = [View(*parse_view(a)) for a in args.snapshots]
    rules = merged_rules(views)
    sdk_props = set()
    for path in glob.glob(os.path.join(args.apps_sdk_ui, "src/styles/variables-*.css")):
        sdk_props |= defined_props(open(path, encoding="utf-8").read())

    all_utils = collections.Counter()
    all_tokens = collections.Counter()
    all_modules = collections.defaultdict(list)
    for v in views:
        all_utils.update(v.utils)
        all_tokens.update(v.tokens)
        for k, ns in v.modules.items():
            all_modules[k].extend(ns)
    page_props = set().union(*(v.props for v in views))
    labels = [v.label for v in views]

    out = [
        "# ChatGPT web UI reference",
        "",
        f"Generated by `scripts/web-ui-snapshot-reference.py` on {datetime.date.today().isoformat()} "
        f"from {len(views)} saved pages of the ChatGPT web app: " + "; ".join(labels) + ".",
        "Regenerate it from new snapshots rather than editing it by hand.",
        "",
        "This is a study of how the live product applies the Apps SDK UI design language: which",
        "utilities, token roles, variants and component shapes it uses on real surfaces. It is",
        "reference material for [the adoption plan](apps-sdk-ui-adoption-plan.md). Our token values",
        "and component CSS are ported from Apps SDK UI (MIT), not from these pages' stylesheets.",
        "",
        "Privacy: only tag names, class names, roles and enum-like `data-*` values were read. No",
        "page text, ids, links, labels or styles are included. The snapshots are not committed.",
        "",
        "## Views",
        "",
        "| View | Theme | Styled elements | Utilities | Components | Tailwind | Apps SDK UI tokens present |",
        "| --- | --- | --- | --- | --- | --- | --- |",
    ]
    for v in views:
        out.append(f"| {v.label} | {v.theme or '?'} | {sum(1 for n in v.nodes if n.classes)} | {len(v.utils)} | "
                   f"{len(v.modules)} | v{v.version} | {len(sdk_props & v.props)} of {len(sdk_props)} |")
    out += ["",
            f"Across all views: {len(all_utils)} distinct utilities, {len(all_modules)} components, "
            f"{len(page_props):,} custom properties ({len(page_props - sdk_props):,} not in Apps SDK UI).", ""]

    # Shared shell vs view-specific.
    shared_mods = set.intersection(*(set(v.modules) for v in views)) if views else set()
    shared_utils = set.intersection(*(set(v.utils) for v in views)) if views else set()
    out += ["## Shared shell and view-specific parts", "",
            f"Components on every view ({len(shared_mods)}): " + (", ".join(sorted(shared_mods)) or "none") + ".", "",
            f"Utilities on every view: {len(shared_utils)}.", ""]
    out += ["| View | Components only here | Utilities only here |", "| --- | --- | --- |"]
    for v in views:
        others_m = set().union(*(set(o.modules) for o in views if o is not v)) if len(views) > 1 else set()
        others_u = set().union(*(set(o.utils) for o in views if o is not v)) if len(views) > 1 else set()
        only_m = sorted(set(v.modules) - others_m)
        only_u = set(v.utils) - others_u
        out.append(f"| {v.label} | {', '.join(only_m) or '—'} | {len(only_u)} |")
    out.append("")

    out += ["## Token roles in use", "",
            "Tokens referenced by utilities and component classes across all views, by family.",
            "`sdk` marks a token defined by Apps SDK UI; others are product-only.", ""]
    out += token_families(all_tokens, sdk_props) + [""]

    for v in views:
        out += [f"## View: {v.label}", ""]
        for name in AREA_MODULES:
            if name in v.modules:
                out += area_section(f"{name} (component)", v.modules[name], v.rules, 20)
        for tag in LANDMARKS:
            roots = [n for n in v.nodes if n.tag == tag]
            if roots:
                out += area_section(f"`<{tag}>`", roots, v.rules, 15)
        view_only = sorted(set(v.modules) - set().union(*(set(o.modules) for o in views if o is not v)) if len(views) > 1 else set(v.modules))
        if view_only:
            out += ["### Components specific to this view", ""]
            out += component_rows({k: v.modules[k] for k in view_only}, v.rules) + [""]

    out += ["## Components (all views)", "",
            "Hashed CSS-module classes, with the `data-*` options seen and the utilities that most",
            "often sit on the same element.", ""]
    out += component_rows(all_modules, rules) + [""]

    variants = collections.Counter()
    for c, n in all_utils.items():
        if ":" in c and not c.startswith("["):
            variants[c.rsplit(":", 1)[0]] += n
    out += ["## Variants", "", "State, responsive and platform variants on utilities, across all views.", "",
            "| Variant | Uses |", "| --- | --- |"]
    out += [f"| {fmt_class(v)} | {n} |" for v, n in variants.most_common(40)]
    out.append("")

    out += ["## Utility index", "",
            "Every token-backed utility across all views with its compiled properties and tokens.", "",
            "| Utility | Uses | Views | CSS properties | Tokens |", "| --- | --- | --- | --- | --- |"]
    for cls, n in sorted(all_utils.items(), key=lambda kv: (-kv[1], kv[0])):
        props, toks = describe(cls, rules)
        if toks:
            seen = sum(1 for v in views if cls in v.utils)
            out.append(f"| {fmt_class(cls)} | {n} | {seen}/{len(views)} | {', '.join(props[:3]) or '—'} | "
                       f"{', '.join('`'+t+'`' for t in toks[:3])} |")
    out.append("")

    os.makedirs(os.path.dirname(args.out) or ".", exist_ok=True)
    with open(args.out, "w", encoding="utf-8") as fh:
        fh.write("\n".join(out))
    print(f"wrote {args.out}: {len(out)} lines, {len(views)} views, {len(all_utils)} utilities, "
          f"{len(all_modules)} components", file=sys.stderr)


if __name__ == "__main__":
    main()
