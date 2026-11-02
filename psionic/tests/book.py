#!/usr/bin/env python3
"""Book tooling: closed example inventory, exact source listings, links and runs.

One checker serves the Psionic, Aethyr, Nostr and Coder books
(`--book psionic|aethyr|nostr|coder`). Each book keeps chapters as
chNNN-slug.md, an examples.json inventory of its examples/*.coax programs, and
a status marker on every design draft. Coder has no example programs of its
own yet: the programs its chapters run are inventoried by another book.
Native-only library examples have a separate closed inventory with their
host consumer and retained evidence; --coax replays runnable examples only.
"""
import argparse
import importlib.util
from concurrent.futures import ThreadPoolExecutor
import os
import json
from pathlib import Path
import re
import resource
import subprocess
from urllib.parse import unquote, urlparse

ROOT = Path(__file__).resolve().parents[2]
BOOKS = {
    # book directory, example programs, project README, least source listings
    "psionic": ("psionic/book", "psionic/examples", "psionic/README.md", 3),
    "aethyr": ("aethyr/book", "aethyr/examples", "aethyr/README.md", 0),
    "nostr": ("nostr/book", "nostr/examples", "nostr/README.md", 0),
    "coder": ("coder/book", None, None, 1),
}
# Coder's chapters 1-22 moved from Psionic; their programs stay in Psionic's
# inventory (and its closed-inventory check) until their code moves.
BORROWED = {"coder": "psionic"}
# Fences a book may use besides jai, shell and text.
FENCES = {"coder": ("```json", "```sh")}
# Aethyr's engine-core tasks each own one chapter, named in its status block.
TASKS = {"aethyr": "apps/openagents-web/projects/verse/work/engine-core.json"}
DRAFT = "> **Status: unimplemented — design draft.**"
# Kinds a book's library must never build (Nostr chapter 74: Cashu and
# Fedimint, excluded by AGENTS.md's Bitcoin-only rule). The table file holds
# the kinds; the pinned README and NIP texts must agree with it; and only the
# files below may name a kind or an ecash word, each for its stated reason.
# A file is added here only with an owner decision recorded in NEEDS_OWNER.md.
EXCLUDED = {
    "nostr": {
        "table": "coax/lib/nostr/ecash.coax",
        "readme": "nostr/nips/official/README.md",
        "nips": ("60", "61", "87"),
        "other": {0: "NIP-01 metadata", 5: "NIP-09 deletion"},
        "scan": ("coax/lib/nostr", "nostr/examples"),
        "allowed": {
            "coax/lib/nostr/ecash.coax": "chapter 74: recognizes the kinds, builds nothing",
            "coax/lib/nostr/nip15.coax": "chapter 66: refuses cashu payment options",
            "nostr/examples/ecash-excluded.coax": "chapter 74's example",
            "nostr/examples/checks/ecash-excluded.coax": "chapter 74's checks",
            "nostr/examples/markets-orders.coax": "chapter 66's example: shows the cashu refusal",
        },
    },
}


def path(name):
    result = (ROOT / name).resolve()
    result.relative_to(ROOT)
    assert result.is_file(), f"missing file: {name}"
    return result


def example_inventory(manifest, actual):
    examples = manifest["examples"]
    libraries = manifest.get("libraries", [])
    assert isinstance(examples, list) and isinstance(libraries, list), "example inventories must be lists"
    sources = [item["source"] for item in examples + libraries]
    assert len(sources) == len(set(sources)), "duplicate example"
    assert set(sources) == actual, "example inventory does not cover every program"
    for item in libraries:
        assert item.get("kind") == "native-library", "library requires native-library kind"
        assert all(isinstance(item.get(key), str) and item[key] for key in ("chapter", "consumer")), "library requires chapter and host consumer"
        assert isinstance(item.get("evidence"), list) and item["evidence"] and all(isinstance(p, str) and p for p in item["evidence"]), "library requires retained evidence paths"
        assert not {"stdout", "exit_status", "modes"}.intersection(item), "native library is not an evaluator transcript"
    return examples, libraries


