#!/usr/bin/env python3
"""Move the physics-engine and spacecraft parts from the Psionic book into the
Aethyr book, renumber what stays, and rewrite every cross-reference.

The mapping is data in move-from-psionic.tsv. Run from any directory:

    aethyr/book/tools/move-from-psionic.py --dry-run   # report, change nothing
    aethyr/book/tools/move-from-psionic.py             # move, rewrite, refresh claims

It is written to be run once, on a fresh origin/main, by the commit that
records the move. Rerunning it after the move finds nothing to move and
refuses.
"""
import argparse
import hashlib
import json
import os
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
TOOLS = Path(__file__).resolve().parent
NAME = {"psionic": "Psionic", "aethyr": "Aethyr"}
BOOKDIR = {"psionic": "psionic/book", "aethyr": "aethyr/book"}


def load_table():
    ranges, parts, literals, keeps = [], {}, [], []
    for line in (TOOLS / "move-from-psionic.tsv").read_text().splitlines():
        if not line.strip() or line.startswith("#"):
            continue
        kind, *cols = line.split("\t")
        if kind == "chapters":
            first, last, book, new_first = cols
            ranges.append((int(first), int(last), book, int(new_first)))
        elif kind == "part":
            old, book, new = cols
            parts[old] = (book, new)
        elif kind == "dropline":
            scope, prefix = cols
            literals.append((scope, None, prefix))
        elif kind == "keep":
            scope, text = cols
            keeps.append((scope, text))
        elif kind == "literal":
            scope, old, new = cols
            literals.append((scope, old.replace("\\n", "\n"), new.replace("\\n", "\n")))
        else:
            raise SystemExit(f"unknown table row: {line}")
    return ranges, parts, literals, keeps


RANGES, PARTS, LITERALS, KEEPS = load_table()
LAST = max(r[1] for r in RANGES)


def mapped(n):
    for first, last, book, new_first in RANGES:
        if first <= n <= last:
            return book, new_first + n - first
    return None


# ---------------------------------------------------------------- files

def chapter_files():
    """Old repository path -> new repository path for every chapter file."""
    moves = {}
    for p in sorted((ROOT / "psionic/book").glob("ch[0-9][0-9][0-9]-*.md")):
        n = int(p.name[2:5])
        book, m = mapped(n)
        new = f"{BOOKDIR[book]}/ch{m:03d}-{p.name[6:]}"
        old = f"psionic/book/{p.name}"
        if new != old:
            moves[old] = new
    return moves


def home(path):
    if path.startswith("aethyr/"):
        return "aethyr"
    return "psionic"


def scoped_files():
    """(old path, rule set) for every file the move rewrites."""
    listed = subprocess.run(["git", "ls-files"], cwd=ROOT, capture_output=True, text=True, check=True).stdout.split("\n")
    out = []
    for f in listed:
        if not f or not (ROOT / f).is_file():
            continue
        if f.startswith("psionic/book/") and f.endswith(".md"):
            out.append((f, "prose"))
        elif f == "psionic/README.md" or (f.startswith("psionic/docs/") and f.endswith(".md")):
            out.append((f, "prose"))
        elif f == "psionic/book/examples.json" or f == "psionic/check.sh":
            out.append((f, "paths"))
        elif f.startswith("docs/verification/") and f.endswith(".md"):
            out.append((f, "links"))
        elif f.startswith("docs/transcripts/"):
            continue
        elif f.endswith(".md") and (f.startswith(("docs/", "coax/", "verse/")) or f in ("README.md", "AGENTS.md", "INVARIANTS.md", "NEEDS_OWNER.md")):
            out.append((f, "cited"))
        elif re.fullmatch(r"psionic/(examples|lib)/.+\.coax", f) or re.fullmatch(r"psionic/tests/[^/]+\.(py|sh|c)", f):
            out.append((f, "code"))
        elif re.fullmatch(r"psionic/fixtures/[^/]+\.out", f):
            out.append((f, "code"))
    return out


# ---------------------------------------------------------------- rewriting

