#!/usr/bin/env python3
# Firm reason: 2 (independent Markdown/HTML/JSON oracle for the Coax generator).
"""Recompute public changelog content independently and exercise failure atomicity."""
from html.parser import HTMLParser
from pathlib import Path
import json
import re
import subprocess
import tempfile
import unittest
from urllib.parse import urljoin

ROOT = Path(__file__).resolve().parents[3]
APP = ROOT / "apps/openagents-web"
SOURCE_URL = "https://github.com/OpenAgentsInc/openagents/blob/main/coax/CHANGELOG.md"
OUTPUTS = ["apps/openagents-web/changelog-content.coax", "apps/openagents-web/coax/changelog/index.html", "apps/openagents-web/coax/changelog/index.md"]


def display(text):
    return text.translate(str.maketrans({"—": "-", "–": "-", "‘": "'", "’": "'", "“": '"', "”": '"'}))


def flattened(text):
    text = re.sub(r"\[([^\]\n]+)\]\(([^)\n]+)\)", lambda match: match[1], text)
    return re.sub(r"\s+", " ", text.replace("`", "").replace("**", "")).strip()


def constants(text):
    result = {}
    for name, value in re.findall(r"^(CC_[A-Z_]+) :: (.*);$", text, re.M):
        value = re.sub(r"^\[CC_BLOCKS\](?:int|string)\.", "", value)
        result[name] = json.loads(value)
    return result


class AccessibleHTML(HTMLParser):
    def __init__(self):
        super().__init__(convert_charrefs=True)
        self.main = False
        self.recording = False
        self.current = []
        self.blocks = []
        self.links = []
        self.active_href = None
        self.active_label = []
        self.tags = []

    def handle_starttag(self, tag, attrs):
        self.tags.append(tag)
        attrs = dict(attrs)
        if tag == "main":
            self.main = True
        if self.main and tag in ("p", "h2"):
            self.recording = True
            self.current = []
        if self.main and tag == "a":
            self.active_href = attrs.get("href")
            self.active_label = []

    def handle_data(self, data):
        if self.recording:
            self.current.append(data)
        if self.active_href is not None:
            self.active_label.append(data)

    def handle_endtag(self, tag):
        if self.main and tag in ("p", "h2"):
            self.blocks.append("".join(self.current))
            self.recording = False
        if tag == "a" and self.active_href is not None:
            self.links.append(("".join(self.active_label), self.active_href))
            self.active_href = None
        if tag == "main":
            self.main = False