def check(name="psionic"):
    book, programs, readme, least = BOOKS[name]
    spec = importlib.util.spec_from_file_location("coax_book_claims", ROOT / "coax/book/capability-claims.py")
    claims = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(claims)
    errors = claims.problems(ROOT)
    assert not errors, "reviewed capability claims: " + "; ".join(errors)
    spec = importlib.util.spec_from_file_location("coax_book_structure", ROOT / "coax/book/check-structure.py")
    registry = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(registry)
    errors = registry.registry_problems(name, ROOT)
    assert not errors, f"chapter registry ({book}/chapters.json): " + "; ".join(errors)
    if name == "psionic":
        # Chapter 250's capability matrix is generated from the registry; its prose claims are bound to it.
        spec = importlib.util.spec_from_file_location("psionic_pytorch_matrix", ROOT / "psionic/tests/pytorch_matrix.py")
        matrix = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(matrix)
        errors = matrix.problems()
        assert not errors, "chapter 250 capability matrix: " + "; ".join(errors)
    if name in BORROWED:
        owner = BORROWED[name]
        manifest = json.loads(path(f"{BOOKS[owner][0]}/examples.json").read_text())
        examples = [item for item in manifest["examples"] if item["chapter"].startswith(book + "/")]
        libraries = [item for item in manifest.get("libraries", []) if item["chapter"].startswith(book + "/")]
    else:
        manifest = json.loads(path(f"{book}/examples.json").read_text())
        assert manifest["schema"] == 1
        actual = {str(p.relative_to(ROOT)) for p in (ROOT / programs).glob("*.coax")}
        examples, libraries = example_inventory(manifest, actual)
        assert examples or least == 0, "empty example inventory"
    linked = set()
    listings = 0
    docs = ([ROOT / readme] if readme else []) + sorted((ROOT / book).glob("*.md"))
    # Chapters that moved to another book (the harness, now Coder chapters
    # 1-22) keep their examples here until their code moves; check them too.
    docs += sorted({ROOT / item["chapter"] for item in examples
                    if not item["chapter"].startswith(book + "/")})
    chapters(name, book)
    pattern = r"<!-- source: ([^ ]+) ([^ ]+) -->\n```jai\n(.*?)\n```"
    for doc in docs:
        text = doc.read_text()
        assert text.endswith("\n") and text.count("```") % 2 == 0, f"bad structure: {doc}"
        in_block = False
        for line in text.splitlines():
            if line.startswith("```"):
                if not in_block:
                    # A project README may also use plain sh fences.
                    fences = ("```jai", "```shell", "```text") + (("```sh",) if doc.name == "README.md" and doc.parent.name != "book" else ())
                    fences += FENCES.get(name, ()) if doc.is_relative_to(ROOT / book) else ()
                    assert line in fences, f"book fence must be jai, shell or text: {doc}: {line}"
                else:
                    assert line == "```", f"bad closing fence: {doc}: {line}"
                in_block = not in_block
        for label, href in re.findall(r"\[([^\]]+)\]\(([^)]+)\)", text):
            parsed = urlparse(href)
            if not parsed.scheme and parsed.path:
                target = (doc.parent / unquote(parsed.path)).resolve()
                target.relative_to(ROOT)
                assert target.exists(), f"broken link in {doc}: {href}"
                linked.add(target)
        found = list(re.finditer(pattern, text, re.DOTALL))
        assert len(found) == text.count("<!-- source:"), f"malformed listing: {doc}"
        for match in found:
            source, tag, listing = match.groups()
            data = path(source).read_text()
            begin, end = f"// book:begin {tag}\n", f"// book:end {tag}"
            assert data.count(begin) == data.count(end) == 1, f"bad source markers: {source} {tag}"
            expected = data.split(begin)[1].split(end)[0].rstrip("\n")
            assert listing == expected, f"listing drift: {doc.name} -> {source} {tag}"
            listings += 1
    assert listings >= least, "required library/example listings missing"
    if name in EXCLUDED:
        excluded(EXCLUDED[name])
    for item in examples:
        source = path(item["source"])
        chapter = path(item["chapter"])
        path(item["stdout"])
        assert source in linked and chapter in linked, f"unlinked example/chapter: {source}"
        assert type(item["exit_status"]) is int and 0 <= item["exit_status"] <= 255
        assert item["modes"] == ["debug", "release"], "both build modes are required"
        steps = item.get("run_steps")
        assert steps is None or (type(steps) is int and 1 <= steps <= 250000000), f"run_steps is 1 to 250000000: {source}"
        limit = item.get("timeout")
        assert limit is None or (type(limit) is int and 60 <= limit <= 600), f"timeout is 60 to 600 seconds: {source}"
    for item in libraries:
        source, chapter = path(item["source"]), path(item["chapter"])
        assert source in linked and chapter in linked, f"unlinked library/chapter: {source}"
        path(item["consumer"])
        for evidence in item["evidence"]:
            path(evidence)
    print(f"{name} book structure: {len(examples)} programs, {len(libraries)} native libraries, {listings} source-linked listings, local links pass")
    return examples