LINK = re.compile(r"\[((?:[^\[\]]|\[[^\[\]]*\])*)\]\(([^)\s]+)\)")
ROOTPATH = re.compile(r"(?<![/.\w])psionic/book/ch(\d{3})-([a-z0-9-]+)\.md")
NUM = r"\d{1,3}"
ITEM = rf"{NUM}(?:\s*[–-]\s*{NUM})?"
SEP = r"(?:,\s+and\s+|,\s+or\s+|,\s+|\s+and\s+|\s+or\s+|\s+to\s+|/)"
END = r"(?![\d]|[.,]\d|[A-Za-z%°-]|\.\d)"
UNITS = r"(?!\s*(?:ms|s|GiB|GB|MiB|MB|KiB|bytes?|tokens?|rpm|km|m|K|W|GW|Hz|fps|FPS|x|×|%|px|steps?|cases?|rows?|values?|runs?|frames?|layers?|experts?|samples?|mutants?|survivors?|traces?|nodes?|items?|tasks?|seeds?|policies|environments?|worlds?|bodies|particles?|points?|questions?|programs?|lines?|checks?|tests?|weights?|parameters?)\b)"
KEYWORD = re.compile(rf"(?P<pre>(?:Psionic|Aethyr)\s+)?(?P<kw>\b[Cc]hapters?\s+|\b[Cc]h\.\s*)(?P<list>{ITEM}{END}(?:{SEP}{ITEM}{END}{UNITS})*)")
PART = re.compile(r"(?P<pre>(?:Psionic|Aethyr)(?:'s|’s)?\s+)?(?P<kw>\b[Pp]arts?\s+)(?P<num>X{0,3}(?:IX|IV|V?I{0,3}))\b(?!\.)")
BARE = re.compile(rf"(?<![\w.,#/\\$@+×*^~-])(?P<item>{ITEM}){END}{UNITS}")
POSSESSIVE = re.compile(rf"(?<![\w.,#/\\$@+-])(?P<num>{NUM})(?P<s>'s|’s)")
# Bare chapter numbers ("then 179, assets 183") are rewritten only in the
# plan, contents, indexes and the moved chapters; elsewhere a bare number is
# usually a count, and other docs keep their own (older) numbering.
BARE_FILES = r"psionic/book/(PLAN|README|SUMMARY)\.md|psionic/README\.md|psionic/docs/(genesis-parity|roadmap)\.md|psionic/book/ch1(6[5-9]|7\d|8[0-5])-.*\.md"
FENCE = re.compile(r"^```[^\n]*\n.*?^```[ \t]*$", re.M | re.S)
INLINE = re.compile(r"`[^`\n]+`")


