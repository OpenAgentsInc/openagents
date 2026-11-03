#!/usr/bin/env python3
# Firm reason: 2 (independent Python reference oracle for the production Coax pin checker, check-pins.coax)
"""Independent reference for check-pins.coax, retained for parity and vector regeneration.

The provenance checks that sit above check_vectors.py (Bitcoin chapter 116).

check_vectors.py proves that each pinned file has the bytes its manifest
entry records. This file checks that the pins agree with each other and with
the books that cite them:

1. Every Bitcoin Core file is pinned at the revision the book targets
   (pins.json), or is a listed exception with its reason.
2. Every manifest entry has exactly one row in SOURCES.md's inventory, with
   the same URL and SHA-256.
3. Every pinned path that a book chapter, an example, a library file or a
   test names is in the manifest.
4. SOURCES.md's "Pins by chapter" index lists, for each Bitcoin chapter, the
   pinned files the chapter cites. --write regenerates it.
5. With --regenerate, every generated *_vectors.coax file under
   coax/tests/bitcoin/ and coax/tests/lightning/ is rebuilt by its make_*.py
   in a scratch copy and compared byte for byte.

Run from the repository root:

    python3 coax/tests/vectors/check_pins.py
    python3 coax/tests/vectors/check_pins.py --regenerate
    python3 coax/tests/vectors/check_pins.py --write     # refresh the index
"""
import argparse
import json
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
CORE = 'bitcoin/bitcoin'
INDEX_BEGIN = '<!-- pins-by-chapter:begin -->'
INDEX_END = '<!-- pins-by-chapter:end -->'
# Folders whose text may name pinned files. The vector folder itself is
# check_vectors.py's.
CITERS = ('bitcoin', 'nostr', 'coax/lib', 'coax/tests')
CITER_SUFFIXES = ('.md', '.py', '.coax', '.json', '.c', '.cjs', '.sh')
# Generators that read their input from the command line.
GENERATOR_ARGS = {
    'coax/tests/bitcoin/make_sighash_vectors.py': ['coax/tests/vectors/upstream/bip143/bip-0143.mediawiki',
                                                   'coax/tests/vectors/upstream/bip341/wallet-test-vectors.json'],
    'coax/tests/bitcoin/make_taproot_vectors.py': ['coax/tests/vectors/upstream/bip341/wallet-test-vectors.json'],
}


def repository(url):
    """owner/name for a raw.githubusercontent.com URL, or None."""
    m = re.match(r'https://raw\.githubusercontent\.com/([^/]+/[^/]+)/', url)
    return m.group(1) if m else None


# ---- 1. the Core target
def core_problems(manifest, pins):
    """Every Core pin at the target revision, or an exception with a reason."""
    target = pins['targets'][CORE]
    revision, exceptions = 'git:' + target['revision'], target.get('exceptions', {})
    problems, report = [], []
    for entry in manifest['files']:
        if repository(entry['url']) != CORE:
            continue
        if entry['revision'] == revision:
            if entry['path'] in exceptions:
                problems.append('%s: listed as an exception but pinned at the target' % entry['path'])
            continue
        reason = exceptions.get(entry['path'])
        report.append((entry['path'], entry['revision'][4:12], reason))
        if not (isinstance(reason, str) and reason.strip()):
            problems.append('%s: Bitcoin Core pin at %s, not the target %s' % (entry['path'], entry['revision'][4:12], target['revision'][:8]))
    paths = {e['path'] for e in manifest['files']}
    problems += ['%s: an exception for a file the manifest does not pin' % p for p in exceptions if p not in paths]
    return problems, report


# ---- 2. the inventory
def inventory_rows(sources):
    section = sources[sources.index('## Inventory'):]
    rows = {}
    for path, url, sha in re.findall(r'^\| \[`([^`]+)`\]\([^)]*\) \| \[[^\]]*\]\(([^)]*)\) \| `([0-9a-f]{64})` \|', section, re.M):
        rows.setdefault(path, []).append((url, sha))
    return rows


def inventory_problems(manifest, sources):
    rows, problems = inventory_rows(sources), []
    files = {e['path']: e for e in manifest['files'] if e['kind'] == 'upstream'}
    for path, entry in files.items():
        found = rows.get(path, [])
        if len(found) != 1:
            problems.append('%s: %d inventory rows in SOURCES.md, not 1' % (path, len(found)))
        elif found[0][1] != entry['sha256']:
            problems.append('%s: the inventory row has another SHA-256' % path)
        elif found[0][0] != entry['url']:
            problems.append('%s: the inventory row links another URL' % path)
    problems += ['%s: an inventory row for a file the manifest does not pin' % p for p in rows if p not in files]
    return problems


# ---- 3. citations
def citations(repo):
    """(cited path, citing file) for every vectors/upstream/... path in the citing folders."""
    found = set()
    for folder in CITERS:
        for f in sorted((repo / folder).rglob('*')):
            if not f.is_file() or f.suffix not in CITER_SUFFIXES or HERE in f.parents:
                continue
            for cited in re.findall(r'tests/vectors/(upstream/[A-Za-z0-9_./+-]*[A-Za-z0-9_+-])', f.read_text(errors='replace')):
                found.add((cited.rstrip('.,)/'), f.relative_to(repo).as_posix()))
    return found