def excluded(rule):
    """The excluded kinds match the pinned texts, and no other file names them."""
    table = path(rule["table"]).read_text()

    def constant(label):
        found = re.search(label + r" :: \[[A-Z_0-9]+\] (?:int|string)\.\[(.*?)\];", table, re.DOTALL)
        assert found, f"{rule['table']}: no {label}"
        return re.findall(r'"([^"]*)"|(\d+)', found.group(1))

    kinds = [int(n) for _, n in constant("EC_KIND")]
    rows = list(zip(kinds, [t for t, _ in constant("EC_NAME")], [t for t, _ in constant("EC_NIP")]))
    words = [t for t, _ in constant("EC_WORDS")]
    readme = path(rule["readme"]).read_text()
    pinned = [(int(k), name, nip) for k, name, nip in re.findall(r"^\| `(\d+)` *\| (.+?) *\| \[(\w+)\]\(\3\.md\)", readme, re.M) if nip in rule["nips"]]
    assert pinned == rows, f"{rule['table']} differs from {rule['readme']}: {pinned} != {rows}"
    for nip in rule["nips"]:
        text = path(f"{Path(rule['readme']).parent}/{nip}.md").read_text()
        mentioned = {int(a or b) for a, b in re.findall(r'kind:(\d+)|"kind": (\d+)', text)}
        extra = mentioned - set(kinds) - set(rule["other"])
        assert not extra, f"NIP-{nip} names a kind in no table: {extra}"
        own = {k for k, _, n in rows if n == nip}
        assert own <= mentioned, f"NIP-{nip} never names {own - mentioned}"
    number = re.compile(r"(?<![0-9A-Za-z_])(?:" + "|".join(map(str, kinds)) + r")(?![0-9A-Za-z_])")
    naming = set()
    for top in rule["scan"]:
        for source in sorted((ROOT / top).rglob("*.coax")):
            text = source.read_text()
            if number.search(text) or any(w in text.lower() for w in words):
                naming.add(str(source.relative_to(ROOT)))
    allowed = set(rule["allowed"])
    assert naming <= allowed, f"files name an excluded kind or ecash with no owner decision: {sorted(naming - allowed)}"
    assert allowed <= naming, f"allowed files that no longer name ecash: {sorted(allowed - naming)}"