class ChangelogOracle(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.tool = ROOT / "coax/web/build/tools/changelog-page"
        result = subprocess.run([str(ROOT / "coax/web/tool.sh"), "coax/web/changelog-page.coax", "--check"], cwd=ROOT, capture_output=True, text=True, timeout=120)
        if result.returncode != 0:
            raise AssertionError(result.stdout + result.stderr)
        cls.version = re.search(r'IMPLEMENTATION_VERSION :: "([^"]+)"', (ROOT / "coax/release.coax").read_text())[1]

    def test_all_canonical_words_releases_links_and_raw_bytes_survive(self):
        source = (ROOT / "coax/CHANGELOG.md").read_text()
        data = constants((APP / "changelog-content.coax").read_text())
        self.assertEqual(data["CC_TITLE"], "Coax changelog")
        self.assertEqual(data["CC_SOURCE_URL"], SOURCE_URL)
        self.assertEqual(data["CC_LATEST"], self.version)
        self.assertEqual(data["CC_MARKDOWN"], source)
        self.assertEqual((APP / "coax/changelog/index.md").read_bytes(), (ROOT / "coax/CHANGELOG.md").read_bytes())
        for name in ("CC_KIND", "CC_TEXT", "CC_LINK"):
            self.assertEqual(len(data[name]), data["CC_BLOCKS"])
        headings = re.findall(r"^## (.+)$", source, re.M)
        self.assertEqual([text for kind, text in zip(data["CC_KIND"], data["CC_TEXT"]) if kind == 1], list(map(display, headings)))
        expected_links = [(display(flattened(label)), urljoin(SOURCE_URL, href)) for label, href in re.findall(r"\[([^\]\n]+)\]\(([^)\n]+)\)", source)]
        actual_links = [(text, href) for kind, text, href in zip(data["CC_KIND"], data["CC_TEXT"], data["CC_LINK"]) if kind == 3]
        self.assertEqual(actual_links, expected_links)
        # Independent whole-document word conservation, omitting repeated link rows.
        expected = re.sub(r"(?m)^# Coax changelog\n|^## ", "", source)
        expected = display(flattened(expected))
        actual = " ".join(text for kind, text in zip(data["CC_KIND"], data["CC_TEXT"]) if kind != 3)
        self.assertEqual(actual, expected)
        html = (APP / "coax/changelog/index.html").read_text()
        parsed = AccessibleHTML()
        parsed.feed(html)
        self.assertEqual(list(map(display, parsed.blocks[1:])), data["CC_TEXT"])
        self.assertEqual([(display(label), href) for label, href in parsed.links[3:]], actual_links)
        self.assertIn('.gpu main{', html)
        self.assertIn('data-app="changelog"', html)
        self.assertIn('href="https://openagents.com/coax/changelog"', html)
        self.assertIn('type="text/markdown" href="/coax/changelog/index.md"', html)
        self.assertFalse(set(parsed.tags) & {"button", "input", "form", "div", "pre"})

    def fixture(self, note='- Encode [source](STATUS.md) <script> & "quote" safely.', version=None):
        version = version or self.version
        digest = "1" * 40
        return f"# Coax changelog\n\n## {version} — 2026-10-31\n\nCommit: [`{digest}`](https://github.com/OpenAgentsInc/openagents/commit/{digest})\n\n{note}\n"

    def run_tool(self, root, mode):
        return subprocess.run([str(self.tool), mode], cwd=root, capture_output=True, text=True, timeout=30)

    def prepare(self, root, source):
        (root / "coax").mkdir(exist_ok=True)
        (root / "coax/CHANGELOG.md").write_text(source)
        for name in OUTPUTS:
            path = root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(b"previous complete output\n")

    def snapshot(self, root):
        return {name: (root / name).read_bytes() for name in OUTPUTS}

    def test_escaping_link_resolution_and_idempotent_drift_checks(self):
        with tempfile.TemporaryDirectory(prefix="coax-changelog-oracle-") as temporary:
            root = Path(temporary)
            self.prepare(root, self.fixture('- Encode [source](../docs/one.md), [anchor](#release), [balanced](https://example.org/a(b)) and [external](https://example.org/a?q=one&two=2) with `call[0](value)` <script> & "quote" safely.'))
            result = self.run_tool(root, "--write")
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            html = (root / OUTPUTS[1]).read_text()
            self.assertIn("&lt;script&gt; &amp; &quot;quote&quot;", html)
            self.assertNotIn("<script> &", html)
            parsed = AccessibleHTML(); parsed.feed(html)
            links = dict(parsed.links)
            self.assertEqual(links["source"], "https://github.com/OpenAgentsInc/openagents/blob/main/coax/../docs/one.md")
            self.assertEqual(links["anchor"], SOURCE_URL + "#release")
            self.assertEqual(links["balanced"], "https://example.org/a(b)")
            self.assertEqual(links["external"], "https://example.org/a?q=one&two=2")
            before = self.snapshot(root)
            mtimes = {name: (root / name).stat().st_mtime_ns for name in OUTPUTS}
            self.assertEqual(self.run_tool(root, "--check").returncode, 0)
            self.assertEqual(self.run_tool(root, "--write").returncode, 0)
            self.assertEqual(before, self.snapshot(root))
            self.assertEqual(mtimes, {name: (root / name).stat().st_mtime_ns for name in OUTPUTS})
            (root / OUTPUTS[0]).write_bytes(b"stale\n")
            stale = self.snapshot(root)
            self.assertNotEqual(self.run_tool(root, "--check").returncode, 0)
            self.assertEqual(stale, self.snapshot(root))
            self.assertEqual(self.run_tool(root, "--write").returncode, 0)
            self.assertEqual(before, self.snapshot(root))

    def test_malformed_or_oversized_sources_never_change_outputs(self):
        major, minor, patch = map(int, self.version.split('.'))
        other = f"{major}.{minor}.{patch + 1}" if patch < 999 else f"{major}.{minor + 1}.0"
        cases = {
            "missing-url": self.fixture("- Refuse [empty]() before publication."),
            "unfinished-url": self.fixture("- Refuse [broken](https://example.org before publication."),
            "javascript": self.fixture("- Refuse [active](javascript:alert) before publication."),
            "insecure-url": self.fixture("- Refuse [insecure](http://example.org) before publication."),
            "empty-https-authority": self.fixture("- Refuse [empty](https://) before publication."),
            "wrong-version": self.fixture(version=other),
            "pending-commit": re.sub(r"Commit: .*", "Commit: pending", self.fixture(), count=1),
            "too-much-source": self.fixture() + "x" * 262145,
            "too-many-blocks": self.fixture("- A bounded concrete release.\n" * 4200),
            "overlong-paragraph": self.fixture("- " + "x" * 8193),
        }
        with tempfile.TemporaryDirectory(prefix="coax-changelog-oracle-") as temporary:
            root = Path(temporary)
            for name, source in cases.items():
                for mode in ("--write", "--check"):
                    with self.subTest(case=name, mode=mode):
                        self.prepare(root, source)
                        before = self.snapshot(root)
                        result = self.run_tool(root, mode)
                        self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
                        self.assertEqual(before, self.snapshot(root))


if __name__ == "__main__":
    unittest.main()