class Rewriter:
    def __init__(self, path, new_path, rules, moves, report):
        self.path, self.new_path, self.rules, self.moves, self.report = path, new_path, rules, moves, report
        self.home = home(new_path)
        self.default = self.home if rules == "prose" or self.home == "aethyr" else "psionic"
        self.slots = []

    def keep(self, text):
        self.slots.append(text)
        # No digits inside a placeholder, so no number rule can see one.
        return "\U0010fffd" + chr(0xF0000 + len(self.slots) - 1) + "\U0010fffe"

    def restore(self, text):
        for _ in range(64):
            if "\U0010fffd" not in text:
                return text
            text = re.sub("\U0010fffd(.)\U0010fffe", lambda m: self.slots[ord(m.group(1)) - 0xF0000], text)
        raise SystemExit(f"{self.path}: unresolved placeholder")

    def note(self, kind, old, new):
        if old != new:
            self.report.append((self.path, kind, old, new))

    # -- numbers
    def item(self, text):
        """'179' or '166–170' -> (book, rendered) or None to leave it."""
        m = re.fullmatch(rf"({NUM})(\s*[–-]\s*)({NUM})", text)
        if m:
            a, dash, b = int(m.group(1)), m.group(2), int(m.group(3))
            if not (1 <= a <= LAST and 1 <= b <= LAST and a < b):
                return None
            ma, mb = mapped(a), mapped(b)
            if ma[0] != mb[0] or mb[1] - ma[1] != b - a:
                self.report.append((self.path, "SPLIT-RANGE", text, "left unchanged; fix by hand"))
                return None
            return ma[0], f"{ma[1]}{dash}{mb[1]}"
        n = int(text)
        if not 1 <= n <= LAST:
            return None
        book, m = mapped(n)
        return book, str(m)

    def render(self, items, seps, pre, kw):
        """Items are rendered (book, text); returns the replacement string."""
        books = {b for b, _ in items}
        context = self.default
        if pre:
            context = pre.strip().lower()
        if len(books) == 1:
            book = books.pop()
            body = "".join(t + (seps[i] if i < len(seps) else "") for i, (_, t) in enumerate(items))
            if book == self.default and not pre:
                return f"{kw}{body}"
            space = re.search(r"\s+$", pre).group(0) if pre else " "
            if book == context and pre:
                return f"{pre}{kw}{body}"
            return f"{NAME[book]}{space}{kw.lower() if kw[0] == 'C' and not pre else kw}{body}"
        # Name the book at each change, so "Aethyr 31, 33 and Psionic 167".
        parts, previous = [], self.default
        for i, (book, t) in enumerate(items):
            parts.append((t if book == previous else f"{NAME[book]} {t}") + (seps[i] if i < len(seps) else ""))
            previous = book
        return f"{pre or ''}{kw}{''.join(parts)}"

    def keyword(self, m):
        pre, kw, lst = m.group("pre"), m.group("kw"), m.group("list")
        tokens = re.split(f"({SEP})", lst)
        raw, seps = tokens[0::2], tokens[1::2]
        items = []
        for t in raw:
            r = self.item(t.strip())
            if r is None:
                return m.group(0)
            if r[0] == "psionic" and int(re.match(NUM, t.strip()).group(0)) < 165 and self.default == "psionic" and not pre:
                pass
            items.append(r)
        out = self.render(items, seps, pre, kw)
        self.note("keyword", m.group(0), out)
        return self.keep(out)

    def part(self, m):
        pre, kw, num = m.group("pre"), m.group("kw"), m.group("num")
        if num not in PARTS:
            return m.group(0)
        if pre and not pre.startswith("Psionic"):
            return m.group(0)
        if self.rules != "prose" and not pre:
            return m.group(0)
        book, new = PARTS[num]
        if book == self.default and not pre:
            out = f"{kw}{new}"
        else:
            out = f"{NAME[book]} {kw}{new}"
        self.note("part", m.group(0), out)
        return self.keep(out)

    def bare(self, m):
        text = m.group("item")
        lo = int(re.match(NUM, text).group(0))
        if lo < 165:
            return m.group(0)
        r = self.item(text)
        if r is None:
            return m.group(0)
        book, t = r
        out = t if book == self.default else f"{NAME[book]} {t}"
        # "165–185, 192" -> "Aethyr 17–37, Psionic 171": name the book again
        # when a list returns to this book's own chapters.
        last = getattr(self, "last_bare", None)
        if book == self.default and last and last[1] != book and last[2] is m.string \
                and re.fullmatch(r",\s+|,?\s+and\s+|,?\s+or\s+", m.string[last[0]:m.start()]):
            out = f"{NAME[book]} {t}"
        self.last_bare = (m.end(), book, m.string)
        self.note("bare", self.context(m), out)
        return self.keep(out)

    def possessive(self, m):
        n = int(m.group("num"))
        if not 1 <= n <= LAST or (n < 165 and self.default == "psionic"):
            return m.group(0)
        book, k = mapped(n)
        out = (str(k) if book == self.default else f"{NAME[book]} {k}") + m.group("s")
        self.note("possessive", self.context(m), out)
        return self.keep(out)

    def context(self, m):
        s = m.string
        return s[max(0, m.start() - 30):m.end() + 20].replace("\n", " ")

    # -- links
    def href(self, target):
        if re.match(r"[a-z]+:", target) or target.startswith("#"):
            return target, None
        path, _, frag = target.partition("#")
        if not path:
            return target, None
        old_abs = os.path.normpath(os.path.join(os.path.dirname(self.path), path))
        new_abs = self.moves.get(old_abs, old_abs)
        if new_abs == old_abs and self.new_path == self.path:
            return target, None
        rel = os.path.relpath(new_abs, os.path.dirname(self.new_path))
        if path.endswith("/") and not rel.endswith("/"):
            rel += "/"
        chapter = None
        m = re.fullmatch(r"(psionic|aethyr)/book/ch(\d{3})-[a-z0-9-]+\.md", new_abs)
        if m and old_abs.startswith("psionic/book/ch"):
            chapter = (m.group(1), int(m.group(2)), int(old_abs[len("psionic/book/ch"):][:3]))
        return rel + (f"#{frag}" if frag else ""), chapter

    def link(self, m):
        label, target = m.group(1), m.group(2)
        new_target, chapter = self.href(target)
        new_label = label
        if chapter:
            book, new_n, old_n = chapter
            # A label that names the chapter by number carries the new number.
            new_label = re.sub(rf"^(\D*?)\b{old_n}\b", lambda k: k.group(1) + str(new_n), label, count=1)
            if book != self.default and re.fullmatch(r"(Chapter|chapter)\s+\d+", new_label):
                new_label = f"{NAME[book]} {new_label.lower() if new_label[0] == 'C' else new_label}"
        elif self.rules in ("prose", "cited", "links"):
            new_label = self.restore(self.text_rules(label))
        out = f"[{new_label}]({new_target})"
        self.note("link", m.group(0), out)
        return self.keep(out)

    # -- passes
    def text_rules(self, text):
        text = KEYWORD.sub(self.keyword, text)
        text = PART.sub(self.part, text)
        return text

    def prose_only(self, text, bare):
        # Protect code fences and inline code from the bare-number rules.
        text = FENCE.sub(lambda m: self.keep(m.group(0)), text)
        text = INLINE.sub(lambda m: self.keep(m.group(0)), text)
        if bare:
            text = POSSESSIVE.sub(self.possessive, text)
            text = BARE.sub(self.bare, text)
        return text

    def tidy(self, text):
        """'Aethyr 17–22, Aethyr 31' reads as 'Aethyr 17–22, 31'."""
        pattern = re.compile(r"\b(Aethyr|Psionic) (\d+(?:–\d+)?)((?:,\s+and\s+|,\s+|\s+and\s+|\s+or\s+))\1 (?=\d)")
        while True:
            new = pattern.sub(lambda m: f"{m.group(1)} {m.group(2)}{m.group(3)}", text)
            if new == text:
                return text
            text = new

    def run(self, text):
        # Literal replacements and kept phrases are final: no rule rewrites them.
        text = re.sub("\U0010fffc(.*?)\U0010fffc", lambda m: self.keep(m.group(1)), text, flags=re.S)
        for scope, phrase in KEEPS:
            if re.fullmatch(scope, self.path) and phrase in text:
                text = text.replace(phrase, self.keep(phrase))
        text = ROOTPATH.sub(self.rootpath, text)
        if self.rules in ("prose", "cited", "links"):
            text = LINK.sub(self.link, text)
        if self.rules in ("prose", "cited", "code"):
            text = self.text_rules(text)
        if self.rules == "prose":
            m = re.fullmatch(r"psionic/book/ch(\d{3})-.*", self.path)
            if m:
                text = re.sub(r"\A# \d+\. ", f"# {mapped(int(m.group(1)))[1]}. ", text)
            text = self.prose_only(text, bare=re.fullmatch(BARE_FILES, self.path) is not None)
        return self.tidy(self.restore(text))

    def rootpath(self, m):
        old = m.group(0)
        new = self.moves.get(old, old)
        self.note("path", old, new)
        return self.keep(new)