def chapters(name, book):
    """Chapter files, headings, contents and plan agree, and numbers have no gaps."""
    files = {}
    for doc in (ROOT / book).glob("ch*.md"):
        # Chapter files carry three-digit numbers so they list in reading order.
        assert re.fullmatch(r"ch\d{3}-[a-z0-9-]+\.md", doc.name), f"chapter file needs a three-digit number: {doc.name}"
        number = int(doc.name[2:5])
        assert number not in files, f"two chapter files numbered {number}"
        files[number] = doc
        lines = doc.read_text().splitlines()
        title = re.fullmatch(r"# (?:(\d+)\.|Chapter (\d+):) .+", lines[0])
        assert title and int(title.group(1) or title.group(2)) == number, f"heading does not match the file number: {doc.name}"
        draft = len(lines) > 2 and lines[2].startswith(DRAFT)
        assert draft or "<!-- source:" in doc.read_text() or DRAFT not in doc.read_text(), f"misplaced status marker: {doc.name}"
        if draft:
            assert "<!-- source:" not in doc.read_text(), f"a design draft has source-linked listings: {doc.name}"
    summary = (ROOT / book / "SUMMARY.md").read_text()
    listed = re.findall(r"^- \[(\d+)\. [^\]]+\]\((ch\d{3}-[a-z0-9-]+\.md)\)", summary, re.M)
    names = [f for _, f in listed]
    assert len(names) == len(set(names)), f"{book}/SUMMARY.md lists a chapter twice"
    for number, f in listed:
        assert int(number) == int(f[2:5]), f"SUMMARY number does not match {f}"
    assert set(names) == {p.name for p in files.values()}, f"{book}/SUMMARY.md does not list exactly the chapter files"
    assert [int(n) for n, _ in listed] == sorted(int(n) for n, _ in listed), f"{book}/SUMMARY.md is out of order"
    plan = ROOT / book / "PLAN.md"
    text = plan.read_text() if plan.is_file() else ""
    # A plan that tables its chapters, written or not, numbers them 1..N once each.
    planned = [int(n) for n in re.findall(r"^\| (?:\[|\*\*)?(\d+)\. ", text, re.M)]
    planned += [int(n) for n in re.findall(r"^\| \d+ \| (?:\[|\*\*)(\d+)\. ", text, re.M)]
    if planned:
        assert sorted(planned) == list(range(1, len(planned) + 1)), f"{book}/PLAN.md chapter numbers are not 1..N once each"
        assert set(files) <= set(planned), f"{book}/PLAN.md is missing a written chapter"
    else:
        assert sorted(files) == list(range(1, len(files) + 1)), f"{book} chapter numbers have a gap"
    if name in TASKS:
        tasks = [t["slug"] for t in json.loads(path(TASKS[name]).read_text())["tasks"]]
        status = {}
        for number, doc in files.items():
            block = []
            for line in doc.read_text().splitlines()[2:]:
                if not line.startswith(">"):
                    break
                block.append(line)
            status[number] = " ".join(block)
        for task in tasks:
            owners = [n for n, block in status.items() if f"`{task}`" in block]
            assert len(owners) == 1, f"engine-core task {task} needs exactly one chapter, has {owners}"


def run_one(compiler, item, mode):
    cmd = [compiler, "run", str(path(item["source"]))]
    if mode == "release":
        cmd.append("--release")
    # A program whose compile-time tables need more than the default budget.
    if "run_steps" in item:
        cmd += ["--run-steps", str(item["run_steps"])]
    # A program whose fixed state is larger than the default 8 MiB stack.
    stack = item.get("stack_kib")
    limit = (lambda: resource.setrlimit(resource.RLIMIT_STACK, (stack * 1024, resource.getrlimit(resource.RLIMIT_STACK)[1]))) if stack else None
    # A program that loads the whole signer compiles for longer than a minute.
    result = subprocess.run(cmd, cwd=ROOT, capture_output=True, timeout=item.get("timeout", 60), preexec_fn=limit)
    assert result.returncode == item["exit_status"], f"exit status: {cmd}\n{result.stderr.decode()}"
    assert not result.stderr, f"unexpected diagnostics: {cmd}\n{result.stderr.decode()}"
    assert result.stdout == path(item["stdout"]).read_bytes(), f"transcript drift: {cmd}\n{result.stdout.decode()}"


def run(compiler, examples, jobs=1):
    runs = [(item, mode) for item in examples for mode in item["modes"]]
    with ThreadPoolExecutor(max_workers=max(1, jobs)) as pool:
        for future in [pool.submit(run_one, compiler, item, mode) for item, mode in runs]:
            future.result()  # in inventory order: the first failure is reported
    print(f"book execution: {len(examples) * 2} runs match output and exit status")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--book", choices=sorted(BOOKS), default="psionic")
    parser.add_argument("--coax", help="also compile and run every example")
    parser.add_argument("--jobs", type=int, default=os.cpu_count() or 1, help="examples run at once")
    args = parser.parse_args()
    examples = check(args.book)
    if args.coax:
        run(str(Path(args.coax).resolve()), examples, args.jobs)


if __name__ == "__main__":
    main()