def citation_problems(manifest, cited):
    paths = {e['path'] for e in manifest['files']}
    problems = []
    for path, by in sorted(cited):
        if path not in paths and not any(p.startswith(path + '/') for p in paths):
            problems.append('%s names %s, which the manifest does not pin' % (by, path))
    return problems


# ---- 4. the index by chapter
def chapter_index(repo, manifest):
    paths = {e['path'] for e in manifest['files']}
    lines = []
    for doc in sorted((repo / 'bitcoin/book').glob('ch*.md')):
        cited = sorted({c.rstrip('.,)/') for c in re.findall(r'coax/tests/vectors/(upstream/[A-Za-z0-9_./+-]*[A-Za-z0-9_+-])', doc.read_text())})
        cited = [c for c in cited if c in paths or any(p.startswith(c + '/') for p in paths)]
        if cited:
            n = int(re.match(r'ch(\d+)', doc.name).group(1))
            lines.append('- Bitcoin chapter %d: %s' % (n, ', '.join('`%s`' % c for c in cited)))
    return '\n'.join([INDEX_BEGIN, *lines, INDEX_END])


def index_problems(repo, manifest, sources):
    if INDEX_BEGIN not in sources or INDEX_END not in sources:
        return ['SOURCES.md has no pins-by-chapter index']
    have = sources[sources.index(INDEX_BEGIN):sources.index(INDEX_END) + len(INDEX_END)]
    return [] if have == chapter_index(repo, manifest) else ['SOURCES.md: the pins-by-chapter index is stale (run --write)']


# ---- 5. generated vectors
def generators(repo):
    for folder in ('coax/tests/bitcoin', 'coax/tests/lightning'):
        yield from sorted((repo / folder).glob('make_*.py'))


def regenerate_problems(repo):
    """Rebuild every generated file in a scratch copy of coax/tests and compare."""
    problems, count = [], 0
    with tempfile.TemporaryDirectory(prefix='coax-pins-') as tmp:
        scratch = Path(tmp)
        shutil.copytree(repo / 'coax/tests', scratch / 'coax/tests',
                        ignore=shutil.ignore_patterns('__pycache__', '.cache'))
        before = {p: p.read_bytes() for p in (scratch / 'coax/tests').rglob('*') if p.is_file()}
        for g in generators(repo):
            name = g.relative_to(repo).as_posix()
            args = [str(scratch / a) for a in GENERATOR_ARGS.get(name, [])]
            r = subprocess.run([sys.executable, '-I', '-B', str(scratch / name), *args], cwd=scratch,
                               capture_output=True, text=True, timeout=600)
            if r.returncode:
                problems.append('%s: exit %d: %s' % (name, r.returncode, (r.stderr or r.stdout).strip()[-200:]))
            count += 1
        for p in sorted((scratch / 'coax/tests').rglob('*')):
            if p.is_file() and '__pycache__' not in p.parts and before.get(p) != p.read_bytes():
                problems.append('%s: its generator writes different bytes' % p.relative_to(scratch).as_posix())
    return problems, count


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--write', action='store_true', help="refresh SOURCES.md's pins-by-chapter index")
    parser.add_argument('--regenerate', action='store_true', help='also rebuild every generated vector file')
    args = parser.parse_args()
    manifest = json.loads((HERE / 'manifest.json').read_text())
    pins = json.loads((HERE / 'pins.json').read_text())
    sources_path = HERE / 'SOURCES.md'
    sources = sources_path.read_text()
    if args.write:
        start, end = sources.index(INDEX_BEGIN), sources.index(INDEX_END) + len(INDEX_END)
        sources = sources[:start] + chapter_index(REPO, manifest) + sources[end:]
        sources_path.write_text(sources)
    problems, report = core_problems(manifest, pins)
    problems += inventory_problems(manifest, sources)
    cited = citations(REPO)
    problems += citation_problems(manifest, cited)
    problems += index_problems(REPO, manifest, sources)
    target = pins['targets'][CORE]
    core = sum(1 for e in manifest['files'] if repository(e['url']) == CORE)
    print('Bitcoin Core target %s (%s): %d pinned files, %d at another revision'
          % (target['revision'][:8], target['describe'], core, len(report)))
    for path, revision, reason in report:
        print('  %s at %s: %s' % (path, revision, reason or 'NOT LISTED'))
    for repo_name, revisions in sorted(other_revisions(manifest).items()):
        print('  note: %s is pinned at %d revisions: %s' % (repo_name, len(revisions), ', '.join(r[4:12] for r in sorted(revisions))))
    print('%d manifest entries, %d citations checked' % (len(manifest['files']), len(cited)))
    if args.regenerate:
        more, count = regenerate_problems(REPO)
        problems += more
        print('%d generators rebuilt their files in a scratch copy' % count)
    for p in problems:
        print('problem: ' + p, file=sys.stderr)
    if problems:
        sys.exit('check_pins.py: %d problems' % len(problems))
    print('check_pins.py: every pin agrees')


def other_revisions(manifest):
    by = {}
    for e in manifest['files']:
        name = repository(e['url'])
        if name and name != CORE:
            by.setdefault(name, set()).add(e['revision'])
    return {k: v for k, v in by.items() if len(v) > 1}


if __name__ == '__main__':
    main()