def apply_literals(path, text, report):
    for scope, old, new in LITERALS:
        if old is None:
            if re.fullmatch(scope, path):
                lines = text.splitlines(keepends=True)
                kept = [line for line in lines if not line.startswith(new)]
                if len(kept) != len(lines):
                    report.append((path, "dropline", new, ""))
                text = "".join(kept)
            continue
        if re.fullmatch(scope, path) and old in text:
            report.append((path, "literal", old[:60], new[:60]))
            text = text.replace(old, "\U0010fffc" + new + "\U0010fffc")
    return text


# ---------------------------------------------------------------- the plan

def drop_sections(text, headings):
    for heading in headings:
        start = text.index(f"\n## {heading}\n")
        end = text.index("\n## ", start + 1)
        text = text[:start] + text[end:]
    return text


def drop_rows(text, numbers):
    """Remove README/SUMMARY/PLAN lines that list a chapter now in Aethyr."""
    out = []
    for line in text.splitlines(keepends=True):
        m = re.match(r"^(?:\| |- )\[?(?:\*\*)?(\d+)\. ", line)
        if m and int(m.group(1)) in numbers:
            continue
        out.append(line)
    return "".join(out)


# ---------------------------------------------------------------- claims

def refresh_claims(changed, report):
    ledger_path = ROOT / "coax/book/capability-claims.json"
    ledger = json.loads(ledger_path.read_text())
    pattern = re.compile(r"<!-- capability-claim: ([a-z][a-z0-9.-]*) -->\n(.*?)\n<!-- /capability-claim -->", re.S)
    for claim in ledger["claims"]:
        doc = ROOT / claim["document"]
        for block in pattern.finditer(doc.read_text()):
            if block.group(1) == claim["id"]:
                digest = hashlib.sha256(block.group(2).encode()).hexdigest()
                if digest != claim["text_sha256"]:
                    report.append((claim["document"], "claim-text", claim["id"], digest))
                    claim["text_sha256"] = digest
        for evidence in claim.get("evidence", []):
            if evidence["path"] in changed:
                digest = hashlib.sha256((ROOT / evidence["path"]).read_bytes()).hexdigest()
                if digest != evidence["sha256"]:
                    report.append((evidence["path"], "claim-evidence", claim["id"], digest))
                    evidence["sha256"] = digest
    ledger_path.write_text(json.dumps(ledger, indent=2, ensure_ascii=False) + "\n")


# ---------------------------------------------------------------- main

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args()
    moves = chapter_files()
    if not any(new.startswith("aethyr/") for new in moves.values()):
        raise SystemExit("nothing to move: the Psionic physics chapters are already in the Aethyr book")
    for new in moves.values():
        if new.startswith("aethyr/") and (ROOT / new).exists():
            raise SystemExit(f"target exists: {new}")
    moved_out = {n for first, last, book, _ in RANGES if book == "aethyr" for n in range(first, last + 1)}
    report, results = [], {}
    for path, rules in scoped_files():
        new_path = moves.get(path, path)
        try:
            text = (ROOT / path).read_text()
        except UnicodeDecodeError:
            continue
        if any(c in text for c in "\U0010fffd\U0010fffe\U0010fffc"):
            raise SystemExit(f"{path}: contains a private-use marker")
        original = text
        if path == "psionic/book/PLAN.md":
            text = drop_sections(text, ["XVIII. A physics engine", "XX. Spacecraft and the solar system"])
        if path in ("psionic/book/README.md", "psionic/book/SUMMARY.md", "psionic/book/PLAN.md"):
            text = drop_rows(text, moved_out)
        text = apply_literals(path, text, report)
        text = Rewriter(path, new_path, rules, moves, report).run(text)
        if text != original or new_path != path:
            results[path] = (new_path, text)
    for path, kind, old, new in report:
        print(f"{kind}\t{path}\t{old!r}\t{new!r}")
    print(f"{len(moves)} chapter files move, {len(results)} files change, {len(report)} rewrites", file=sys.stderr)
    if args.dry_run:
        return
    for old, new in sorted(moves.items(), key=lambda kv: int(Path(kv[0]).name[2:5])):
        (ROOT / new).parent.mkdir(parents=True, exist_ok=True)
        subprocess.run(["git", "mv", old, new], cwd=ROOT, check=True)
    for path, (new_path, text) in results.items():
        (ROOT / new_path).write_text(text)
    refresh_claims({new for new, _ in results.values()}, [])


if __name__ == "__main__":
    main()
